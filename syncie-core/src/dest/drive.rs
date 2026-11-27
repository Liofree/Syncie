use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use super::Destination;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenStore {
    access_token: String,
    refresh_token: String,
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    client_secret: String,
}

/// Prüft den HTTP-Status und liefert erst dann den JSON-Body.
/// Ohne das werden API-Fehler (401/403) still zu leeren Ergebnislisten.
async fn json_ok(resp: reqwest::Response, ctx: &str) -> Result<serde_json::Value, String> {
    let status = resp.status();
    let body: serde_json::Value = resp.json().await
        .map_err(|e| format!("{ctx} parse: {e}"))?;
    if !status.is_success() {
        let msg = body["error"]["message"].as_str().unwrap_or("unbekannter Fehler");
        return Err(format!("{ctx}: HTTP {} — {msg}", status.as_u16()));
    }
    Ok(body)
}

pub struct DriveDest {
    folder_id: String,
    token_path: PathBuf,
    client: reqwest::Client,
    /// Ben, 05.08.2026: Zugriffstoken samt Ablauf.
    ///
    /// Vorher holte JEDER einzelne Aufruf ein frisches Token - bei 1.800
    /// Dateien in zwei Bereichen sind das mehrere tausend Anfragen an
    /// Googles Anmeldeserver, nur um immer dasselbe zu bekommen. Genau
    /// daran ist der Lauf gestorben: "OAuth: HTTP 503". Ein Token gilt
    /// eine Stunde; einmal holen reicht.
    token_cache: std::sync::Mutex<Option<(String, std::time::Instant)>>,
    /// Ordnerpfad (relativ zum Wurzelordner, z.B. "Spiegel/LUNA/data") ->
    /// Drive-Ordner-ID. Ben, 05.08.2026: Ohne den Cache loeste JEDE Datei
    /// ihren Ordnerpfad neu auf - resolve_folder und ensure_folder liefen
    /// Abschnitt fuer Abschnitt per API von der Wurzel los, fuer jede der
    /// tausenden Dateien im selben Ordner erneut. Das war der Grund, warum
    /// ein Ordner mit vielen kleinen Dateien quaelend langsam hochlud.
    /// Innerhalb eines Laufs werden Ordner nur angelegt, nie geloescht -
    /// eine gemerkte ID bleibt also gueltig.
    folder_cache: std::sync::Mutex<HashMap<String, String>>,
    /// Seit die Uploads parallel laufen (Ben, 19.10.2026): zwei Dateien im
    /// selben NEUEN Ordner legten ihn sonst zweimal an - Drive erlaubt
    /// gleichnamige Ordner. Ordner anlegen geht deshalb nur einzeln.
    ordner_sperre: tokio::sync::Mutex<()>,
}

/// Fehler, die von selbst wieder weggehen: Server überlastet, gedrosselt,
/// kurzer Aussetzer. Bei denen lohnt ein zweiter Versuch - im Gegensatz zu
/// 401/404, wo dieselbe Antwort wiederkäme.
///
/// Ben, 23.08.2026: 403 ist bei Google BEIDES. "User rate limit exceeded"
/// kommt als 403 und geht nach ein paar Sekunden weg; eine fehlende
/// Berechtigung kommt ebenfalls als 403 und geht nie weg. Der Code allein
/// entscheidet das nicht - deshalb zaehlt hier der Grund im Koerper. Genau
/// daran ist der erste grosse Lauf gestorben.
fn ist_voruebergehend(code: u16, koerper: &str) -> bool {
    matches!(code, 429 | 500 | 502 | 503 | 504)
        || (code == 403
            && (koerper.contains("rateLimitExceeded")
                || koerper.contains("userRateLimitExceeded")
                || koerper.contains("User rate limit")
                || koerper.contains("Rate Limit Exceeded")))
}

