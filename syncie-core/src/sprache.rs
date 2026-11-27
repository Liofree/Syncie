//! Sprache der Oberfläche (Ben, 27.11.2026): Deutsch, Englisch, Spanisch.
//!
//! Der deutsche Text ist zugleich der Schlüssel: `t("Bereit")` liefert je
//! nach Sprache "Bereit", "Ready" oder "Listo". Fehlt eine Übersetzung,
//! bleibt der deutsche Text stehen - der Test `sprache.rs` schlägt dann an.
//! Platzhalter wie `{n}` ersetzt der Aufrufer mit `.replace`.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sprache {
    De,
    En,
    Es,
}

/// 0 = noch nicht bestimmt, sonst 1 + Index.
static AKTIV: AtomicU8 = AtomicU8::new(0);

fn als_zahl(s: Sprache) -> u8 {
    match s { Sprache::De => 1, Sprache::En => 2, Sprache::Es => 3 }
}

/// Eingestellte Sprache; beim ersten Aufruf aus den Einstellungen, sonst
/// aus der Systemsprache.
pub fn aktiv() -> Sprache {
    match AKTIV.load(Ordering::Relaxed) {
        1 => Sprache::De,
        2 => Sprache::En,
        3 => Sprache::Es,
        _ => {
            let s = crate::verlauf::einstellungen_lesen().sprache.unwrap_or_else(system);
            AKTIV.store(als_zahl(s), Ordering::Relaxed);
            s
        }
    }
}

/// Sprache wählen und speichern - gilt sofort und auch für die Aufgabenplanung.
pub fn waehlen(s: Sprache) {
    AKTIV.store(als_zahl(s), Ordering::Relaxed);
    let mut e = crate::verlauf::einstellungen_lesen();
    e.sprache = Some(s);
    crate::verlauf::einstellungen_speichern(&e);
}

/// Windows-Anzeigesprache: Deutsch und Spanisch erkannt, sonst Englisch.
#[cfg(windows)]
pub fn system() -> Sprache {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // SAFETY: parameterlose Win32-Funktion, liefert nur eine Zahl.
    match unsafe { GetUserDefaultUILanguage() } & 0x3ff {
        0x07 => Sprache::De,
        0x0a => Sprache::Es,
        _ => Sprache::En,
    }
}

#[cfg(not(windows))]
pub fn system() -> Sprache {
    match std::env::var("LANG").unwrap_or_default().get(..2) {
        Some("de") => Sprache::De,
        Some("es") => Sprache::Es,
        _ => Sprache::En,
    }
}

/// Übersetzt einen deutschen Oberflächentext.
pub fn t(de: &str) -> &str {
    let spalte = match aktiv() {
        Sprache::De => return de,
        Sprache::En => 1,
        Sprache::Es => 2,
    };
    // ponytail: lineare Suche über ~120 Einträge je Aufruf - bei 60 Bildern/s
    // unmerklich; HashMap, falls die Liste stark wächst
    TEXTE.iter().find(|z| z.0 == de)
        .map(|z| if spalte == 1 { z.1 } else { z.2 })
        .unwrap_or(de)
}

