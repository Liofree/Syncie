use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use crate::dest::Destination;
use crate::scanner;
use futures_util::StreamExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TrashEntry {
    pub(crate) deleted_at: chrono::DateTime<Utc>,
    pub(crate) size: u64,
}

/// Dateinamen des Papierkorbs - an vier Stellen gebraucht (protect,
/// zurueckholen, spaeter, Ausblenden beim Spiegeln), deshalb Konstanten.
pub(crate) const TRASH_INDEX: &str = ".syncie_trash.json";
pub(crate) const TRASH_DIR: &str = ".syncie_trash";

/// Eine Datei, die bald endgueltig aus dem Papierkorb verschwindet.
/// Wandert als JSON nach papierkorb.json und von dort ins LUNA-HUD.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PapierkorbWarnung {
    /// Pfad im Backup, inkl. Quell-Ordnername als oberste Ebene,
    /// z.B. "LUNA/data/foo.txt" - genau der Schluessel im Trash-Index.
    pub pfad: String,
    /// Tage bis zur endgueltigen Loeschung (0 = beim naechsten Lauf weg).
    pub tage: i64,
    pub groesse: u64,
}

/// Trash-Index lesen. Fehlt er oder ist er kaputt, ist der Papierkorb leer -
/// das ist kein Fehler, sondern der Normalfall beim ersten Lauf.
pub(crate) async fn trash_lesen(dest: &dyn Destination) -> HashMap<String, TrashEntry> {
    let daten = dest.read(TRASH_INDEX).await.unwrap_or_else(|_| b"{}".to_vec());
    serde_json::from_str(&String::from_utf8_lossy(&daten)).unwrap_or_default()
}

pub(crate) async fn trash_schreiben(
    dest: &dyn Destination,
    trash: &HashMap<String, TrashEntry>,
) -> Result<(), String> {
    let json = serde_json::to_string_pretty(trash).map_err(|e| e.to_string())?;
    dest.write(TRASH_INDEX, json.into_bytes()).await
}

/// Der Kopf, den trash_payload vor die Nutzdaten setzt. Beim Zurueckholen
/// muss er wieder ab - sonst laege eine beschaedigte Datei im Quellordner.
pub(crate) fn nutzdaten_ohne_kopf(daten: &[u8]) -> &[u8] {
    if !daten.starts_with(b"SYNCIE_TRASH\n") {
        return daten;   // Altbestand ohne Kopf
    }
    // Kopf ist "SYNCIE_TRASH\n<zeitstempel>\n" - also bis zum ZWEITEN \n.
    match daten.iter().enumerate().filter(|(_, b)| **b == b'\n').nth(1) {
        Some((i, _)) => &daten[i + 1..],
        None => daten,
    }
}

/// Wie viele Tage vor der endgueltigen Loeschung aus dem Papierkorb gewarnt
/// wird (Ben, 05.08.2026). Dateien, die innerhalb dieser Frist faellig
/// werden, meldet protect() als Warnung zurueck.
const PAPIERKORB_VORWARN_TAGE: i64 = 7;

/// Gleichzeitige Uploads. Drive drosselt ab einigen Schreibvorgaengen je
/// Sekunde mit 403/429 - die fangen die Wiederholungen in drive.rs ab.
const PARALLEL: usize = 8;
/// Ab dieser Groesse laeuft eine Datei allein (ganze Datei liegt im Speicher).
const GROSS: u64 = 256 << 20;
/// Alle so viele hochgeladenen Dateien wird der Zustand gespeichert.
const ZWISCHENSTAND: usize = 500;