/// Wartezeit vor dem nächsten Versuch: 1s, 2s, 4s, 8s, 16s - plus bis zu
/// einer halben Sekunde Streuung. Ohne die Streuung laufen alle
/// gedrosselten Anfragen im Gleichtakt wieder los und werden erneut
/// gedrosselt. Die Streuung kommt aus der Uhr, damit kein Zufallspaket
/// dafuer noetig ist.
fn wartezeit(versuch: u32) -> std::time::Duration {
    let streuung = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::from(d.subsec_millis()) % 500);
    std::time::Duration::from_millis((1000u64 << versuch) + streuung)
}

/// Fuenf Versuche statt drei: bei einem Ratenlimit bringt der zweite
/// Versuch nach einer Sekunde selten etwas, der nach 16 schon.
const VERSUCHE: u32 = 5;

impl DriveDest {
    pub fn new(folder_id: String, token_path_override: Option<PathBuf>) -> Result<Self, String> {
        let token_path = token_path_override.unwrap_or_else(|| {
            std::env::var("APPDATA")
                .or_else(|_| std::env::var("HOME"))
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("syncie")
                .join("drive_token.json")
        });

        if !token_path.exists() {
            return Err(format!(
                "Google-Drive-Token nicht gefunden: {}\n\n\
                So erstellst du den Token:\n\
                1. https://console.cloud.google.com → Projekt erstellen → Drive API aktivieren\n\
                2. OAuth 2.0 Client-ID (Desktop-App) erstellen\n\
                3. https://developers.google.com/oauthplayground öffnen\n\
                4. Drive API v3 → https://www.googleapis.com/auth/drive.file\n\
                5. \"Authorize APIs\" → \"Exchange authorization code for tokens\"\n\
                6. Refresh Token kopieren\n\
                7. Datei speichern als: {}\n\
                Format: {{\"access_token\":\"...\",\"refresh_token\":\"...\"}}",
                token_path.display(),
                token_path.display(),
            ));
        }

        Ok(Self {
            folder_id,
            token_path,
            client: reqwest::Client::new(),
            token_cache: std::sync::Mutex::new(None),
            folder_cache: std::sync::Mutex::new(HashMap::new()),
            ordner_sperre: tokio::sync::Mutex::new(()),
        })
    }

    /// Gültiges Zugriffstoken - aus dem Zwischenspeicher, solange es hält.
    ///
    /// Der Mutex wird bewusst NICHT über ein await gehalten: sperren, Wert
    /// lesen, sofort freigeben. Sonst blockiert ein langsamer Netzaufruf
    /// alle anderen Threads.
    async fn access_token(&self) -> Result<String, String> {
        if let Ok(cache) = self.token_cache.lock() {
            if let Some((tok, gueltig_bis)) = cache.as_ref() {
                if std::time::Instant::now() < *gueltig_bis {
                    return Ok(tok.clone());
                }
            }
        }

        let (tok, sekunden) = self.token_frisch_holen().await?;

        // 60 s Sicherheitsabstand: ein Token, das mitten im Upload abläuft,
        // kostet sonst einen unnötigen Fehlschlag.
        let haltbar = std::time::Duration::from_secs(sekunden.saturating_sub(60).max(60));
        if let Ok(mut cache) = self.token_cache.lock() {
            *cache = Some((tok.clone(), std::time::Instant::now() + haltbar));
        }
        Ok(tok)
    }

