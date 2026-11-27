//! Wo der laufende Sicherungslauf gerade steht.
//!
//! Als Datei neben profiles.json, weil die Anzeigen in FREMDEN Prozessen
//! sitzen: das Syncie-Fenster liest sie aus seinem eigenen Lauf, LUNA aus
//! dem Lauf von syncie-cli.exe. Gemeinsamen Speicher gibt es da nicht.
//! Gleiches Muster wie papierkorb.json - kein zweiter Mechanismus.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Nicht bei jeder Datei schreiben: das waeren hunderte Schreibvorgaenge
/// je Sekunde fuer eine Anzeige, die niemand so schnell liest.
const TAKT: Duration = Duration::from_millis(400);

/// Aelter als das hier = Leiche eines abgestuerzten Laufs, nicht "laeuft".
pub const VERFALL_SEKUNDEN: u64 = 120;

pub fn pfad() -> std::path::PathBuf {
    crate::profile::profiles_path().with_file_name("fortschritt.json")
}

/// `gesamt` 0 heisst: Zahl noch unbekannt, der Scan laeuft noch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lage {
    pub profil: String,
    pub phase: String,
    pub getan: usize,
    pub gesamt: usize,
    /// Unix-Sekunden der letzten Meldung.
    pub zeit: u64,
}

pub struct Fortschritt {
    profil: String,
    zuletzt: Mutex<Instant>,
}

impl Fortschritt {
    pub fn neu(profil: &str) -> Self {
        Self { profil: profil.to_string(), zuletzt: Mutex::new(Instant::now() - TAKT) }
    }

    pub fn melden(&self, phase: &str, getan: usize, gesamt: usize) {
        {
            // getan == 0 ist der Phasenwechsel - der geht immer durch,
            // sonst stuende die alte Phase noch bis zu 400 ms da.
            let mut zuletzt = match self.zuletzt.lock() { Ok(z) => z, Err(e) => e.into_inner() };
            if getan != 0 && zuletzt.elapsed() < TAKT {
                return;
            }
            *zuletzt = Instant::now();
        }
        let lage = Lage {
            profil: self.profil.clone(),
            phase: phase.to_string(),
            getan,
            gesamt,
            zeit: SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()),
        };
        let pfad = pfad();
        if let Some(ordner) = pfad.parent() {
            let _ = std::fs::create_dir_all(ordner);
        }
        if let Ok(txt) = serde_json::to_string(&lage) {
            let _ = std::fs::write(&pfad, txt);
        }
    }
}

/// Loeschen als Drop: so raeumt auch der Abbruch- und der Fehlerweg auf,
/// ohne dass an drei Stellen ein Aufruf stehen muss.
impl Drop for Fortschritt {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(pfad());
    }
}

/// Fuer die Anzeigen. `None`, wenn nichts laeuft oder die Datei veraltet ist.
pub fn lesen() -> Option<Lage> {
    let lage: Lage = std::fs::read_to_string(pfad())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())?;
    let jetzt = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    (jetzt.saturating_sub(lage.zeit) < VERFALL_SEKUNDEN).then_some(lage)
}
