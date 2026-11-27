use std::path::PathBuf;
use std::process::Command;
use crate::profile::Profile;

/// Name der Windows-Aufgabe für ein Profil.
fn task_name(profile_name: &str) -> String {
    format!("Syncie - {profile_name}")
}

/// Pfad zur CLI neben der laufenden EXE.
fn cli_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("Kein EXE-Verzeichnis")?;
    let cli = dir.join("syncie-cli.exe");
    if !cli.exists() {
        return Err(format!("syncie-cli.exe nicht gefunden neben {}", exe.display()));
    }
    Ok(cli)
}

fn weekday_codes(p: &Profile) -> Vec<&'static str> {
    let s = &p.schedule;
    [
        (s.mon, "MON"), (s.tue, "TUE"), (s.wed, "WED"), (s.thu, "THU"),
        (s.fri, "FRI"), (s.sat, "SAT"), (s.sun, "SUN"),
    ]
    .iter()
    .filter_map(|(on, code)| on.then_some(*code))
    .collect()
}

/// Legt die Aufgabe in der Windows-Aufgabenplanung an (überschreibt eine
/// bestehende gleichen Namens). Bei "Nur manuell" wird sie stattdessen entfernt.
pub fn register(p: &Profile) -> Result<String, String> {
    if p.schedule.manual_only {
        unregister(&p.name)?;
        return Ok(crate::sprache::t("Zeitplan entfernt (nur manuell)").into());
    }

    let days = weekday_codes(p);
    if days.is_empty() {
        return Err(crate::sprache::t("Kein Wochentag ausgewählt").into());
    }

    let cli = cli_path()?;
    let tr = format!("\"{}\" --run --profile \"{}\"", cli.display(), p.name);
    let st = format!("{:02}:{:02}", p.schedule.hour, p.schedule.minute);

    let out = Command::new("schtasks")
        .args([
            "/Create", "/F",
            "/TN", &task_name(&p.name),
            "/TR", &tr,
            "/SC", "WEEKLY",
            "/D", &days.join(","),
            "/ST", &st,
        ])
        .output()
        .map_err(|e| format!("schtasks: {e}"))?;

    if out.status.success() {
        Ok(crate::sprache::t("Zeitplan aktiv: {tage} um {zeit}")
            .replace("{tage}", &days.join(",")).replace("{zeit}", &st))
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn unregister(profile_name: &str) -> Result<(), String> {
    Command::new("schtasks")
        .args(["/Delete", "/F", "/TN", &task_name(profile_name)])
        .output()
        .map_err(|e| format!("schtasks: {e}"))?;
    // Fehler ignorieren: eine nicht vorhandene Aufgabe ist kein Problem.
    Ok(())
}
