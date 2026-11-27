pub mod profile;
pub mod scanner;
pub mod protect;
pub mod retention;
pub mod schedule;
pub mod dest;
pub mod fortschritt;
pub mod zustand;
pub mod drive_auth;
pub mod papierkorb;
pub mod verlauf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    General(String),
}

/// Name des Ordners im Ziel. Als Konstante, weil ein Tippfehler ein
/// zweites, stilles Backup anlegen wuerde statt einen Fehler zu werfen.
///
/// Ben, 23.08.2026: frueher zwei Bereiche - "Spiegel" 1:1 und "Archiv"
/// mit Papierkorb. Beide enthielten DIESELBEN Dateien; das Archiv war der
/// Spiegel plus Papierkorb, der Spiegel also doppelte Laufzeit, doppelter
/// Platz und doppelter Upload ohne eigenen Nutzen. Geblieben ist einer:
/// ein 1:1-Spiegel, dessen Geloeschtes `retention_months` lang im
/// Papierkorb erreichbar bleibt.
pub const BEREICH: &str = "Spiegel";

/// Ergebnis eines Backup-Laufs. Ben, 05.08.2026: frueher nur die Zahl der
/// kopierten Dateien - jetzt zusaetzlich `warnungen` fuer Dateien, die
/// bald endgueltig aus dem Papierkorb geloescht werden (siehe protect).
pub struct BackupErgebnis {
    pub kopiert: usize,
    pub warnungen: Vec<protect::PapierkorbWarnung>,
    /// Dateien, die nicht lesbar waren - fast immer, weil ein Programm sie
    /// gerade offen haelt (Windows-Fehler 32/33). Sie fehlen im Backup und
    /// muessen sichtbar sein, sonst faellt es niemandem auf.
    pub uebersprungen: Vec<String>,
}

/// Ein Eintrag in papierkorb.json - die Warnung plus das Profil, ueber das
/// sie zurueckgeholt werden kann.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PapierkorbEintrag {
    pub profil: String,
    pub pfad: String,
    pub tage: i64,
    pub groesse: u64,
}

/// Lokale Statusdatei, die das LUNA-HUD liest. Sie liegt neben
/// profiles.json und wird bei JEDEM Lauf neu geschrieben - auch leer.
/// Das ist Absicht: nur so verschwindet die Warnung im HUD wieder, wenn
/// eine Datei zurueckgeholt oder verlaengert wurde.
pub fn papierkorb_status_pfad() -> std::path::PathBuf {
    profile::profiles_path().with_file_name("papierkorb.json")
}

fn warnungen_log_pfad() -> std::path::PathBuf {
    profile::profiles_path().with_file_name("warnungen.log")
}

/// Haengt die Warnungen mit Zeitstempel und Profilnamen an warnungen.log an.
/// Bewusst zusaetzlich zur JSON-Datei: das Log wird nie ueberschrieben, es
/// bleibt also nachvollziehbar, was wann faellig war.
fn warnungen_loggen(profil: &str, warnungen: &[protect::PapierkorbWarnung]) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true).append(true).open(warnungen_log_pfad())
    {
        let zeit = chrono::Local::now().format("%Y-%m-%d %H:%M");
        for w in warnungen {
            let _ = writeln!(
                f, "[{zeit}] {profil}: {} wird in {} Tag(en) endgueltig geloescht",
                w.pfad, w.tage);
        }
    }
}

/// Haengt uebersprungene Dateien ans selbe Log - im selben Format, damit
/// es nur EINE Datei gibt, in die man schaut, wenn etwas fehlt.
fn uebersprungen_loggen(profil: &str, dateien: &[String]) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true).append(true).open(warnungen_log_pfad())
    {
        let zeit = chrono::Local::now().format("%Y-%m-%d %H:%M");
        for d in dateien {
            let _ = writeln!(f, "[{zeit}] {profil}: NICHT gesichert - {d}");
        }
    }
}

/// Liest die Eintraege aus papierkorb.json. Fehlt sie, ist die Liste leer.
pub fn papierkorb_status_lesen() -> Vec<PapierkorbEintrag> {
    std::fs::read_to_string(papierkorb_status_pfad())
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| serde_json::from_value(v["eintraege"].clone()).ok())
        .unwrap_or_default()
}

/// Schreibt papierkorb.json neu - aber nur fuer die Profile, die gerade
/// gelaufen sind. Eintraege anderer Profile bleiben stehen.
///
/// Ohne diese Mischung wuerde ein Lauf mit `--profile NAME` (so startet die
/// Aufgabenplanung) die Warnungen aller uebrigen Profile stillschweigend
/// loeschen, und im HUD verschwaende eine echte Warnung.
///
/// Fehler werden bewusst geschluckt: ein nicht schreibbarer Status darf
/// niemals ein gelungenes Backup zu einem Fehlschlag machen.
pub fn papierkorb_status_aktualisieren(gelaufene: &[String], neue: &[PapierkorbEintrag]) {
    let mut alle: Vec<PapierkorbEintrag> = papierkorb_status_lesen()
        .into_iter()
        .filter(|e| !gelaufene.contains(&e.profil))
        .collect();
    alle.extend_from_slice(neue);
    alle.sort_by_key(|e| e.tage);

    let inhalt = serde_json::json!({
        "stand": chrono::Local::now().format("%d.%m.%Y %H:%M").to_string(),
        "eintraege": alle,
    });
    let pfad = papierkorb_status_pfad();
    if let Some(ordner) = pfad.parent() {
        let _ = std::fs::create_dir_all(ordner);
    }
    if let Ok(txt) = serde_json::to_string_pretty(&inhalt) {
        let _ = std::fs::write(&pfad, txt);
    }
}

