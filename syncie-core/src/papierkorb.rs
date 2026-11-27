//! Die zwei Aktionen, die Ben im LUNA-HUD je Datei waehlen kann, bevor sie
//! endgueltig aus dem Papierkorb faellt (Ben, 05.08.2026):
//!
//!   zurueckholen  Datei aus dem Drive-Papierkorb zurueck in den lokalen
//!                 Quellordner schreiben. Damit ist sie wieder "lebendig":
//!                 der naechste Lauf findet sie in der Quelle und raeumt
//!                 sie von selbst aus dem Papierkorb.
//!   spaeter       Nur die Frist zuruecksetzen - die Datei bleibt weitere
//!                 volle `retention_months` im Papierkorb liegen.
//!
//! Beide brauchen Netz (Drive) und laufen deshalb NICHT im HUD-Takt,
//! sondern nur auf Knopfdruck ueber syncie-cli.

use std::path::PathBuf;
use chrono::Utc;
use crate::dest::{self, Destination};
use crate::profile::Profile;
use crate::protect::{self, TRASH_DIR};
use crate::{scanner, Error, BEREICH};

/// Baut das Ziel (Drive oder lokal) fuer ein Profil. Gleiche Reihenfolge
/// wie in run_backup - der Papierkorb liegt im Bereich BEREICH.
fn ziel_bauen(profile: &Profile) -> Result<Box<dyn Destination>, Error> {
    if profile.drive_enabled {
        if profile.drive_folder_id.is_empty() {
            return Err(Error::General("Kein Google-Drive-Ordner ausgewählt".into()));
        }
        Ok(Box::new(
            dest::drive::DriveDest::new(profile.drive_folder_id.clone(), None)
                .map_err(Error::General)?,
        ))
    } else {
        Ok(Box::new(dest::local::LocalDest::new(profile.base_dest_dir())))
    }
}

/// Uebersetzt einen Backup-Pfad ("LUNA/data/foo.txt") in den echten
/// lokalen Pfad. Die oberste Ebene ist der Ordnername der Quelle (siehe
/// scanner::root_label) - daraus laesst sich der Quellordner eindeutig
/// zurueckfinden.
///
/// Gibt None, wenn der Quellordner nicht mehr im Profil steht: dann waere
/// jedes Ziel geraten, und geraten wird beim Schreiben auf fremde Platten
/// nicht.
fn lokales_ziel(profile: &Profile, pfad: &str) -> Option<PathBuf> {
    let (label, rest) = pfad.split_once('/')?;
    for quelle in &profile.source_paths {
        if scanner::root_label(quelle) == label {
            let mut ziel = quelle.clone();
            for teil in rest.split('/') {
                ziel.push(teil);
            }
            return Some(ziel);
        }
    }
    None
}

/// Holt eine Datei aus dem Papierkorb zurueck in den lokalen Quellordner.
///
/// Das ist die EINZIGE Stelle, an der Syncie lokal schreibt - und sie legt
/// ausschliesslich an. Eine schon vorhandene Datei wird NICHT ueberbuegelt:
/// Wer die Datei inzwischen selbst wiederhergestellt hat, soll sie nicht
/// durch einen alten Stand aus dem Papierkorb verlieren.
pub fn zurueckholen(profile: &Profile, pfad: &str) -> Result<String, Error> {
    let ziel_pfad = lokales_ziel(profile, pfad).ok_or_else(|| {
        Error::General(format!(
            "Quellordner zu '{pfad}' steht nicht mehr im Profil - Ziel unklar"
        ))
    })?;
    if ziel_pfad.exists() {
        return Err(Error::General(format!(
            "{} existiert lokal bereits - nichts überschrieben",
            ziel_pfad.display()
        )));
    }

    let ziel = ziel_bauen(profile)?;
    let rt = tokio::runtime::Runtime::new().map_err(|e| Error::General(e.to_string()))?;

    rt.block_on(async {
        let bereich = dest::PrefixDest::new(ziel.as_ref(), BEREICH);
        let roh = bereich
            .read(&format!("{TRASH_DIR}/{pfad}"))
            .await
            .map_err(|e| Error::General(format!("Papierkorb lesen: {e}")))?;
        let daten = protect::nutzdaten_ohne_kopf(&roh);

        if let Some(ordner) = ziel_pfad.parent() {
            std::fs::create_dir_all(ordner)?;
        }
        std::fs::write(&ziel_pfad, daten)?;

        // Erst nach dem erfolgreichen Schreiben aus dem Index nehmen -
        // sonst waere die Datei bei einem Schreibfehler aus beiden Welten
        // verschwunden.
        let mut trash = protect::trash_lesen(&bereich).await;
        if trash.remove(pfad).is_some() {
            protect::trash_schreiben(&bereich, &trash)
                .await
                .map_err(Error::General)?;
        }
        let _ = bereich.delete(&format!("{TRASH_DIR}/{pfad}")).await;

        Ok(format!("{pfad} zurückgeholt nach {}", ziel_pfad.display()))
    })
}

/// Setzt die Loeschfrist zurueck: die Datei bleibt weitere volle
/// `retention_months` im Papierkorb.
pub fn spaeter(profile: &Profile, pfad: &str) -> Result<String, Error> {
    let ziel = ziel_bauen(profile)?;
    let rt = tokio::runtime::Runtime::new().map_err(|e| Error::General(e.to_string()))?;

    rt.block_on(async {
        let bereich = dest::PrefixDest::new(ziel.as_ref(), BEREICH);
        let mut trash = protect::trash_lesen(&bereich).await;
        match trash.get_mut(pfad) {
            Some(eintrag) => eintrag.deleted_at = Utc::now(),
            None => {
                return Err(Error::General(format!(
                    "{pfad} liegt nicht (mehr) im Papierkorb"
                )))
            }
        }
        protect::trash_schreiben(&bereich, &trash)
            .await
            .map_err(Error::General)?;
        Ok(format!(
            "{pfad} bleibt weitere {} Monate erhalten",
            profile.retention_months
        ))
    })
}
