//! Profil-Seite (Ben, 27.11.2026): vier Schritte untereinander - Was,
//! Wohin, Wann, gelöschte Dateien. Die Profilauswahl steht links in der
//! Seitenleiste (`profilliste`), nicht mehr in einer eigenen Spalte.
//!
//! Gespeichert wird automatisch, sobald sich das Formular ändert - vorher
//! gab es einen Speichern-Knopf UND ein Schreiben in jedem Bild, was sich
//! widersprach.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use syncie_core::profile::Profile;
use crate::theme;

#[derive(Default)]
pub struct SettingsTab {
    profiles: Vec<Profile>,
    selected: Option<usize>,
    form: Profile,
    status: String,
    drive_folders: Vec<(String, String)>,
    drive_auth_rx: Option<mpsc::Receiver<Result<(), String>>>,
    loeschen_fragen: bool,
}

// Ablageort und Laden liegen in syncie-core, damit CLI und GUI dieselbe
// Datei benutzen.
pub use syncie_core::profile::{load_all as load_profiles, profiles_path};

/// "C:\Users\x\Documents\LUNA" -> "Documents\LUNA": der volle Pfad steht im
/// Tooltip, auf dem Chip reichen die letzten zwei Teile.
fn kurz(p: &Path) -> String {
    let teile: Vec<_> = p.components().rev().take(2)
        .map(|c| c.as_os_str().to_string_lossy().trim_end_matches('\\').to_string())
        .collect();
    teile.into_iter().rev().collect::<Vec<_>>().join("\\")
}

/// Nummer im Kreis links, daneben die Karte mit Überschrift.
fn schritt(ui: &mut egui::Ui, nr: u8, titel: &str, inhalt: impl FnOnce(&mut egui::Ui)) {
    // Karte um den Kreis eingerückt; die Einrückung gilt nur für diese Zeile.
    let mitte = ui.cursor().min + egui::vec2(12.0, 22.0);
    ui.painter().circle_filled(mitte, 12.0, theme::FLAECHE_2);
    ui.painter().text(mitte, egui::Align2::CENTER_CENTER, nr.to_string(),
                      egui::FontId::proportional(12.0), theme::TEXT);
    // Breite fest ausrechnen (Einrückung 34, Kartenrand 2x14 + Linie): aus
    // available_width innerhalb der Karte liefen die Karten rechts hinaus.
    let innen = ui.available_width() - 34.0 - 30.0;
    let alt = ui.spacing().indent;
    ui.spacing_mut().indent = 34.0;
    ui.indent(titel, |ui| {
        ui.spacing_mut().indent = alt;
        theme::karte(theme::FLAECHE_1).show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(innen);
                theme::kopfzeile(ui, titel);
                ui.add_space(4.0);
                inhalt(ui);
            });
        });
    });
    ui.spacing_mut().indent = alt;
    ui.add_space(10.0);
}

fn leise(text: &str) -> egui::RichText {
    egui::RichText::new(text).size(11.5).color(theme::TEXT_LEISE)
}

impl SettingsTab {
    pub fn load(&mut self) {
        self.profiles = load_profiles();
    }