    /// Holt ein neues Token. Wiederholt bei vorübergehenden Fehlern -
    /// genau hier schlug der Lauf mit "HTTP 503" fehl.
    async fn token_frisch_holen(&self) -> Result<(String, u64), String> {
        let data = std::fs::read_to_string(&self.token_path)
            .map_err(|e| format!("Token lesen: {e}"))?;
        let token: TokenStore = serde_json::from_str(&data)
            .map_err(|e| format!("Token parsen: {e}"))?;

        let mut letzter = String::new();
        for versuch in 0..VERSUCHE {
            let resp = self.client
                .post("https://oauth2.googleapis.com/token")
                .form(&[
                    ("refresh_token", token.refresh_token.as_str()),
                    ("client_id", token.client_id.as_str()),
                    ("client_secret", token.client_secret.as_str()),
                    ("grant_type", "refresh_token"),
                ])
                .send()
                .await;

            let resp = match resp {
                Ok(r) => r,
                Err(e) => {                      // gar keine Verbindung
                    letzter = format!("OAuth: {e}");
                    if versuch + 1 < VERSUCHE {
                        tokio::time::sleep(wartezeit(versuch)).await;
                        continue;
                    }
                    break;
                }
            };

            let code = resp.status().as_u16();
            if !resp.status().is_success() && ist_voruebergehend(code, "") && versuch + 1 < VERSUCHE {
                let w = wartezeit(versuch);
                eprintln!("[Drive] OAuth HTTP {code} - Versuch {} von {VERSUCHE} in {}s.",
                          versuch + 2, w.as_secs());
                tokio::time::sleep(w).await;
                continue;
            }

            let body = json_ok(resp, "OAuth").await?;
            let tok = body["access_token"].as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| format!("Kein access_token: {}", body))?;
            let sek = body["expires_in"].as_u64().unwrap_or(3600);
            return Ok((tok, sek));
        }
        Err(letzter)
    }

    /// Sucht einen Eintrag nach Name **im angegebenen Elternordner**.
    /// Ohne parent_id würde nur die Wurzel durchsucht — Dateien in
    /// Unterordnern gälten als nicht vorhanden und würden dupliziert.
    async fn find_in(&self, name: &str, parent_id: &str, folders_only: bool) -> Result<Option<String>, String> {
        let token = self.access_token().await?;
        let kind = if folders_only {
            " and mimeType = 'application/vnd.google-apps.folder'"
        } else {
            ""
        };
        let q = format!("name = '{}' and '{}' in parents and trashed = false{}",
            name.replace('\'', "\\'"), parent_id, kind);

        let resp = self.client
            .get("https://www.googleapis.com/drive/v3/files")
            .bearer_auth(&token)
            .query(&[("q", q.as_str()), ("fields", "files(id,name)")])
            .send()
            .await
            .map_err(|e| format!("List: {e}"))?;

        let body = json_ok(resp, "List").await?;

        Ok(body["files"].as_array()
            .and_then(|a| a.first())
            .and_then(|f| f["id"].as_str())
            .map(|s| s.to_string()))
    }

    /// Gemerkte Ordner-ID (Cache) oder None. Der Mutex wird nur kurz
    /// gehalten, nie ueber ein await - siehe access_token.
    fn ordner_cache_holen(&self, pfad: &str) -> Option<String> {
        self.folder_cache.lock().ok()?.get(pfad).cloned()
    }

    fn ordner_cache_merken(&self, pfad: &str, id: &str) {
        if let Ok(mut c) = self.folder_cache.lock() {
            c.insert(pfad.to_string(), id.to_string());
        }
    }

    /// Ordner-ID zu einem Pfad, ohne etwas anzulegen. Nutzt den Ordner-
    /// Cache: ein bereits aufgeloester Abschnitt kostet keinen API-Aufruf.
    async fn resolve_folder(&self, path: &str) -> Result<Option<String>, String> {
        let mut parent = self.folder_id.clone();
        let mut kumuliert = String::new();
        for part in path.split('/').filter(|p| !p.is_empty()) {
            kumuliert = if kumuliert.is_empty() { part.to_string() }
                        else { format!("{kumuliert}/{part}") };
            if let Some(id) = self.ordner_cache_holen(&kumuliert) {
                parent = id;
                continue;
            }
            match self.find_in(part, &parent, true).await? {
                Some(id) => { self.ordner_cache_merken(&kumuliert, &id); parent = id; }
                None => return Ok(None),
            }
        }
        Ok(Some(parent))
    }

    /// Datei-ID zu einem vollständigen relativen Pfad.
    async fn find_path(&self, path: &str) -> Result<Option<String>, String> {
        let filename = path.rsplit('/').next().unwrap_or(path);
        let parent_path = path.rsplitn(2, '/').nth(1).unwrap_or("");
        match self.resolve_folder(parent_path).await? {
            Some(parent_id) => self.find_in(filename, &parent_id, false).await,
            None => Ok(None),
        }
    }

    async fn find_or_create_folder(&self, name: &str, parent_id: &str) -> Result<String, String> {
        if let Some(id) = self.find_in(name, parent_id, true).await? {
            return Ok(id);
        }
        let token = self.access_token().await?;

        let meta = serde_json::json!({
            "name": name,
            "mimeType": "application/vnd.google-apps.folder",
            "parents": [parent_id],
        });

        let resp = self.client
            .post("https://www.googleapis.com/drive/v3/files")
            .bearer_auth(&token)
            .header("Content-Type", "application/json")
            .body(meta.to_string())
            .send()
            .await
            .map_err(|e| format!("Create folder: {e}"))?;

        let body = json_ok(resp, "Create folder").await?;

        body["id"].as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| format!("Folder create failed: {}", body))
    }

    /// Upload; `pruefen` = vorher nach vorhandener Datei suchen (PATCH statt
    /// neu anlegen). false nur, wenn die Ziel-Liste sicher sagt: noch nicht da.
    async fn hochladen(&self, path: &str, data: Vec<u8>, pruefen: bool) -> Result<(), String> {
        let filename = path.rsplit('/').next().unwrap_or(path).to_string();
        let parent_path = path.rsplitn(2, '/').nth(1).unwrap_or("");
        let parent_id = if parent_path.is_empty() {
            self.folder_id.clone()
        } else {
            self.ensure_folder(parent_path).await?
        };

        // Neue Datei laut Ziel-Liste: keine Suche (Ben, 19.10.2026).
        let existing = if pruefen {
            self.find_in(&filename, &parent_id, false).await?
        } else {
            None
        };
        let mut letzter = String::new();

        for versuch in 0..VERSUCHE {
            let token = self.access_token().await?;

            let antwort = if let Some(file_id) = &existing {
                let url = format!(
                    "https://www.googleapis.com/upload/drive/v3/files/{}?uploadType=media",
                    file_id);
                self.client.patch(&url).bearer_auth(&token)
                    .body(data.clone()).send().await
            } else {
                let url = "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart";
                let metadata = serde_json::json!({ "name": filename, "parents": [parent_id] });
                let part_meta = reqwest::multipart::Part::text(metadata.to_string())
                    .mime_str("application/json").map_err(|e| e.to_string())?;
                let part_file = reqwest::multipart::Part::bytes(data.clone())
                    .mime_str("application/octet-stream").map_err(|e| e.to_string())?;
                let form = reqwest::multipart::Form::new()
                    .part("metadata", part_meta)
                    .part("media", part_file);
                self.client.post(url).bearer_auth(&token).multipart(form).send().await
            };

            match antwort {
                Ok(resp) if resp.status().is_success() => return Ok(()),
                Ok(resp) => {
                    let code = resp.status().as_u16();
                    let text = resp.text().await.unwrap_or_default();
                    letzter = format!("Upload {path}: HTTP {code} — {}",
                                      text.chars().take(200).collect::<String>());
                    if !ist_voruebergehend(code, &text) {
                        return Err(letzter);       // 401/404, echtes 403: zweiter Versuch bringt nichts
                    }
                }
                Err(e) => letzter = format!("Upload {path}: {e}"),
            }

            if versuch + 1 < VERSUCHE {
                let w = wartezeit(versuch);
                eprintln!("[Drive] {letzter} - Versuch {} von {VERSUCHE} in {}s.",
                          versuch + 2, w.as_secs());
                tokio::time::sleep(w).await;
            }
        }
        Err(letzter)
    }

    async fn ensure_folder(&self, path: &str) -> Result<String, String> {
        let _einzeln = self.ordner_sperre.lock().await;
        let mut parent_id = self.folder_id.clone();
        let mut kumuliert = String::new();
        for part in path.split('/').filter(|p| !p.is_empty()) {
            kumuliert = if kumuliert.is_empty() { part.to_string() }
                        else { format!("{kumuliert}/{part}") };
            if let Some(id) = self.ordner_cache_holen(&kumuliert) {
                parent_id = id;
                continue;
            }
            parent_id = self.find_or_create_folder(part, &parent_id).await?;
            self.ordner_cache_merken(&kumuliert, &parent_id);
        }
        Ok(parent_id)
    }
}