/// Sichert das Profil an EIN Ziel (Drive wenn aktiviert, sonst lokal):
/// eine 1:1-Kopie der Quellen unter BEREICH. Was in der Quelle geloescht
/// wird, wandert dort in einen Papierkorb und bleibt `retention_months`
/// lang erreichbar.
///
/// Rueckgabe: kopierte Dateien plus etwaige Papierkorb-Warnungen.
pub fn run_backup(
    profile: &profile::Profile,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<BackupErgebnis, Error> {
    // Verlauf und Meldung (Ben, 27.11.2026): nur das Ergebnis mitschreiben,
    // der Lauf selbst bleibt unberuehrt.
    let start = chrono::Local::now();
    let uhr = std::time::Instant::now();
    let erg = lauf(profile, cancel);
    let eintrag = verlauf::Lauf {
        profil: profile.name.clone(),
        start: start.format("%d.%m.%Y %H:%M").to_string(),
        dauer_s: uhr.elapsed().as_secs(),
        kopiert: erg.as_ref().map_or(0, |e| e.kopiert),
        uebersprungen: erg.as_ref().map_or(0, |e| e.uebersprungen.len()),
        papierkorb_warnungen: erg.as_ref().map_or(0, |e| e.warnungen.len()),
        fehler: erg.as_ref().err().map(|e| e.to_string()),
    };
    verlauf::eintragen(&eintrag);
    verlauf::melden_nach(&eintrag);
    erg
}

fn lauf(
    profile: &profile::Profile,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<BackupErgebnis, Error> {
    // Nur ein Lauf gleichzeitig, ueber Prozesse hinweg: LUNA, Aufgabenplanung
    // und Fenster wissen nichts voneinander (Ben, 15.10.2026 - zwei Laeufe
    // parallel auf dasselbe Drive). OS-Sperre statt Marker-Datei: stuerzt ein
    // Lauf ab, gibt Windows sie selbst frei - keine Leiche, die alles blockiert.
    // Gehalten bis zum Ende der Funktion ('_sperre' lebt so lange).
    let sperr_pfad = profile::profiles_path().with_file_name("lauf.lock");
    if let Some(ordner) = sperr_pfad.parent() {
        std::fs::create_dir_all(ordner)?;
    }
    let _sperre = std::fs::OpenOptions::new()
        .create(true).write(true).truncate(false)
        .open(&sperr_pfad)?;
    match _sperre.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            return Err(Error::General(
                "Es läuft schon eine Sicherung - dieser Lauf wurde nicht gestartet".into(),
            ));
        }
        Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
    }

    // Fehlt eine Quelle (Platte nicht angesteckt, Ordner umbenannt), sieht
    // der Scan sie als leer - und der Papierkorb schluckte das halbe
    // Backup. Lieber hier abbrechen als dort raten (Ben, 23.08.2026).
    if let Some(fehlt) = profile.source_paths.iter().find(|p| !p.exists()) {
        return Err(Error::General(format!(
            "Quellordner nicht erreichbar: {} - nichts geändert",
            fehlt.display()
        )));
    }

    let rt = tokio::runtime::Runtime::new().map_err(|e| Error::General(e.to_string()))?;

    let target: Box<dyn dest::Destination> = if profile.drive_enabled {
        if profile.drive_folder_id.is_empty() {
            return Err(Error::General("Kein Google-Drive-Ordner ausgewählt".into()));
        }
        Box::new(
            dest::drive::DriveDest::new(profile.drive_folder_id.clone(), None)
                .map_err(Error::General)?,
        )
    } else {
        Box::new(dest::local::LocalDest::new(profile.base_dest_dir()))
    };

    let exclude = profile.exclude_patterns();
    // Einweg-Ordner als Zielpraefix, wie der Scanner sie benennt - ganze
    // Quell-Ordner oder Unterordner beliebiger Tiefe. Was unter keiner
    // Quelle mehr liegt, zaehlt nicht.
    let einweg: Vec<String> = profile.einweg.iter()
        .filter_map(|p| scanner::einweg_praefix(&profile.source_paths, p))
        .collect();
    // Meldet im Takt, wo der Lauf steht - fuer das Syncie-Fenster und das
    // LUNA-HUD. Raeumt beim Verlassen selbst auf (Drop).
    let fort = fortschritt::Fortschritt::neu(&profile.name);
    let mut zustand = zustand::Zustand::laden(&profile.name);

    let ergebnis = rt.block_on(async {
        let bereich = dest::PrefixDest::new(target.as_ref(), BEREICH);
        let (kopiert, warnungen, uebersprungen) = protect::protect(
            &profile.source_paths,
            &bereich,
            profile.retention_months,
            &exclude,
            &einweg,
            cancel,
            &fort,
            &mut zustand,
        )
        .await
        .map_err(Error::General)?;

        Ok::<_, Error>(BackupErgebnis { kopiert, warnungen, uebersprungen })
    });

    // Auch nach einem Abbruch: die bis dahin hochgeladenen Dateien sollen
    // beim naechsten Lauf nicht noch einmal hochgehen.
    zustand.speichern();
    let ergebnis = ergebnis?;

    if !ergebnis.warnungen.is_empty() {
        warnungen_loggen(&profile.name, &ergebnis.warnungen);
    }
    if !ergebnis.uebersprungen.is_empty() {
        uebersprungen_loggen(&profile.name, &ergebnis.uebersprungen);
    }
    Ok(ergebnis)
}
