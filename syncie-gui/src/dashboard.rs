use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use chrono::Local;
use crate::theme;

pub struct DashboardTab {
    running: bool,
    status: String,
    last_run: Option<String>,
    result_rx: Option<mpsc::Receiver<Result<(usize, Vec<String>, Vec<String>), String>>>,
    /// Dateien, die bald endgueltig aus dem Papierkorb geloescht werden -
    /// wird als Ansage-Karte ueber der Uebersicht gezeigt (Ben, 05.08.2026).
    warnungen: Vec<String>,
    cancel: Arc<AtomicBool>,
}

impl Default for DashboardTab {
    fn default() -> Self {
        // Letzter Lauf aus dem Verlauf - auch einer aus der Aufgabenplanung
        // (Ben, 27.11.2026). Vorher stand hier nach jedem Start nur "Bereit".
        let letzter = syncie_core::verlauf::lesen().pop();
        let status = match &letzter {
            None => "Bereit".into(),
            Some(l) => match (&l.fehler, l.uebersprungen) {
                (Some(f), _) => format!("Fehler: {}: {f}", l.profil),
                (None, 0) => format!("{} Dateien gesichert", l.kopiert),
                (None, k) => format!("{} gesichert · {k} gesperrt, nicht gesichert", l.kopiert),
            },
        };
        Self {
            running: false,
            status,
            last_run: letzter.map(|l| l.start),
            result_rx: None,
            // Warnungen aus dem letzten Lauf - auch aus der Aufgabenplanung,
            // nicht nur aus einem Lauf in diesem Fenster (Ben, 19.10.2026).
            warnungen: syncie_core::papierkorb_status_lesen().iter()
                .map(|e| format!("{} · noch {} Tag(e)", e.pfad, e.tage))
                .collect(),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl DashboardTab {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        if let Some(rx) = &self.result_rx {
            if let Ok(result) = rx.try_recv() {
                self.result_rx = None;
                self.running = false;
                match result {
                    Ok((n, warnungen, gesperrt)) => {
                        // Gesperrte Dateien fehlen im Backup - das gehoert in
                        // die Zeile, die man sowieso liest, nicht nur ins Log.
                        self.status = match gesperrt.len() {
                            0 => format!("{n} Dateien gesichert"),
                            k => format!("{n} gesichert · {k} gesperrt, nicht gesichert"),
                        };
                        self.last_run = Some(Local::now().format("%d.%m.%Y %H:%M").to_string());
                        self.warnungen = warnungen;
                    }
                    Err(e) => self.status = format!("Fehler: {e}"),
                }
            }
        }

        if self.running {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
        }

        let profiles = crate::settings::load_profiles();

        self.statuszeile(ui, &profiles);
        ui.add_space(12.0);
        self.papierkorb_warnung(ui);
        self.kennzahlen(ui, &profiles);
        ui.add_space(16.0);
        self.profilliste(ui, &profiles);
    }

    /// Zustand, Zeitpunkt und die beiden Knöpfe in einer Zeile.
    fn statuszeile(&mut self, ui: &mut egui::Ui, profiles: &[syncie_core::profile::Profile]) {
        // Die Statusfarbe steht NIE allein - der Text daneben trägt die
        // Aussage, die Farbe verstärkt sie nur.
        let (farbe, titel) = if self.running {
            (theme::SPIEGEL, "Sicherung läuft")
        } else if self.status.starts_with("Fehler") {
            (theme::KRITISCH, "Letzter Lauf fehlgeschlagen")
        } else if self.last_run.is_some() {
            (theme::GUT, "Bereit — letzter Lauf ohne Fehler")
        } else {
            (theme::TEXT_LEISE, "Bereit")
        };

        theme::karte(theme::FLAECHE_1).show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 5.0, farbe);

                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(titel).size(14.0).strong().color(theme::TEXT));
                    // Waehrend des Laufs sagt die Datei mehr als der Status:
                    // welches Profil, welcher Bereich, die wievielte Datei.
                    let lage = self.running.then(syncie_core::fortschritt::lesen).flatten();
                    let unter = match (&lage, &self.last_run) {
                        (Some(l), _) if l.gesamt > 0 => format!(
                            "{} · {} · Datei {} von {}", l.profil, l.phase, l.getan, l.gesamt),
                        (Some(l), _) => format!("{} · {} · wird gelesen ...", l.profil, l.phase),
                        (None, Some(lr)) => format!("{} · {}", self.status, lr),
                        (None, None) => self.status.clone(),
                    };
                    ui.label(egui::RichText::new(unter).size(11.5).color(theme::TEXT_LEISE));
                    if let Some(l) = lage.filter(|l| l.gesamt > 0) {
                        ui.add(egui::ProgressBar::new(l.getan as f32 / l.gesamt as f32)
                            .desired_width(240.0)
                            .desired_height(5.0)
                            .fill(theme::SPIEGEL));
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.running {
                        let knopf = egui::Button::new(
                            egui::RichText::new("Abbrechen").strong().color(theme::KRITISCH))
                            .fill(theme::FLAECHE_2)
                            .stroke(egui::Stroke::new(1.0_f32, theme::KRITISCH))
                            .corner_radius(theme::RUNDUNG);
                        if ui.add(knopf).clicked() {
                            self.cancel.store(true, Ordering::Relaxed);
                            self.status = "Wird abgebrochen ...".into();
                        }
                    } else {
                        ui.add_enabled_ui(!profiles.is_empty(), |ui| {
                            let knopf = egui::Button::new(
                                egui::RichText::new("Sicherung starten")
                                    .strong().color(theme::FLAECHE_0))
                                .fill(theme::SPIEGEL)
                                .corner_radius(theme::RUNDUNG);
                            if ui.add(knopf).clicked() {
                                self.running = true;
                                self.status = "Läuft ...".into();
                                self.start_backup();
                            }
                        });
                    }
                });
            });
        });
    }

    /// Die Kennzahlen in EINER Leiste, durch dünne Linien getrennt.
    ///
    /// Ben, 05.08.2026: Vorher waren es drei eigenständige Karten. Weil nur
    /// die erste eine Fußzeile trug, machte egui sie unterschiedlich hoch
    /// und richtete sie mittig aus - sie standen versetzt wie eine Treppe.
    /// Eine gemeinsame Fläche kann gar nicht erst verrutschen, und die
    /// Fußzeile bekommt festen Platz, ob sie belegt ist oder nicht.
    fn kennzahlen(&self, ui: &mut egui::Ui, profiles: &[syncie_core::profile::Profile]) {
        let quellen: usize = profiles.iter().map(|p| p.source_paths.len()).sum();
        let haltbarkeit = profiles.iter().map(|p| p.retention_months).max().unwrap_or(6);
        let geplant = profiles.iter().filter(|p| !p.schedule.manual_only).count();

        let werte: [(&str, String, &str, String); 3] = [
            ("Profile", profiles.len().to_string(), "",
             if geplant > 0 { format!("{geplant} nach Zeitplan") } else { "nur manuell".into() }),
            ("Quell-Ordner", quellen.to_string(), "",
             if profiles.iter().any(|p| p.drive_enabled) { "Ziel Google Drive".into() }
             else { "Ziel lokal".into() }),
            ("Papierkorb", haltbarkeit.to_string(), "Monate",
             "gelöschte Dateien".into()),
        ];

        theme::karte(theme::FLAECHE_1).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.columns(werte.len(), |spalten| {
                for (i, (titel, wert, einheit, fuss)) in werte.iter().enumerate() {
                    let ui = &mut spalten[i];
                    // Trennlinie links neben jeder Spalte außer der ersten.
                    if i > 0 {
                        let r = ui.max_rect();
                        ui.painter().vline(
                            r.left() - 6.0,
                            r.top()..=(r.top() + 46.0),
                            egui::Stroke::new(1.0_f32, theme::LINIE),
                        );
                    }
                    theme::kennzahl(ui, titel, wert, einheit,
                                    Some((theme::TEXT_LEISE, fuss.as_str())));
                }
            });
        });
    }

    fn profilliste(&self, ui: &mut egui::Ui, profiles: &[syncie_core::profile::Profile]) {
        theme::kopfzeile(ui, "Profile");
        ui.add_space(6.0);

        if profiles.is_empty() {
            theme::karte(theme::FLAECHE_1).show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Noch kein Profil. Lege unter „Profile\" eines an.")
                        .color(theme::TEXT_LEISE),
                );
            });
            return;
        }

        for p in profiles {
            theme::karte(theme::FLAECHE_1).show(ui, |ui| {
                ui.set_width(ui.available_width() - 30.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&p.name).size(13.5).strong().color(theme::TEXT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let ziel = if p.drive_enabled { "Google Drive" } else { "Lokal" };
                        ui.label(egui::RichText::new(ziel).size(11.5).color(theme::TEXT_LEISE));
                    });
                });

                let plan = if p.schedule.manual_only {
                    "nur manuell".to_string()
                } else {
                    format!("{:02}:{:02} Uhr", p.schedule.hour, p.schedule.minute)
                };
                ui.label(
                    egui::RichText::new(format!(
                        "{} Quell-Ordner · {} · Papierkorb {} Monate",
                        p.source_paths.len(), plan, p.retention_months))
                        .size(11.5).color(theme::TEXT_LEISE));
            });
            ui.add_space(8.0);
        }
    }

    fn start_backup(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.result_rx = Some(rx);
        self.cancel.store(false, Ordering::Relaxed);
        let cancel = Arc::clone(&self.cancel);
        let profiles = crate::settings::load_profiles();
        std::thread::spawn(move || {
            let mut total = 0usize;
            let mut warnungen: Vec<String> = Vec::new();
            let mut gesperrt: Vec<String> = Vec::new();
            let mut gelaufen: Vec<String> = Vec::new();
            let mut eintraege: Vec<syncie_core::PapierkorbEintrag> = Vec::new();
            for p in &profiles {
                match syncie_core::run_backup(p, &cancel) {
                    Ok(e) => {
                        total += e.kopiert;
                        gesperrt.extend(e.uebersprungen.iter().cloned());
                        gelaufen.push(p.name.clone());
                        for w in &e.warnungen {
                            warnungen.push(format!(
                                "{} · noch {} Tag(e)", w.pfad, w.tage));
                            eintraege.push(syncie_core::PapierkorbEintrag {
                                profil: p.name.clone(), pfad: w.pfad.clone(),
                                tage: w.tage, groesse: w.groesse,
                            });
                        }
                    }
                    Err(e) => { let _ = tx.send(Err(format!("{}: {e}", p.name))); return; }
                }
            }
            // Statusdatei fuer das LUNA-HUD mitschreiben.
            syncie_core::papierkorb_status_aktualisieren(&gelaufen, &eintraege);
            let _ = tx.send(Ok((total, warnungen, gesperrt)));
        });
    }

    /// Ansage ueber der Uebersicht, wenn Dateien bald endgueltig aus dem
    /// Papierkorb verschwinden. Nur sichtbar, wenn es welche gibt - sonst
    /// nimmt sie keinen Platz weg.
    fn papierkorb_warnung(&self, ui: &mut egui::Ui) {
        if self.warnungen.is_empty() {
            return;
        }
        theme::karte(theme::FLAECHE_1).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 5.0, theme::KRITISCH);
                ui.label(egui::RichText::new(format!(
                    "⚠ {} Datei(en) verschwinden bald endgültig aus dem Papierkorb",
                    self.warnungen.len()
                )).size(14.0).strong().color(theme::KRITISCH));
            });
            for w in &self.warnungen {
                ui.label(egui::RichText::new(w).size(11.5).color(theme::TEXT_LEISE));
            }
        });
        ui.add_space(12.0);
    }
}
