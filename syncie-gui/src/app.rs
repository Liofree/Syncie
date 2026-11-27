use crate::dashboard::DashboardTab;
use crate::settings::SettingsTab;
use crate::verlauf::VerlaufTab;
use crate::theme;

pub struct App {
    tab: Tab,
    dashboard: DashboardTab,
    settings: SettingsTab,
    verlauf: VerlaufTab,
    /// Das Aussehen wird EINMAL gesetzt, nicht in jedem Bild neu.
    /// set_visuals in update() baute den Stil 60-mal je Sekunde neu auf.
    stil_gesetzt: bool,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Tab { Dashboard, Settings, Verlauf, Ueber }

impl Default for App {
    fn default() -> Self {
        let mut app = Self {
            tab: Tab::Dashboard,
            dashboard: DashboardTab::default(),
            settings: SettingsTab::default(),
            verlauf: VerlaufTab::default(),
            stil_gesetzt: false,
        };
        app.settings.load();
        app
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.stil_gesetzt {
            theme::anwenden(ctx);
            self.stil_gesetzt = true;
        }

        // Ben, 27.11.2026: Die Auswahl steht links - Navigation und Profile
        // in einer Seitenleiste, rechts nur der Inhalt.
        egui::SidePanel::left("seite")
            .exact_width(210.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::FLAECHE_1)
                    .stroke(egui::Stroke::new(1.0_f32, theme::LINIE))
                    .inner_margin(egui::Margin::symmetric(10, 14)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    // Markenzeichen: Ring in der Spiegel-Farbe.
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(16.0, 16.0), egui::Sense::hover());
                    ui.painter().circle_stroke(
                        rect.center(), 6.5, egui::Stroke::new(2.0_f32, theme::SPIEGEL));
                    ui.label(
                        egui::RichText::new("Syncie").size(15.0).strong().color(theme::TEXT));
                });
                ui.add_space(16.0);

                if theme::eintrag(ui, self.tab == Tab::Dashboard, "Übersicht", "", None).clicked() {
                    self.tab = Tab::Dashboard;
                }
                if theme::eintrag(ui, self.tab == Tab::Verlauf, "Verlauf", "", None).clicked() {
                    self.tab = Tab::Verlauf;
                }
                ui.add_space(14.0);
                ui.horizontal(|ui| { ui.add_space(10.0); theme::kopfzeile(ui, "Profile"); });
                ui.add_space(2.0);
                if self.settings.profilliste(ui, self.tab == Tab::Settings) {
                    self.tab = Tab::Settings;
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    let rechts = concat!("v", env!("CARGO_PKG_VERSION"));
                    if theme::eintrag(ui, self.tab == Tab::Ueber, "Über Syncie", rechts, None).clicked() {
                        self.tab = Tab::Ueber;
                    }
                    ui.add_space(8.0);
                    // Spendenknopf auf jeder Seite (Ben, 27.11.2026). Ohne
                    // Link grau und ohne Wirkung - der Link kommt in ueber.rs.
                    let url = crate::ueber::SPENDE_URL;
                    let knopf = egui::Button::new(
                        egui::RichText::new("☕  Kaffee spendieren").strong()
                            .color(if url.is_empty() { theme::TEXT_LEISE } else { theme::FLAECHE_0 }))
                        .fill(if url.is_empty() { theme::FLAECHE_2 } else { theme::WARNUNG })
                        .corner_radius(theme::RUNDUNG)
                        .min_size(egui::vec2(ui.available_width(), 32.0));
                    if ui.add_enabled(!url.is_empty(), knopf)
                        .on_disabled_hover_text("Spendenlink folgt")
                        .clicked()
                    {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                    }
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::FLAECHE_0)
                    .inner_margin(egui::Margin::symmetric(24, 20)),
            )
            .show(ctx, |ui| {
                // Breite VOR der ScrollArea festhalten: sonst merkt sie sich
                // eine einmal zu breite Zeile und alle Karten laufen rechts raus.
                let breite = ui.available_width();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.set_max_width(breite);
                    match self.tab {
                        Tab::Dashboard => self.dashboard.show(ui),
                        Tab::Settings => self.settings.show(ui),
                        Tab::Verlauf => self.verlauf.show(ui),
                        Tab::Ueber => crate::ueber::show(ui),
                    }
                });
            });
    }
}
