//! Seite „Verlauf" (Ben, 27.11.2026): Meldung nach dem Lauf ein-/ausschalten
//! und die letzten Läufe aller Profile, neuester oben.

use syncie_core::verlauf::{self, Melden};
use crate::theme;

pub struct VerlaufTab {
    melden: Melden,
}

impl Default for VerlaufTab {
    fn default() -> Self {
        Self { melden: verlauf::einstellungen_lesen().melden }
    }
}

fn dauer(s: u64) -> String {
    match s {
        0..=59 => format!("{s} s"),
        60..=3599 => format!("{} min", s / 60),
        _ => format!("{} h {:02} min", s / 3600, s % 3600 / 60),
    }
}

impl VerlaufTab {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Verlauf").size(19.0).color(theme::TEXT));
        ui.add_space(14.0);

        theme::karte(theme::FLAECHE_1).show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::kopfzeile(ui, "Meldung nach dem Lauf");
            ui.add_space(4.0);
            let vorher = self.melden;
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.melden, Melden::Aus, "Aus");
                ui.selectable_value(&mut self.melden, Melden::NurProbleme, "Nur bei Problemen");
                ui.selectable_value(&mut self.melden, Melden::Immer, "Nach jedem Lauf");
            });
            ui.label(egui::RichText::new(
                "Windows-Meldung unten rechts. Probleme: Lauf fehlgeschlagen oder Dateien \
                 gesperrt und nicht gesichert. Gilt auch für Läufe aus der Aufgabenplanung.")
                .size(11.5).color(theme::TEXT_LEISE));
            if self.melden != vorher {
                verlauf::einstellungen_speichern(&verlauf::Einstellungen { melden: self.melden });
            }
        });
        ui.add_space(12.0);

        // ponytail: liest die Datei in jedem Bild neu - bei 200 Zeilen egal,
        // zwischenspeichern, falls der Verlauf deutlich größer wird
        let laeufe = verlauf::lesen();
        theme::karte(theme::FLAECHE_1).show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::kopfzeile(ui, "Letzte Läufe");
            ui.add_space(4.0);
            if laeufe.is_empty() {
                ui.label(egui::RichText::new("Noch kein Lauf aufgezeichnet.").color(theme::TEXT_LEISE));
                return;
            }
            egui::Grid::new("laeufe").num_columns(6).spacing([18.0, 7.0]).striped(false).show(ui, |ui| {
                for k in ["Start", "Profil", "Dauer", "Gesichert", "Gesperrt", "Ergebnis"] {
                    theme::kopfzeile(ui, k);
                }
                ui.end_row();
                for l in laeufe.iter().rev().take(50) {
                    ui.label(egui::RichText::new(&l.start).color(theme::TEXT));
                    ui.label(&l.profil);
                    ui.label(dauer(l.dauer_s));
                    ui.label(l.kopiert.to_string());
                    ui.label(if l.uebersprungen > 0 {
                        egui::RichText::new(l.uebersprungen.to_string()).color(theme::WARNUNG)
                    } else {
                        egui::RichText::new("0")
                    });
                    match &l.fehler {
                        Some(f) => ui.add(egui::Label::new(
                            egui::RichText::new(format!("Fehler: {f}")).color(theme::KRITISCH)).truncate())
                            .on_hover_text(f),
                        None if l.uebersprungen > 0 =>
                            ui.label(egui::RichText::new("unvollständig").color(theme::WARNUNG)),
                        None => ui.label(egui::RichText::new("ok").color(theme::GUT)),
                    };
                    ui.end_row();
                }
            });
        });
    }
}