#[async_trait]
impl Destination for DriveDest {
    /// Ben, 05.08.2026 - zwei Dinge geradegezogen:
    ///
    /// 1. Der PATCH-Zweig (Datei existiert schon) prüfte den HTTP-Status
    ///    GAR NICHT. Ein abgelehnter Upload — 403, 503, Kontingent voll —
    ///    galt als erfolgreich, die Datei fehlte still im Backup, und die
    ///    .hash daneben behauptete, sie sei aktuell. Stiller Datenverlust.
    /// 2. Kein zweiter Versuch. Bei 1.800 Dateien ist ein einzelner
    ///    Aussetzer fast sicher; er riss den ganzen Lauf ab.
    ///
    /// Der Körper wird je Versuch neu aufgebaut: ein verbrauchter Request
    /// lässt sich nicht wiederholen.
    async fn write(&self, path: &str, data: Vec<u8>) -> Result<(), String> {
        self.hochladen(path, data, true).await
    }

    async fn write_neu(&self, path: &str, data: Vec<u8>) -> Result<(), String> {
        self.hochladen(path, data, false).await
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        let file_id = self.find_path(path).await?
            .ok_or_else(|| format!("Datei nicht gefunden: {path}"))?;

        let token = self.access_token().await?;
        let resp = self.client
            .get(format!("https://www.googleapis.com/drive/v3/files/{file_id}?alt=media"))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| format!("Download: {e}"))?;

