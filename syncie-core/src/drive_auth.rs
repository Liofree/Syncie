use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::Duration;

pub struct AuthResult {
    pub access_token: String,
    pub refresh_token: String,
}

pub fn start_auth(client_id: &str, client_secret: &str) -> Result<AuthResult, String> {
    // Einmal binden, kein Drop — vermeidet Port-Race zwischen std und tokio
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Kein Port verfügbar: {e}"))?;
    let port = listener.local_addr().map_err(|e| format!("Port: {e}"))?.port();

    // 127.0.0.1 statt localhost → zwingt IPv4, keine ::1-Probleme
    let redirect_uri = format!("http://127.0.0.1:{port}");

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?\
        client_id={}&redirect_uri={}&response_type=code&\
        scope=https://www.googleapis.com/auth/drive&\
        access_type=offline&prompt=consent",
        client_id, redirect_uri
    );

    webbrowser::open(&auth_url).map_err(|e| format!("Browser öffnen: {e}"))?;

    // Socket offen lassen, auf Verbindung warten — kein tokio nötig
    let code = wait_for_code(&listener)?;
    drop(listener); // erst nach code-empfang schließen

    let cid = client_id.to_string();
    let cs = client_secret.to_string();

    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async move {
        let client = reqwest::Client::new();
        let resp = client
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("code", code),
                ("client_id", cid),
                ("client_secret", cs),
                ("redirect_uri", redirect_uri),
                ("grant_type", "authorization_code".to_string()),
            ])
            .send()
            .await
            .map_err(|e| format!("Token: {e}"))?;

        let body: serde_json::Value = resp.json().await.map_err(|e| format!("JSON: {e}"))?;
        let access_token = body["access_token"].as_str()
            .ok_or_else(|| format!("Kein access_token: {body}"))?
            .to_string();
        let refresh_token = body["refresh_token"].as_str()
            .ok_or_else(|| format!("Kein refresh_token: {body}"))?
            .to_string();

        Ok(AuthResult { access_token, refresh_token })
    })
}

fn wait_for_code(listener: &TcpListener) -> Result<String, String> {
    let start = std::time::Instant::now();

    // Blocking accept in einem Thread, main loop pollt mit Timeout
    let (tx, rx) = std::sync::mpsc::channel();
    let listener_clone = listener.try_clone().map_err(|e| format!("Clone: {e}"))?;

    std::thread::spawn(move || {
        loop {
            match listener_clone.accept() {
                Ok((mut stream, _)) => {
                    stream.set_read_timeout(Some(Duration::from_secs(3))).ok();
                    let mut buf = [0u8; 8192];
                    if let Ok(n) = stream.read(&mut buf) {
                        let req = String::from_utf8_lossy(&buf[..n]);
                        // Parse code aus Query-Parametern (egal an welcher Position)
                        let code = req.lines()
                            .next()
                            .and_then(|line| line.split_whitespace().nth(1))
                            .and_then(|path| path.split('?').nth(1))
                            .and_then(|qs| qs.split('&')
                                .find_map(|p| p.strip_prefix("code="))
                                .map(|c| c.to_string())
                            );

                        if let Some(c) = code {
                            // Erfolg — sende HTML-Seite zurück
                            let response = b"HTTP/1.1 200 OK\r\n\
                                Content-Type: text/html; charset=utf-8\r\n\
                                Connection: close\r\n\r\n\
                                <html><body style='font-family:sans-serif;text-align:center;padding:60px 0'>\
                                <h1 style='color:#4caf50'>Syncie</h1>\
                                <p>Angemeldet. Fenster schliessen.</p></body></html>";
                            stream.write_all(response).ok();
                            stream.flush().ok();
                            let _ = tx.send(Ok(c));
                            return;
                        }
                    }
                    // Keine code-Parameter → Verbindung schließen, weiterhören
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                }
                Err(_) => break,
            }
        }
    });

    loop {
        if start.elapsed().as_secs() > 120 {
            return Err(crate::sprache::t("Zeitüberschreitung (2 Minuten)").into());
        }
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(Ok(code)) => return Ok(code),
            Ok(Err(e)) => return Err(e),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => return Err("Fehler beim Warten".into()),
        }
    }
}

pub fn is_authenticated() -> bool {
    let p = token_path();
    if !p.exists() { return false; }
    std::fs::read_to_string(&p).ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .map(|v| v.get("client_id").and_then(|c| c.as_str()).unwrap_or("") != "")
        .unwrap_or(false)
}

pub fn token_path() -> PathBuf {
    std::env::var("APPDATA")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("syncie")
        .join("drive_token.json")
}

pub fn save_token(auth: &AuthResult, client_id: &str, client_secret: &str) -> Result<(), String> {
    let p = token_path();
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    let json = serde_json::json!({
        "access_token": auth.access_token,
        "refresh_token": auth.refresh_token,
        "client_id": client_id,
        "client_secret": client_secret,
    });
    std::fs::write(&p, json.to_string()).map_err(|e| e.to_string())
}
