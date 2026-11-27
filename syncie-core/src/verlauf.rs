//! Lauf-Verlauf und Benachrichtigung (Ben, 27.11.2026).
//!
//! `run_backup` schreibt nach jedem Lauf eine Zeile nach verlauf.jsonl -
//! egal ob aus dem Fenster, der Aufgabenplanung oder LUNA. Die Sicherung
//! selbst merkt davon nichts: hier wird nur ihr Ergebnis abgelegt.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Ein Lauf eines Profils.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lauf {
    pub profil: String,
    /// "27.11.2026 23:00" - dasselbe Format wie die Übersicht.
    pub start: String,
    pub dauer_s: u64,
    pub kopiert: usize,
    pub uebersprungen: usize,
    pub papierkorb_warnungen: usize,
    #[serde(default)]
    pub fehler: Option<String>,
}

impl Lauf {
    /// Fehlgeschlagen ODER Dateien fehlen im Backup - beides will man wissen.
    pub fn hat_probleme(&self) -> bool {
        self.fehler.is_some() || self.uebersprungen > 0
    }
}

/// Wann nach einem Lauf eine Windows-Meldung kommt. Standard: nie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Melden {
    #[default]
    Aus,
    NurProbleme,
    Immer,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Einstellungen {
    #[serde(default)]
    pub melden: Melden,
}

/// So viele Läufe bleiben stehen - reicht für Monate, bleibt klein.
pub const BEHALTEN: usize = 200;

fn datei(name: &str) -> PathBuf {
    crate::profile::profiles_path().with_file_name(name)
}

pub fn einstellungen_lesen() -> Einstellungen {
    std::fs::read_to_string(datei("einstellungen.json")).ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn einstellungen_speichern(e: &Einstellungen) {
    let p = datei("einstellungen.json");
    if let Some(o) = p.parent() { let _ = std::fs::create_dir_all(o); }
    if let Ok(txt) = serde_json::to_string_pretty(e) { let _ = std::fs::write(p, txt); }
}

/// Alle Läufe, ältester zuerst. Kaputte Zeilen werden übergangen.
pub fn lesen() -> Vec<Lauf> {
    std::fs::read_to_string(datei("verlauf.jsonl")).unwrap_or_default()
        .lines()
        .filter_map(|z| serde_json::from_str(z).ok())
        .collect()
}

pub fn eintragen(lauf: &Lauf) {
    let mut alle = lesen();
    alle.push(lauf.clone());
    let weg = alle.len().saturating_sub(BEHALTEN);
    let txt: String = alle[weg..].iter()
        .filter_map(|l| serde_json::to_string(l).ok())
        .map(|z| z + "\n")
        .collect();
    let p = datei("verlauf.jsonl");
    if let Some(o) = p.parent() { let _ = std::fs::create_dir_all(o); }
    let _ = std::fs::write(p, txt);
}

pub fn soll_melden(m: Melden, probleme: bool) -> bool {
    match m {
        Melden::Aus => false,
        Melden::NurProbleme => probleme,
        Melden::Immer => true,
    }
}

/// Nach dem Lauf: Meldung, falls eingeschaltet.
pub fn melden_nach(l: &Lauf) {
    if !soll_melden(einstellungen_lesen().melden, l.hat_probleme()) {
        return;
    }
    let text = match (&l.fehler, l.uebersprungen) {
        (Some(f), _) => format!("{}: fehlgeschlagen - {f}", l.profil),
        (None, 0) => format!("{}: {} Dateien gesichert", l.profil, l.kopiert),
        (None, n) => format!("{}: {} gesichert, {n} gesperrt und nicht gesichert", l.profil, l.kopiert),
    };
    hinweis(&text, l.hat_probleme());
}

/// Meldung unten rechts in Windows (Ben, 19.10.2026, aus der CLI hierher
/// geholt): PowerShell + Windows Forms, keine zusätzliche Bibliothek. Läuft
/// losgelöst, damit der Lauf nicht auf die Meldung wartet. Der Text geht
/// über eine Umgebungsvariable hinein - so kann kein Zeichen darin das
/// Skript verändern.
#[cfg(windows)]
pub fn hinweis(text: &str, warnung: bool) {
    use std::os::windows::process::CommandExt;
    const OHNE_FENSTER: u32 = 0x0800_0000;
    let (icon, art) = if warnung { ("Warning", "Warning") } else { ("Information", "Info") };
    let skript = format!(
        "Add-Type -AssemblyName System.Windows.Forms,System.Drawing; \
         $n = New-Object System.Windows.Forms.NotifyIcon; \
         $n.Icon = [System.Drawing.SystemIcons]::{icon}; $n.Visible = $true; \
         $n.ShowBalloonTip(15000, 'Syncie', $env:SYNCIE_HINWEIS, [System.Windows.Forms.ToolTipIcon]::{art}); \
         Start-Sleep -Seconds 16; $n.Dispose()");
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &skript])
        .env("SYNCIE_HINWEIS", text)
        .creation_flags(OHNE_FENSTER)
        .spawn();
}

#[cfg(not(windows))]
pub fn hinweis(_text: &str, _warnung: bool) {}
