use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub source_paths: Vec<PathBuf>,
    pub dest_path: PathBuf,
    pub retention_months: u32,
    pub schedule: Schedule,
    pub drive_enabled: bool,
    pub drive_folder_id: String,
    pub drive_client_id: String,
    pub drive_client_secret: String,
    /// Quell-Ordner mit Einweg-Sicherung (Ben, 27.11.2026): was dort lokal
    /// geloescht wird, bleibt im Ziel liegen und wandert NICHT in den
    /// Papierkorb. Leer = alles wird gespiegelt wie bisher; fehlt das Feld in
    /// einer alten profiles.json, gilt ebenfalls leer.
    #[serde(default)]
    pub einweg: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub manual_only: bool,
    pub mon: bool,
    pub tue: bool,
    pub wed: bool,
    pub thu: bool,
    pub fri: bool,
    pub sat: bool,
    pub sun: bool,
    pub hour: u8,
    pub minute: u8,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            manual_only: true,
            mon: true, tue: true, wed: true, thu: true, fri: true,
            sat: false, sun: false,
            hour: 3, minute: 0,
        }
    }
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "Neues Profil".into(),
            source_paths: vec![],
            dest_path: PathBuf::new(),
            retention_months: 6,
            schedule: Schedule::default(),
            drive_enabled: false,
            drive_folder_id: String::new(),
            drive_client_id: String::new(),
            drive_client_secret: String::new(),
            einweg: vec![],
        }
    }
}

impl Profile {
    pub fn base_dest_dir(&self) -> PathBuf {
        self.dest_path.join(&self.name)
    }

    pub fn exclude_patterns(&self) -> Vec<String> {
        vec![
            ".git".into(), "node_modules".into(), "target".into(),
            "$RECYCLE.BIN".into(), "System Volume Information".into(),
            // Aus dem Quellcode jederzeit neu erzeugbar - sie aendern sich
            // bei jedem Lauf und waeren sonst der groesste Posten im
            // Papierkorb (Ben, 23.08.2026).
            "__pycache__".into(), ".mypy_cache".into(), ".ruff_cache".into(),
            // Der Shared-Memory-Spiegel einer offenen SQLite-Datei: aendert
            // sich staendig, ist ausserhalb des laufenden Programms wertlos
            // und ist genau die Datei, die beim Lesen gesperrt ist.
            "*.db-shm".into(),
            ".pytest_cache".into(), "venv".into(), ".venv".into(),
            // Zugangsdaten: jederzeit neu beschaffbar, gehoeren nicht
            // unverschluesselt ins Drive. Das '*' fasst auch
            // spotify_credentials.json / spotify_token.json mit.
            "*credentials.json".into(), "*token.json".into(),
            // Browser-Cache (LUNA data/chrome-ohr, data/webview): aendert sich
            // staendig, ist beim Lesen oft gesperrt (Ben, 19.10.2026).
            "chrome-ohr".into(), "webview".into(),
        ]
    }

    /// Läuft das Profil an diesem Wochentag nach Zeitplan?
    pub fn runs_on(&self, weekday: chrono::Weekday) -> bool {
        use chrono::Weekday::*;
        if self.schedule.manual_only {
            return false;
        }
        match weekday {
            Mon => self.schedule.mon,
            Tue => self.schedule.tue,
            Wed => self.schedule.wed,
            Thu => self.schedule.thu,
            Fri => self.schedule.fri,
            Sat => self.schedule.sat,
            Sun => self.schedule.sun,
        }
    }
}

/// Ablageort der Profile — von GUI und CLI gemeinsam genutzt.
pub fn profiles_path() -> PathBuf {
    std::env::var("APPDATA")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("syncie")
        .join("profiles.json")
}

pub fn load_all() -> Vec<Profile> {
    std::fs::read_to_string(profiles_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