pub async fn protect(
    source_paths: &[PathBuf],
    dest: &dyn Destination,
    retention_months: u32,
    exclude: &[String],
    einweg: &[String],
    cancel: &AtomicBool,
    fort: &crate::fortschritt::Fortschritt,
    zustand: &mut crate::zustand::Zustand,
) -> Result<(usize, Vec<PapierkorbWarnung>, Vec<String>), String> {
    fort.melden(crate::BEREICH, 0, 0);
    let src_files = scanner::scan(source_paths, exclude);
    let now = Utc::now();
    let cutoff = now - chrono::Duration::days(retention_months as i64 * 30);

    // Load trash index
    let mut trash = trash_lesen(dest).await;

    let mut src_set: HashMap<String, &scanner::ScannedFile> = HashMap::new();
    for f in &src_files {
        src_set.insert(f.relative_path.clone(), f);
    }

    // Was im Ziel liegt, EINMAL abfragen: Schritt 1 braucht es, um eine
    // im Ziel fehlende Datei trotz unveraendertem Zustand nachzureichen,
    // Schritt 2, um Geloeschtes zu finden. Bei Drive ist jede Abfrage Netz.
    // Ist die Liste nicht zu haben (Netzfehler), gilt keine Datei als sicher
    // neu: write() sucht dann selbst - sonst legte Drive Doppelte an.
    let dest_liste = dest.list("").await;
    let liste_ok = dest_liste.is_ok();
    let dest_files = dest_liste.unwrap_or_default();
    let vorhanden: std::collections::HashSet<&String> = dest_files.iter().collect();

    // Schritt 1: neue und geaenderte Dateien hochladen.
    // Groesse und Aenderungszeit statt Hash: das erspart es, jede Datei bei
    // jedem Lauf komplett zu lesen. Der Blick ins Ziel deckt den Fall ab,
    // dass dort jemand aufgeraeumt hat.
    let zu_tun: Vec<&scanner::ScannedFile> = src_files.iter()
        .filter(|f| !zustand.unveraendert(f) || !vorhanden.contains(&f.relative_path))
        .collect();

    // Parallel (Ben, 19.10.2026): einzeln dauerte der Erstlauf mit 35.000
    // neuen Dateien ueber 12 h. Eine grosse Datei belegt alle Plaetze, damit
    // nicht PARALLEL grosse Dateien zugleich im Speicher liegen.
    let plaetze = tokio::sync::Semaphore::new(PARALLEL);
    let mut laeufe = futures_util::stream::iter(zu_tun.iter().map(|f| {
        let plaetze = &plaetze;
        let neu = liste_ok && !vorhanden.contains(&f.relative_path);
        async move {
            let anteil = if f.size > GROSS { PARALLEL as u32 } else { 1 };
            let _platz = plaetze.acquire_many(anteil).await.map_err(|e| e.to_string())?;
            // Eine gesperrte Datei (Datenbank offen, Programm laeuft) darf den
            // Lauf NICHT abreissen - sonst kostet eine einzige Datei alle
            // uebrigen. Sie wird uebersprungen, gemeldet und beim naechsten
            // Lauf erneut versucht: in den Zustand kommt sie nicht, und in der
            // Quelle steht sie weiterhin, also fasst sie auch Schritt 2 nicht an.
            match lesen_mit_geduld(&f.full_path).await {
                Ok(data) => {
                    if neu { dest.write_neu(&f.relative_path, data).await?; }
                    else { dest.write(&f.relative_path, data).await?; }
                    Ok::<_, String>((*f, None))
                }
                Err(e) => Ok((*f, Some(format!("{}: {e}", f.relative_path)))),
            }
        }
    })).buffer_unordered(PARALLEL);

    let mut copied = 0usize;
    let mut erledigt = 0usize;
    let mut uebersprungen: Vec<String> = Vec::new();
    while let Some(erg) = laeufe.next().await {
        let (f, fehler) = erg?;
        erledigt += 1;
        fort.melden(crate::BEREICH, erledigt, zu_tun.len());
        match fehler {
            None => {
                zustand.merken(f);
                copied += 1;
                // Zwischenstand sichern (Ben, 19.10.2026): stirbt der Lauf
                // (Absturz, Rechner aus), muessen die schon hochgeladenen
                // Dateien beim naechsten Mal nicht noch einmal hoch.
                if copied % ZWISCHENSTAND == 0 { zustand.speichern(); }
            }
            Some(e) => uebersprungen.push(e),
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(format!("Abgebrochen ({copied} Dateien gesichert)"));
        }
    }
    drop(laeufe);

    for f in &src_files {
        // Datei ist (wieder) da: die Kopie im Papierkorb hat sich erledigt.
        // Frueher fiel nur der Index-Eintrag weg und die Kopie blieb fuer
        // immer liegen - die Aufbewahrungsfrist erreichte sie nie mehr.
        if trash.remove(&f.relative_path).is_some() {
            let _ = dest.delete(&format!("{TRASH_DIR}/{}", f.relative_path)).await;
        }
    }

    // Schritt 2: was im Ziel liegt und in der Quelle fehlt, in den Papierkorb.
    for dpath in &dest_files {
        if cancel.load(Ordering::Relaxed) {
            return Err(format!("Abgebrochen ({copied} Dateien gesichert)"));
        }
        if dpath == TRASH_INDEX { continue; }
        if dpath.ends_with(".hash") {
            // Altbestand: bis 23.08.2026 lag neben jeder Datei ihr Hash im
            // Ziel. Der Vergleich laeuft jetzt lokal - weg damit, einmalig.
            let _ = dest.delete(dpath).await;
            continue;
        }
        // Einweg-Ordner: lokal Geloeschtes bleibt im Ziel (Ben, 27.11.2026).
        if einweg.iter().any(|e| dpath.starts_with(&format!("{e}/"))) { continue; }
        if !src_set.contains_key(dpath) && !dpath.starts_with(&format!("{TRASH_DIR}/")) {
            // Move to trash
            if let Ok(data) = dest.read(dpath).await {
                let trash_path = format!("{TRASH_DIR}/{dpath}");
                if trash_path.rsplitn(2, '/').nth(1).is_some() {
                    // Write to trash
                    let payload = trash_payload(&data, now);
                    dest.write(&trash_path, payload).await?;
                }
                dest.delete(dpath).await?;
                // Aus dem Zustand nehmen: taucht die Datei spaeter wieder
                // auf, muss sie neu hochgeladen werden - sonst gaelte sie
                // als unveraendert und fehlte im Ziel.
                zustand.vergessen(dpath);
                trash.insert(dpath.clone(), TrashEntry { deleted_at: now, size: data.len() as u64 });
            }
        }
    }

    // Schritt 3: Papierkorb aufraeumen, older than retention_months. Dabei zugleich die
    // Vorwarnung sammeln: Dateien, die noch NICHT faellig sind, aber
    // innerhalb der naechsten PAPIERKORB_VORWARN_TAGE endgueltig geloescht
    // werden. So bekommt Ben eine Chance, sie vorher aus dem Drive-
    // Papierkorb (.syncie_trash/) herauszuholen (Ben, 05.08.2026).
    let aufbewahrung = chrono::Duration::days(retention_months as i64 * 30);
    let vorwarn_grenze = cutoff + chrono::Duration::days(PAPIERKORB_VORWARN_TAGE);
    let mut to_remove: Vec<String> = Vec::new();
    let mut warnungen: Vec<PapierkorbWarnung> = Vec::new();
    for (name, entry) in &trash {
        if entry.deleted_at < cutoff {
            to_remove.push(name.clone());
        } else if entry.deleted_at < vorwarn_grenze {
            let faellig = entry.deleted_at + aufbewahrung;
            warnungen.push(PapierkorbWarnung {
                pfad: name.clone(),
                tage: (faellig - now).num_days().max(0),
                groesse: entry.size,
            });
        }
    }
    // Dringendstes zuerst - so steht im HUD oben, was zuerst wegfaellt.
    warnungen.sort_by_key(|w| w.tage);
    for name in &to_remove {
        let trash_path = format!("{TRASH_DIR}/{name}");
        let _ = dest.delete(&trash_path).await;
        trash.remove(name);
    }

    trash_schreiben(dest, &trash).await?;

    Ok((copied, warnungen, uebersprungen))
}