        resp.bytes().await
            .map(|b| b.to_vec())
            .map_err(|e| format!("Download bytes: {e}"))
    }

    async fn delete(&self, path: &str) -> Result<(), String> {
        if let Some(file_id) = self.find_path(path).await? {
            let token = self.access_token().await?;
            self.client
                .delete(format!("https://www.googleapis.com/drive/v3/files/{file_id}"))
                .bearer_auth(&token)
                .send()
                .await
                .map_err(|e| format!("Delete: {e}"))?;
        }
        Ok(())
    }

    /// Rekursiv über den ganzen Backup-Baum — Drive kennt keine rekursive
    /// Abfrage, also Ordner für Ordner. Ohne das sieht protect() nur die
    /// oberste Ebene und der Papierkorb greift nie.
    async fn list(&self, prefix: &str) -> Result<Vec<String>, String> {
        let token = self.access_token().await?;
        let mut names = Vec::new();

        // Ben, 05.08.2026: 'prefix' wurde bisher ignoriert - die Liste kam
        // IMMER vom Wurzelordner. Solange es nur einen Bereich gab, fiel das
        // nicht auf. Mit Spiegel/ und Archiv/ nebeneinander waere es
        // gefaehrlich: der Spiegel-Durchlauf saehe die Archiv-Dateien als
        // "nicht mehr in der Quelle" und wuerde sie loeschen.
        let start = prefix.trim_matches('/');
        let start_id = if start.is_empty() {
            self.folder_id.clone()
        } else {
            match self.resolve_folder(start).await? {
                Some(id) => id,
                None => return Ok(vec![]),   // Bereich existiert noch nicht
            }
        };
        // Wie bei LocalDest sind die zurueckgegebenen Pfade relativ zur
        // WURZEL, tragen den Praefix also mit.
        let mut queue = vec![(start_id, start.to_string())];

        while let Some((folder_id, base)) = queue.pop() {
            let q = format!("'{}' in parents and trashed = false", folder_id);
            let mut page = String::new();
            loop {
                let mut params = vec![
                    ("q", q.as_str()),
                    ("fields", "nextPageToken, files(id,name,mimeType)"),
                    ("pageSize", "1000"),
                ];
                if !page.is_empty() {
                    params.push(("pageToken", page.as_str()));
                }

                let resp = self.client
                    .get("https://www.googleapis.com/drive/v3/files")
                    .bearer_auth(&token)
                    .query(&params)
                    .send()
                    .await
                    .map_err(|e| format!("List: {e}"))?;

                let body = json_ok(resp, "List").await?;

                for f in body["files"].as_array().cloned().unwrap_or_default() {
                    let (id, name) = match (f["id"].as_str(), f["name"].as_str()) {
                        (Some(i), Some(n)) => (i.to_string(), n.to_string()),
                        _ => continue,
                    };
                    let path = if base.is_empty() { name } else { format!("{base}/{name}") };
                    if f["mimeType"].as_str() == Some("application/vnd.google-apps.folder") {
                        queue.push((id, path));
                    } else {
                        names.push(path);
                    }
                }

                match body["nextPageToken"].as_str() {
                    Some(t) => page = t.to_string(),
                    None => break,
                }
            }
        }

        names.retain(|n| n.starts_with(prefix));
        Ok(names)
    }
}

