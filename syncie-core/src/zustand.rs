//! Was beim letzten Lauf schon im Ziel lag: Groesse und Aenderungszeit je
//! Datei, lokal neben profiles.json.
//!
//! Vorher wurde bei JEDEM Lauf jede Quelldatei komplett gelesen und
//! gehasht, nur um festzustellen, dass sie unveraendert ist - und dazu je
//! Datei eine .hash-Datei aus dem Ziel geholt, ueber Netz. Groesse und
//! Zeitstempel liefert das Dateisystem beim Auflisten frei mit.
//!
//! Fehlt die Datei (Neuinstallation, geloescht), gilt alles als geaendert:
//! ein teurer, aber richtiger Lauf ist besser als ein billiger, der eine
//! Aenderung verschluckt.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use crate::scanner::ScannedFile;

/// Groesse und Aenderungszeit (Unix-Sekunden) - zusammen aussagekraeftig
/// genug: eine geaenderte Datei behaelt praktisch nie beides.
type Stand = HashMap<String, (u64, u64)>;

pub struct Zustand {
    pfad: PathBuf,
    stand: Stand,
}

/// Ein Profilname darf Leerzeichen und Umlaute haben, ein Dateiname
/// besser nicht - deshalb alles Fremde auf '_'.
fn dateiname(profil: &str) -> String {
    let sauber: String = profil
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("zustand-{sauber}.json")
}

fn zeit(f: &ScannedFile) -> u64 {
    f.modified.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Zustand {
    pub fn laden(profil: &str) -> Self {
        let pfad = crate::profile::profiles_path().with_file_name(dateiname(profil));
        let stand = std::fs::read_to_string(&pfad)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { pfad, stand }
    }

    pub fn unveraendert(&self, f: &ScannedFile) -> bool {
        self.stand.get(&f.relative_path) == Some(&(f.size, zeit(f)))
    }

    pub fn merken(&mut self, f: &ScannedFile) {
        self.stand.insert(f.relative_path.clone(), (f.size, zeit(f)));
    }

    pub fn vergessen(&mut self, pfad: &str) {
        self.stand.remove(pfad);
    }

    /// Fehler werden geschluckt: ein nicht schreibbarer Zustand kostet den
    /// naechsten Lauf Zeit, aber er darf kein gelungenes Backup kippen.
    pub fn speichern(&self) {
        if let Some(ordner) = self.pfad.parent() {
            let _ = std::fs::create_dir_all(ordner);
        }
        if let Ok(txt) = serde_json::to_string(&self.stand) {
            let _ = std::fs::write(&self.pfad, txt);
        }
    }
}