/// Liest die Datei, mit zwei Nachschlaegen.
///
/// Windows meldet Fehler 32/33, solange ein anderes Programm die Datei
/// offen haelt - LUNA laeuft beim Sichern immer, und seine SQLite-Dateien
/// sind waehrend eines Schreibvorgangs kurz gesperrt. Diese Sperren sind
/// nach Sekundenbruchteilen weg; ohne Nachschlag kostet so ein Zufall die
/// Datei fuer den ganzen Lauf (Ben, 23.08.2026).
async fn lesen_mit_geduld(pfad: &std::path::Path) -> std::io::Result<Vec<u8>> {
    const ANLAEUFE: u32 = 3;
    for versuch in 0..ANLAEUFE {
        // tokio::fs: die parallelen Uploads laufen in EINEM Task - ein
        // blockierendes Lesen hielte alle anderen an.
        match tokio::fs::read(pfad).await {
            Ok(daten) => return Ok(daten),
            Err(e) if versuch + 1 == ANLAEUFE => return Err(e),
            Err(_) => {
                tokio::time::sleep(std::time::Duration::from_millis(250 << versuch)).await;
            }
        }
    }
    unreachable!("die Schleife kehrt im letzten Durchgang zurueck")
}

fn trash_payload(data: &[u8], deleted_at: chrono::DateTime<Utc>) -> Vec<u8> {
    let ts = deleted_at.timestamp().to_string();
    let header = format!("SYNCIE_TRASH\n{ts}\n");
    let mut payload = header.into_bytes();
    payload.extend_from_slice(data);
    payload
}