pub fn list_drive_folders() -> Result<Vec<(String, String)>, String> {
    let token_path = crate::drive_auth::token_path();
    if !token_path.exists() {
        return Err("Nicht angemeldet".into());
    }
    let data = std::fs::read_to_string(&token_path).map_err(|e| e.to_string())?;
    let ts: TokenStore = serde_json::from_str(&data).map_err(|e| e.to_string())?;

    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async {
        if ts.client_id.is_empty() || ts.client_secret.is_empty() {
            return Err("Token veraltet — bitte erneut mit Google anmelden".into());
        }
        let client = reqwest::Client::new();

        let resp = client
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("refresh_token", ts.refresh_token.as_str()),
                ("client_id", ts.client_id.as_str()),
                ("client_secret", ts.client_secret.as_str()),
                ("grant_type", "refresh_token"),
            ])
            .send().await.map_err(|e| e.to_string())?;
        let body = json_ok(resp, "OAuth").await?;
        let token = body["access_token"].as_str().ok_or("Kein Token")?.to_string();

        // Serverseitig filtern: nur eigene Ordner auf oberster Ebene von
        // "Meine Ablage". Ohne 'root' in parents kommen alle Unterordner aller
        // Ebenen plus geteilte/App-Ordner (kryptische Hex-Namen) mit.
        let mut folders = Vec::new();
        let mut page_token = String::new();
        loop {
            let mut q = vec![
                ("q", "mimeType = 'application/vnd.google-apps.folder' \
                       and trashed = false and 'root' in parents and 'me' in owners"),
                ("fields", "nextPageToken, files(id,name)"),
                ("pageSize", "1000"),
                ("orderBy", "name"),
            ];
            if !page_token.is_empty() {
                q.push(("pageToken", page_token.as_str()));
            }

            let resp = client
                .get("https://www.googleapis.com/drive/v3/files")
                .bearer_auth(&token)
                .query(&q)
                .send().await.map_err(|e| e.to_string())?;
            let body = json_ok(resp, "Ordnerliste").await?;

            for f in body["files"].as_array().cloned().unwrap_or_default() {
                if let (Some(id), Some(name)) = (f["id"].as_str(), f["name"].as_str()) {
                    folders.push((id.to_string(), name.to_string()));
                }
            }

            match body["nextPageToken"].as_str() {
                Some(t) => page_token = t.to_string(),
                None => break,
            }
        }

        Ok(folders)
    })
}
