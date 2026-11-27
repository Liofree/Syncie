//! Seite „Über Syncie" (Ben, 27.11.2026).
//!
//! Alles, was sich vor der Veröffentlichung noch ändert, steht in den
//! Konstanten hier oben - Studio-Name, Lizenz, Links. Leerer Link = „folgt".

use crate::theme;
use syncie_core::sprache::t;

/// Herausgeber (Ben, 27.11.2026: liofree.com).
pub const STUDIO: &str = "Liofree";
/// Leitsatz des Studios, dezent unter dem Herausgeber.
pub const SLOGAN: &str = "Quiet tools. Real impact.";
/// Lizenz des Quellcodes - zusammen mit der LICENSE-Datei im Repo festlegen.
pub const LIZENZ: &str = "MIT";
/// Projektseite (z. B. GitHub-Repo mit Releases).
pub const PROJEKT_URL: &str = "https://liofree.com";
/// Spendenlink (Ko-fi, Ben 27.11.2026).
pub const SPENDE_URL: &str = "https://ko-fi.com/liofree";

fn zeile(ui: &mut egui::Ui, titel: &str, inhalt: impl FnOnce(&mut egui::Ui)) {
    ui.label(egui::RichText::new(titel).color(theme::TEXT_LEISE));
    inhalt(ui);
    ui.end_row();
}

fn link(ui: &mut egui::Ui, url: &str) {
    if url.is_empty() {
        ui.label(egui::RichText::new(t("folgt")).color(theme::TEXT_LEISE));
    } else {
        ui.hyperlink(url);
    }
}

pub fn show(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
        ui.painter().circle_stroke(rect.center(), 15.0, egui::Stroke::new(4.0_f32, theme::SPIEGEL));
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Syncie").size(22.0).strong().color(theme::TEXT));
            ui.label(egui::RichText::new(concat!("Version ", env!("CARGO_PKG_VERSION")))
                .color(theme::TEXT_LEISE));
        });
    });
    ui.add_space(14.0);

    theme::karte(theme::FLAECHE_1).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(egui::RichText::new(
            t("Syncie sichert deine Ordner auf Google Drive oder ein lokales Laufwerk - als Spiegel mit Papierkorb oder als Einweg-Sicherung, von Hand oder nach Zeitplan."))
            .color(theme::TEXT));
        ui.add_space(10.0);
        egui::Grid::new("ueber").num_columns(2).spacing([24.0, 8.0]).show(ui, |ui| {
            zeile(ui, t("Herausgeber"), |ui| {
                ui.horizontal(|ui| {
                    ui.label(STUDIO);
                    ui.label(egui::RichText::new(SLOGAN).italics().size(11.5).color(theme::TEXT_LEISE));
                });
            });
            zeile(ui, t("Lizenz"), |ui| { ui.label(LIZENZ); });
            zeile(ui, t("Projektseite"), |ui| link(ui, PROJEKT_URL));
            zeile(ui, t("Unterstützen"), |ui| link(ui, SPENDE_URL));
        });
    });
    ui.add_space(12.0);

    theme::karte(theme::FLAECHE_1).show(ui, |ui| {
        ui.set_width(ui.available_width());
        theme::kopfzeile(ui, t("Datenschutz"));
        ui.add_space(4.0);
        ui.label(egui::RichText::new(
            t("Syncie läuft nur auf deinem Rechner. Einstellungen, Verlauf und die Google-Anmeldung liegen in %APPDATA%\\syncie. Deine Dateien gehen nur an das Ziel, das du wählst - an den Herausgeber wird nichts gesendet."))
            .size(12.0).color(theme::TEXT_2));
        ui.add_space(8.0);
        theme::kopfzeile(ui, t("Gebaut mit"));
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Rust · egui/eframe · tokio · reqwest · serde · chrono · walkdir · rfd")
            .size(12.0).color(theme::TEXT_2));
    });
}