/// (Deutsch, Englisch, Spanisch)
pub const TEXTE: &[(&str, &str, &str)] = &[
    // Seitenleiste, Übersicht
    ("Übersicht", "Overview", "Resumen"),
    ("Verlauf", "History", "Historial"),
    ("Profile", "Profiles", "Perfiles"),
    ("Profil", "Profile", "Perfil"),
    ("+ Neues Profil", "+ New profile", "+ Nuevo perfil"),
    ("Über Syncie", "About Syncie", "Acerca de Syncie"),
    ("Kaffee spendieren", "Buy me a coffee", "Invítame a un café"),
    ("Spendenlink folgt", "Donation link coming soon", "Enlace de donación próximamente"),
    ("manuell", "manual", "manual"),
    ("Bereit", "Ready", "Listo"),
    ("Bereit — letzter Lauf ohne Fehler", "Ready — last run without errors", "Listo — última copia sin errores"),
    ("Sicherung läuft", "Backup running", "Copia en curso"),
    ("Letzter Lauf fehlgeschlagen", "Last run failed", "La última copia falló"),
    ("Fehler", "Error", "Error"),
    ("Abbrechen", "Cancel", "Cancelar"),
    ("Wird abgebrochen ...", "Cancelling ...", "Cancelando ..."),
    ("Sicherung starten", "Start backup", "Iniciar copia"),
    ("Läuft ...", "Running ...", "En curso ..."),
    ("Datei {n} von {m}", "file {n} of {m}", "archivo {n} de {m}"),
    ("wird gelesen ...", "scanning ...", "leyendo ..."),
    ("Spiegel", "Mirror", "Espejo"),
    ("{n} Dateien gesichert", "{n} files backed up", "{n} archivos copiados"),
    ("{n} gesichert · {k} gesperrt, nicht gesichert", "{n} backed up · {k} locked, not backed up",
     "{n} copiados · {k} bloqueados, sin copiar"),
    ("noch {n} Tag(e)", "{n} day(s) left", "quedan {n} día(s)"),
    ("{n} Datei(en) verschwinden bald endgültig aus dem Papierkorb",
     "{n} file(s) will soon be deleted from the recycle bin for good",
     "{n} archivo(s) se borrarán pronto definitivamente de la papelera"),
    ("Quell-Ordner", "Source folders", "Carpetas de origen"),
    ("{n} Quell-Ordner", "{n} source folders", "{n} carpetas de origen"),
    ("{n} nach Zeitplan", "{n} scheduled", "{n} programados"),
    ("nur manuell", "manual only", "solo manual"),
    ("Ziel Google Drive", "target Google Drive", "destino Google Drive"),
    ("Ziel lokal", "target local drive", "destino local"),
    ("Papierkorb", "Recycle bin", "Papelera"),
    ("Monate", "months", "meses"),
    ("gelöschte Dateien", "deleted files", "archivos borrados"),
    ("Papierkorb {n} Monate", "recycle bin {n} months", "papelera {n} meses"),
    ("Noch kein Profil. Lege links mit „+ Neues Profil“ eines an.",
     "No profile yet. Create one on the left with “+ New profile”.",
     "Aún no hay perfiles. Crea uno a la izquierda con «+ Nuevo perfil»."),
    ("Lokal", "Local", "Local"),
    ("{hh}:{mm} Uhr", "{hh}:{mm}", "{hh}:{mm} h"),
    // Profil-Seite
    ("Bei Google angemeldet", "Signed in with Google", "Sesión iniciada con Google"),
    ("Wähle links ein Profil aus oder lege ein neues an.",
     "Pick a profile on the left or create a new one.",
     "Elige un perfil a la izquierda o crea uno nuevo."),
    ("Name des Profils - zum Ändern hineinklicken", "Profile name - click to edit",
     "Nombre del perfil - haz clic para cambiarlo"),
    ("automatisch gespeichert", "saved automatically", "guardado automáticamente"),
    ("Was wird gesichert", "What to back up", "Qué se copia"),
    ("Ordner entfernen", "Remove folder", "Quitar carpeta"),
    ("Spiegeln", "Mirror", "Espejo"),
    ("Einweg", "One-way", "Solo ida"),
    ("Unterordner wieder spiegeln", "Mirror this subfolder again", "Volver a reflejar esta subcarpeta"),
    ("+ Unterordner als Einweg", "+ Subfolder as one-way", "+ Subcarpeta solo ida"),
    ("Ein Unterordner, dessen gelöschte Dateien im Ziel bleiben sollen",
     "A subfolder whose deleted files should stay in the backup",
     "Una subcarpeta cuyos archivos borrados deben quedarse en la copia"),
    ("{a} liegt nicht unter {b}", "{a} is not inside {b}", "{a} no está dentro de {b}"),
    ("+ Ordner hinzufügen", "+ Add folder", "+ Añadir carpeta"),
    ("Lokal gelöschte Dateien wandern in den Papierkorb und sind nach der Frist weg.",
     "Files deleted locally go to the recycle bin and are gone after the retention period.",
     "Los archivos borrados en el equipo van a la papelera y desaparecen tras el plazo."),
    ("Lokal gelöschte Dateien bleiben dauerhaft im Ziel.",
     "Files deleted locally stay in the backup for good.",
     "Los archivos borrados en el equipo se quedan para siempre en la copia."),
    ("Zurück von Einweg auf Spiegeln: beim nächsten Lauf kommt alles, was es lokal nicht mehr gibt, in den Papierkorb.",
     "Switching from one-way back to mirror: on the next run everything that no longer exists locally goes to the recycle bin.",
     "Al volver de solo ida a espejo: en la siguiente copia todo lo que ya no existe en el equipo va a la papelera."),
    ("Wohin", "Where to", "Destino"),
    ("Ordner wählen", "choose folder", "elegir carpeta"),
    ("Lokal / USB", "Local / USB", "Local / USB"),
    ("angemeldet", "signed in", "sesión iniciada"),
    ("nicht angemeldet", "not signed in", "sin sesión"),
    ("Ziel-Ordner:", "Target folder:", "Carpeta de destino:"),
    ("Wählen …", "Choose …", "Elegir …"),
    ("Ordner in Drive:", "Folder in Drive:", "Carpeta en Drive:"),
    ("keiner gewählt", "none selected", "ninguna elegida"),
    ("gewählt", "selected", "elegida"),
    ("— auswählen —", "— select —", "— elegir —"),
    ("Ordner laden", "Load folders", "Cargar carpetas"),
    ("{n} Ordner gefunden", "{n} folders found", "{n} carpetas encontradas"),
    ("Eigene Google-Client-ID", "Your own Google client ID", "Tu propio ID de cliente de Google"),
    ("Einmalig: console.cloud.google.com → Projekt → Drive API aktivieren → OAuth-Client-ID (Desktop-App).",
     "Once: console.cloud.google.com → project → enable Drive API → OAuth client ID (desktop app).",
     "Una vez: console.cloud.google.com → proyecto → activar Drive API → ID de cliente OAuth (app de escritorio)."),
    ("Neu anmelden", "Sign in again", "Volver a iniciar sesión"),
    ("Mit Google anmelden", "Sign in with Google", "Iniciar sesión con Google"),
    ("Wann", "When", "Cuándo"),
    ("Nach Zeitplan sichern", "Back up on a schedule", "Copiar según horario"),
    ("Läuft nur, wenn du in der Übersicht auf „Sicherung starten“ klickst.",
     "Runs only when you click “Start backup” in the overview.",
     "Solo se ejecuta cuando haces clic en «Iniciar copia» en el resumen."),
    ("Mo", "Mo", "L"),
    ("Di", "Tu", "M"),
    ("Mi", "We", "X"),
    ("Do", "Th", "J"),
    ("Fr", "Fr", "V"),
    ("Sa", "Sa", "S"),
    ("So", "Su", "D"),
    ("Uhr", "", "h"),
    ("In Windows-Aufgabenplanung eintragen", "Add to Windows Task Scheduler",
     "Añadir al Programador de tareas de Windows"),
    ("nach jeder Änderung am Zeitplan", "after every schedule change", "tras cada cambio de horario"),
    ("Gelöschte Dateien", "Deleted files", "Archivos borrados"),
    ("Profil löschen", "Delete profile", "Eliminar perfil"),
    ("„{name}“ wirklich löschen?", "Really delete “{name}”?", "¿Eliminar «{name}» de verdad?"),
    ("Ja, löschen", "Yes, delete", "Sí, eliminar"),
    ("Profil gelöscht", "Profile deleted", "Perfil eliminado"),
    // Verlauf
    ("Meldung nach dem Lauf", "Notification after a run", "Aviso tras la copia"),
    ("Aus", "Off", "Desactivado"),
    ("Nur bei Problemen", "Only on problems", "Solo si hay problemas"),
    ("Nach jedem Lauf", "After every run", "Tras cada copia"),
    ("Windows-Meldung unten rechts. Probleme: Lauf fehlgeschlagen oder Dateien gesperrt und nicht gesichert. Gilt auch für Läufe aus der Aufgabenplanung.",
     "Windows notification at the bottom right. Problems: run failed or files locked and not backed up. Also applies to scheduled runs.",
     "Aviso de Windows abajo a la derecha. Problemas: la copia falló o hay archivos bloqueados sin copiar. También para las copias programadas."),
    ("Letzte Läufe", "Recent runs", "Últimas copias"),
    ("Noch kein Lauf aufgezeichnet.", "No runs recorded yet.", "Aún no hay copias registradas."),
    ("Start", "Start", "Inicio"),
    ("Dauer", "Duration", "Duración"),
    ("Gesichert", "Backed up", "Copiados"),
    ("Gesperrt", "Locked", "Bloqueados"),
    ("Ergebnis", "Result", "Resultado"),
    ("unvollständig", "incomplete", "incompleta"),
    ("fehlgeschlagen", "failed", "falló"),
    // Über Syncie
    ("folgt", "coming soon", "próximamente"),
    ("Syncie sichert deine Ordner auf Google Drive oder ein lokales Laufwerk - als Spiegel mit Papierkorb oder als Einweg-Sicherung, von Hand oder nach Zeitplan.",
     "Syncie backs up your folders to Google Drive or a local drive - as a mirror with a recycle bin or as a one-way backup, by hand or on a schedule.",
     "Syncie copia tus carpetas a Google Drive o a una unidad local - como espejo con papelera o como copia de solo ida, a mano o según horario."),
    ("Herausgeber", "Publisher", "Editor"),
    ("Lizenz", "License", "Licencia"),
    ("Projektseite", "Website", "Sitio web"),
    ("Unterstützen", "Support", "Apoyar"),
    ("Datenschutz", "Privacy", "Privacidad"),
    ("Syncie läuft nur auf deinem Rechner. Einstellungen, Verlauf und die Google-Anmeldung liegen in %APPDATA%\\syncie. Deine Dateien gehen nur an das Ziel, das du wählst - an den Herausgeber wird nichts gesendet.",
     "Syncie runs only on your computer. Settings, history and the Google sign-in are stored in %APPDATA%\\syncie. Your files only go to the target you choose - nothing is sent to the publisher.",
     "Syncie funciona solo en tu ordenador. Los ajustes, el historial y la sesión de Google se guardan en %APPDATA%\\syncie. Tus archivos solo van al destino que elijas - no se envía nada al editor."),
    ("Gebaut mit", "Built with", "Hecho con"),
    // Meldungen aus dem Kern
    ("Es läuft schon eine Sicherung - dieser Lauf wurde nicht gestartet",
     "A backup is already running - this run was not started",
     "Ya hay una copia en curso - esta no se ha iniciado"),
    ("Quellordner nicht erreichbar: {p} - nichts geändert",
     "Source folder not reachable: {p} - nothing changed",
     "Carpeta de origen no disponible: {p} - no se cambió nada"),
    ("Kein Google-Drive-Ordner ausgewählt", "No Google Drive folder selected",
     "No se ha elegido carpeta de Google Drive"),
    ("Zeitplan entfernt (nur manuell)", "Schedule removed (manual only)", "Horario eliminado (solo manual)"),
    ("Kein Wochentag ausgewählt", "No weekday selected", "No se ha elegido ningún día"),
    ("Zeitplan aktiv: {tage} um {zeit}", "Schedule active: {tage} at {zeit}", "Horario activo: {tage} a las {zeit}"),
    ("Zeitüberschreitung (2 Minuten)", "Timed out (2 minutes)", "Tiempo agotado (2 minutos)"),
    ("{n} Datei(en) werden in {d} Tag(en) endgültig aus dem Papierkorb gelöscht. Syncie öffnen, um sie zurückzuholen.",
     "{n} file(s) will be deleted from the recycle bin for good in {d} day(s). Open Syncie to restore them.",
     "{n} archivo(s) se borrarán definitivamente de la papelera en {d} día(s). Abre Syncie para recuperarlos."),
];
