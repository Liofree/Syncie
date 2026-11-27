//! Headless-Schnittstelle für die Windows-Aufgabenplanung und externe Aufrufer
//! (z.B. Luna). Gibt immer eine JSON-Zeile auf stdout aus.
//!
//!   syncie-cli --list                    Profile auflisten
//!   syncie-cli --run [--profile NAME]    Backup starten (alle oder eins)
//!   syncie-cli --due                     nur Profile, die heute laut Zeitplan dran sind
//!
//! Papierkorb (fuer den Warndreieck-Knopf im LUNA-HUD):
//!   syncie-cli --papierkorb                          Liste aus papierkorb.json (kein Netz)
//!   syncie-cli --zurueckholen --profile P --pfad X   Datei lokal wiederherstellen
//!   syncie-cli --spaeter      --profile P --pfad X   Frist zuruecksetzen
//!
//! Exit-Code: 0 = ok, 1 = Fehler.

use std::sync::atomic::AtomicBool;
use chrono::{Datelike, Local};
use syncie_core::profile;

fn out(v: serde_json::Value) {
    println!("{v}");
}


fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    let value_of = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };

    let profiles = profile::load_all();

    if has("--list") || args.is_empty() {
        out(serde_json::json!({
            "ok": true,
            "profiles": profiles.iter().map(|p| serde_json::json!({
                "name": p.name,
                "sources": p.source_paths.len(),
                "target": if p.drive_enabled { "drive" } else { "local" },
                "manual_only": p.schedule.manual_only,
                "hour": p.schedule.hour,
                "minute": p.schedule.minute,
            })).collect::<Vec<_>>(),
        }));
        return;
    }

    // Papierkorb-Liste: liest NUR die lokale papierkorb.json, kein Drive-
    // Zugriff. Das HUD darf den Knopf nicht mit einer Netzabfrage bezahlen.
    if has("--papierkorb") {
        let eintraege = syncie_core::papierkorb_status_lesen();
        out(serde_json::json!({ "ok": true, "eintraege": eintraege }));
        return;
    }

    // Die beiden Aktionen aus der HUD-Liste. Beide brauchen Drive und
    // laufen deshalb nur auf Knopfdruck.
    if has("--zurueckholen") || has("--spaeter") {
        let (Some(pname), Some(pfad)) = (value_of("--profile"), value_of("--pfad")) else {
            out(serde_json::json!({ "ok": false, "error": "--profile und --pfad nötig" }));
            std::process::exit(1);
        };
        let Some(p) = profiles.iter().find(|p| p.name == pname) else {
            out(serde_json::json!({ "ok": false, "error": format!("Profil '{pname}' nicht gefunden") }));
            std::process::exit(1);
        };
        let ergebnis = if has("--zurueckholen") {
            syncie_core::papierkorb::zurueckholen(p, &pfad)
        } else {
            syncie_core::papierkorb::spaeter(p, &pfad)
        };
        match ergebnis {
            Ok(meldung) => {
                // Erledigten Eintrag sofort aus der Statusdatei nehmen -
                // sonst zeigt das HUD ihn bis zum naechsten Backup weiter.
                let rest: Vec<_> = syncie_core::papierkorb_status_lesen()
                    .into_iter()
                    .filter(|e| !(e.profil == pname && e.pfad == pfad))
                    .collect();
                syncie_core::papierkorb_status_aktualisieren(&[pname.clone()],
                    &rest.into_iter().filter(|e| e.profil == pname).collect::<Vec<_>>());
                out(serde_json::json!({ "ok": true, "meldung": meldung }));
            }
            Err(e) => {
                out(serde_json::json!({ "ok": false, "error": e.to_string() }));
                std::process::exit(1);
            }
        }
        return;
    }

    if !has("--run") {
        out(serde_json::json!({ "ok": false, "error": "Unbekannte Argumente. --list | --run [--profile NAME] [--due] | --papierkorb | --zurueckholen|--spaeter --profile NAME --pfad X" }));
        std::process::exit(1);
    }

    let wanted = value_of("--profile");
    let only_due = has("--due");
    let today = Local::now().weekday();

    let selected: Vec<_> = profiles
        .iter()
        .filter(|p| wanted.as_ref().is_none_or(|w| &p.name == w))
        .filter(|p| !only_due || p.runs_on(today))
        .collect();

    if selected.is_empty() {
        out(serde_json::json!({ "ok": true, "ran": 0, "files": 0, "note": "Kein passendes Profil" }));
        return;
    }

    let cancel = AtomicBool::new(false);
    let mut total = 0usize;
    let mut results = Vec::new();
    // Papierkorb-Warnungen aller gelaufenen Profile fuer papierkorb.json,
    // die das LUNA-HUD liest.
    let mut gelaufen: Vec<String> = Vec::new();
    let mut papierkorb: Vec<syncie_core::PapierkorbEintrag> = Vec::new();

    for p in &selected {
        match syncie_core::run_backup(p, &cancel) {
            Ok(e) => {
                total += e.kopiert;
                gelaufen.push(p.name.clone());
                papierkorb.extend(e.warnungen.iter().map(|w| syncie_core::PapierkorbEintrag {
                    profil: p.name.clone(), pfad: w.pfad.clone(),
                    tage: w.tage, groesse: w.groesse,
                }));
                results.push(serde_json::json!({
                    "profile": p.name, "ok": true, "files": e.kopiert,
                    "warnungen": e.warnungen,
                    // Gesperrte Dateien: im Backup fehlen sie, also duerfen
                    // sie in der Antwort nicht fehlen.
                    "uebersprungen": e.uebersprungen,
                }));
            }
            Err(e) => {
                results.push(serde_json::json!({ "profile": p.name, "ok": false, "error": e.to_string() }));
            }
        }
    }

    syncie_core::papierkorb_status_aktualisieren(&gelaufen, &papierkorb);
    if let Some(naechste) = papierkorb.iter().map(|e| e.tage).min() {
        let text = syncie_core::sprache::t(
            "{n} Datei(en) werden in {d} Tag(en) endgültig aus dem Papierkorb gelöscht. Syncie öffnen, um sie zurückzuholen.")
            .replace("{n}", &papierkorb.len().to_string()).replace("{d}", &naechste.to_string());
        syncie_core::verlauf::hinweis(&text, true);
    }

    let ok = results.iter().all(|r| r["ok"] == true);
    out(serde_json::json!({ "ok": ok, "ran": selected.len(), "files": total, "results": results }));
    if !ok {
        std::process::exit(1);
    }
}