    fn speichern(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.profiles) {
            let p = profiles_path();
            let _ = std::fs::create_dir_all(p.parent().unwrap());
            let _ = std::fs::write(&p, json);
        }
    }

    /// Profile in der Seitenleiste. true = ein Profil wurde gewählt oder
    /// angelegt, die App wechselt dann auf diese Seite.
    pub fn profilliste(&mut self, ui: &mut egui::Ui, seite_offen: bool) -> bool {
        let mut gewaehlt = None;
        for (i, p) in self.profiles.iter().enumerate() {
            let (punkt, rechts) = if p.schedule.manual_only {
                (theme::TEXT_LEISE, "manuell".to_string())
            } else {
                (theme::GUT, format!("{:02}:{:02}", p.schedule.hour, p.schedule.minute))
            };
            let aktiv = seite_offen && self.selected == Some(i);
            if theme::eintrag(ui, aktiv, &p.name, &rechts, Some(punkt)).clicked() {
                gewaehlt = Some(i);
            }
        }
        if theme::eintrag(ui, false, "+ Neues Profil", "", None).clicked() {
            self.profiles.push(Profile {
                name: format!("Profil {}", self.profiles.len() + 1), ..Default::default()
            });
            self.speichern();
            gewaehlt = Some(self.profiles.len() - 1);
        }
        match gewaehlt {
            Some(i) => {
                self.selected = Some(i);
                self.form = self.profiles[i].clone();
                self.loeschen_fragen = false;
                true
            }
            None => false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        if let Some(ref rx) = self.drive_auth_rx {
            if let Ok(result) = rx.try_recv() {
                self.drive_auth_rx = None;
                self.status = match result {
                    Ok(()) => "Bei Google angemeldet".into(),
                    Err(e) => format!("Fehler: {e}"),
                };
            }
        }

        let Some(idx) = self.selected else {
            ui.label(leise("Wähle links ein Profil aus oder lege ein neues an."));
            return;
        };

        self.kopf(ui);
        ui.add_space(14.0);
        self.schritt_was(ui);
        self.schritt_wohin(ui);
        self.schritt_wann(ui);
        schritt(ui, 4, "Gelöschte Dateien", |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut self.form.retention_months, 1..=12).text("Monate"));
            });
            ui.label(leise("In der Quelle gelöschte Dateien bleiben so lange im Papierkorb, \
                            danach sind sie endgültig weg."));
        });
        self.loeschen(ui, idx);

        // Automatisch speichern - nur wenn sich wirklich etwas geändert hat.
        if let Some(i) = self.selected {
            if serde_json::to_string(&self.form).ok() != serde_json::to_string(&self.profiles[i]).ok() {
                self.profiles[i] = self.form.clone();
                self.speichern();
            }
        }
    }

    /// Name als große, direkt bearbeitbare Überschrift; rechts der Status.
    fn kopf(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.form.name)
                .font(egui::FontId::proportional(19.0))
                .text_color(theme::TEXT)
                .frame(false)
                .desired_width(ui.available_width() - 230.0))
                .on_hover_text("Name des Profils - zum Ändern hineinklicken");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.status.is_empty() {
                    ui.label(leise("automatisch gespeichert"));
                } else {
                    if ui.add(egui::Button::new(leise("×")).frame(false)).clicked() {
                        self.status.clear();
                    }
                    let farbe = if self.status.starts_with("Fehler") { theme::KRITISCH } else { theme::GUT };
                    ui.add(egui::Label::new(
                        egui::RichText::new(&self.status).size(11.5).color(farbe)).truncate());
                }
            });
        });
        let ziel = if self.form.drive_enabled { "Google Drive" } else { "lokal" };
        ui.label(leise(&format!("{} Quell-Ordner · Ziel {ziel}", self.form.source_paths.len())));
    }

    /// Ein Ordner je Zeile: Name, Pfad, rechts der Schalter Spiegeln | Einweg
    /// und ×. Ben, 27.11.2026: als Wort im Chip sah man nicht, dass es ein
    /// Schalter ist.
    fn schritt_was(&mut self, ui: &mut egui::Ui) {
        let quellen = &mut self.form.source_paths;
        let einweg = &mut self.form.einweg;
        let status = &mut self.status;
        schritt(ui, 1, "Was wird gesichert", |ui| {
            let mut weg = None;
            let mut umschalten = None;
            let mut unter_weg = None;
            let mut unter_neu = None;
            for (i, sp) in quellen.iter().enumerate() {
                if i > 0 {
                    ui.separator();
                }
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.label(egui::RichText::new(kurz(sp)).color(theme::TEXT));
                        ui.add(egui::Label::new(leise(&sp.display().to_string())).truncate());
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(leise("×")).frame(false))
                            .on_hover_text("Ordner entfernen").clicked()
                        {
                            weg = Some(i);
                        }
                        ui.add_space(6.0);
                        let ist_einweg = einweg.contains(sp);
                        if theme::segment(ui, ist_einweg).is_some_and(|neu| neu != ist_einweg) {
                            umschalten = Some(sp.clone());
                        }
                    });
                });

                // Unterordner als Einweg (Ben, 27.11.2026): der Ordner wird
                // gespiegelt, nur die gewaehlten Unterordner - beliebig viele,
                // beliebig tief - behalten lokal Geloeschtes. Steht der ganze
                // Ordner auf Einweg, ist das schon abgedeckt.
                if einweg.contains(sp) {
                    continue;
                }
                for e in einweg.iter().filter(|e| e.starts_with(sp) && *e != sp) {
                    ui.horizontal(|ui| {
                        ui.add_space(18.0);
                        ui.label(egui::RichText::new("Einweg").size(11.0).strong().color(theme::WARNUNG));
                        let rel = e.strip_prefix(sp).map(|r| r.display().to_string()).unwrap_or_default();
                        ui.label(egui::RichText::new(rel).color(theme::TEXT_2))
                            .on_hover_text(e.display().to_string());
                        if ui.add(egui::Button::new(leise("×")).frame(false))
                            .on_hover_text("Unterordner wieder spiegeln").clicked()
                        {
                            unter_weg = Some(e.clone());
                        }
                    });
                }
                ui.horizontal(|ui| {
                    ui.add_space(18.0);
                    if ui.add(egui::Button::new(
                        egui::RichText::new("+ Unterordner als Einweg").size(11.5).color(theme::WARNUNG))
                        .frame(false))
                        .on_hover_text("Ein Unterordner, dessen gelöschte Dateien im Ziel bleiben sollen")
                        .clicked()
                    {
                        if let Some(p) = rfd::FileDialog::new().set_directory(sp).pick_folder() {
                            if p.starts_with(sp) && p != *sp {
                                unter_neu = Some(p);
                            } else {
                                *status = format!("Fehler: {} liegt nicht unter {}", p.display(), sp.display());
                            }
                        }
                    }
                });
            }
            if let Some(e) = unter_weg {
                einweg.retain(|x| *x != e);
            }
            if let Some(p) = unter_neu {
                if !einweg.contains(&p) { einweg.push(p); }
            }
            if let Some(i) = weg {
                let p = quellen.remove(i);
                // Mit dem Ordner gehen auch seine Einweg-Unterordner.
                einweg.retain(|e| !e.starts_with(&p));
            }
            if let Some(p) = umschalten {
                if einweg.contains(&p) { einweg.retain(|e| *e != p); } else { einweg.push(p); }
            }

            ui.add_space(6.0);
            let knopf = egui::Button::new(
                egui::RichText::new("+ Ordner hinzufügen").strong().color(theme::SPIEGEL))
                .fill(theme::FLAECHE_2).corner_radius(7);
            if ui.add(knopf).clicked() {
                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                    quellen.push(p);
                }
            }
            ui.add_space(8.0);
            egui::Grid::new("modi").num_columns(2).spacing([10.0, 3.0]).show(ui, |ui| {
                ui.label(egui::RichText::new("Spiegeln").size(11.5).strong().color(theme::SPIEGEL));
                ui.label(leise("Lokal gelöschte Dateien wandern in den Papierkorb und sind nach der Frist weg."));
                ui.end_row();
                ui.label(egui::RichText::new("Einweg").size(11.5).strong().color(theme::WARNUNG));
                ui.label(leise("Lokal gelöschte Dateien bleiben dauerhaft im Ziel."));
                ui.end_row();
            });
            ui.label(leise("Zurück von Einweg auf Spiegeln: beim nächsten Lauf kommt alles, was es lokal nicht mehr gibt, in den Papierkorb."));
        });
    }

    fn schritt_wohin(&mut self, ui: &mut egui::Ui) {
        let angemeldet = syncie_core::drive_auth::is_authenticated();
        // Die Closure leiht sich Teile von self - deshalb einzeln herausgeholt.
        let form = &mut self.form;
        let status = &mut self.status;
        let ordner = &mut self.drive_folders;
        let auth_rx = &mut self.drive_auth_rx;

        schritt(ui, 2, "Wohin", |ui| {
            ui.horizontal(|ui| {
                let lokal_unter = if form.dest_path.as_os_str().is_empty() {
                    "Ordner wählen".to_string()
                } else {
                    kurz(&form.dest_path)
                };
                if theme::kachel(ui, !form.drive_enabled, "Lokal / USB", &lokal_unter).clicked() {
                    form.drive_enabled = false;
                }
                let drive_unter = if angemeldet { "angemeldet" } else { "nicht angemeldet" };
                if theme::kachel(ui, form.drive_enabled, "Google Drive", drive_unter).clicked() {
                    form.drive_enabled = true;
                }
            });
            ui.add_space(8.0);

            if !form.drive_enabled {
                ui.horizontal(|ui| {
                    ui.label("Ziel-Ordner:");
                    let mut dp = form.dest_path.to_string_lossy().to_string();
                    ui.add(egui::TextEdit::singleline(&mut dp).desired_width(ui.available_width() - 90.0));
                    form.dest_path = PathBuf::from(dp);
                    if ui.button("Wählen …").clicked() {
                        if let Some(p) = rfd::FileDialog::new().pick_folder() {
                            form.dest_path = p;
                        }
                    }
                });
                return;
            }

            if angemeldet {
                ui.horizontal(|ui| {
                    ui.label("Ordner in Drive:");
                    if ordner.is_empty() {
                        let text = if form.drive_folder_id.is_empty() { "keiner gewählt" } else { "gewählt" };
                        ui.label(egui::RichText::new(text).color(theme::TEXT));
                    } else {
                        egui::ComboBox::from_id_salt("drive_folder")
                            .selected_text(ordner.iter()
                                .find(|(i, _)| *i == form.drive_folder_id)
                                .map(|(_, n)| n.as_str())
                                .unwrap_or("— auswählen —"))
                            .show_ui(ui, |ui| {
                                for (id, name) in ordner.iter() {
                                    ui.selectable_value(&mut form.drive_folder_id, id.clone(), name);
                                }
                            });
                    }
                    if ui.button("Ordner laden").clicked() {
                        match syncie_core::dest::drive::list_drive_folders() {
                            Ok(f) => {
                                *status = format!("{} Ordner gefunden", f.len());
                                *ordner = f;
                            }
                            Err(e) => *status = format!("Fehler: {e}"),
                        }
                    }
                });
            }

            egui::CollapsingHeader::new("Eigene Google-Client-ID")
                .default_open(!angemeldet)
                .show(ui, |ui| {
                    ui.label(leise("Einmalig: console.cloud.google.com → Projekt → Drive API aktivieren \
                                    → OAuth-Client-ID (Desktop-App)."));
                    egui::Grid::new("client").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                        ui.label("Client-ID");
                        ui.add(egui::TextEdit::singleline(&mut form.drive_client_id).desired_width(320.0));
                        ui.end_row();
                        ui.label("Client-Secret");
                        ui.add(egui::TextEdit::singleline(&mut form.drive_client_secret)
                            .password(true).desired_width(320.0));
                        ui.end_row();
                    });
                    let kann = !form.drive_client_id.is_empty() && auth_rx.is_none();
                    let text = if angemeldet { "Neu anmelden" } else { "Mit Google anmelden" };
                    if ui.add_enabled(kann, egui::Button::new(text)).clicked() {
                        let cid = form.drive_client_id.clone();
                        let cs = form.drive_client_secret.clone();
                        let (tx, rx) = mpsc::channel();
                        *auth_rx = Some(rx);
                        std::thread::spawn(move || {
                            let result = syncie_core::drive_auth::start_auth(&cid, &cs)
                                .and_then(|a| syncie_core::drive_auth::save_token(&a, &cid, &cs));
                            let _ = tx.send(result);
                        });
                    }
                });
        });
    }

    fn schritt_wann(&mut self, ui: &mut egui::Ui) {
        let form = &mut self.form;
        let status = &mut self.status;
        schritt(ui, 3, "Wann", |ui| {
            let mut geplant = !form.schedule.manual_only;
            ui.checkbox(&mut geplant, "Nach Zeitplan sichern");
            form.schedule.manual_only = !geplant;
            if !geplant {
                ui.label(leise("Läuft nur, wenn du in der Übersicht auf „Sicherung starten\" klickst."));
                return;
            }
            ui.horizontal(|ui| {
                let s = &mut form.schedule;
                for (an, tag) in [(&mut s.mon, "Mo"), (&mut s.tue, "Di"), (&mut s.wed, "Mi"),
                                  (&mut s.thu, "Do"), (&mut s.fri, "Fr"), (&mut s.sat, "Sa"),
                                  (&mut s.sun, "So")] {
                    if ui.selectable_label(*an, tag).clicked() {
                        *an = !*an;
                    }
                }
                ui.add_space(10.0);
                ui.add(egui::DragValue::new(&mut s.hour).range(0..=23).custom_formatter(|v, _| format!("{v:02}")));
                ui.label(":");
                ui.add(egui::DragValue::new(&mut s.minute).range(0..=59).custom_formatter(|v, _| format!("{v:02}")));
                ui.label("Uhr");
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("In Windows-Aufgabenplanung eintragen").clicked() {
                    *status = match syncie_core::schedule::register(form) {
                        Ok(msg) => msg,
                        Err(e) => format!("Fehler: {e}"),
                    };
                }
                ui.label(leise("nach jeder Änderung am Zeitplan"));
            });
        });
    }

    /// Löschen mit Rückfrage - ein Klick allein entfernt kein Profil.
    fn loeschen(&mut self, ui: &mut egui::Ui, idx: usize) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(34.0);
            if !self.loeschen_fragen {
                if ui.add(egui::Button::new(egui::RichText::new("Profil löschen").color(theme::KRITISCH))
                    .frame(false)).clicked()
                {
                    self.loeschen_fragen = true;
                }
                return;
            }
            ui.label(egui::RichText::new(format!("„{}\" wirklich löschen?", self.form.name)).color(theme::TEXT));
            if ui.add(egui::Button::new(egui::RichText::new("Ja, löschen").color(theme::KRITISCH))
                .stroke(egui::Stroke::new(1.0_f32, theme::KRITISCH))).clicked()
            {
                self.profiles.remove(idx);
                self.speichern();
                self.selected = None;
                self.form = Profile::default();
                self.loeschen_fragen = false;
                self.status = "Profil gelöscht".into();
            }
            if ui.button("Abbrechen").clicked() {
                self.loeschen_fragen = false;
            }
        });
    }
}
