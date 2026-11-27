//! Farben und Bausteine der Oberfläche (Ben, 05.08.2026).
//!
//! Alles an EINER Stelle: vorher standen Farbwerte als Literale in den
//! Knöpfen verstreut (`Color32::from_rgb(40,120,200)`, `Color32::YELLOW`),
//! und jede neue Ansicht erfand ihre eigenen.
//!
//! Die beiden Bereichsfarben sind gegen die Kartenfläche geprüft:
//! Farbfehlsichtigkeit ΔE 19,6 · Normalsicht 20,9 · beide über 3:1
//! Kontrast. Die Statusfarben stehen NIE allein — immer mit Wort daneben,
//! weil Grün (fertig) und Aqua (Archiv) sich sonst zu ähnlich sind.

use egui::Color32;

pub const FLAECHE_0: Color32 = Color32::from_rgb(0x14, 0x14, 0x13); // Fenster
pub const FLAECHE_1: Color32 = Color32::from_rgb(0x1a, 0x1a, 0x19); // Karte
pub const FLAECHE_2: Color32 = Color32::from_rgb(0x23, 0x23, 0x22); // erhöht
pub const LINIE: Color32 = Color32::from_rgb(0x33, 0x33, 0x2f);

pub const TEXT: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
pub const TEXT_2: Color32 = Color32::from_rgb(0xc3, 0xc2, 0xb7);
pub const TEXT_LEISE: Color32 = Color32::from_rgb(0x8a, 0x8a, 0x80);

/// Farbe der Sicherung — Statuspunkt, Knopf, Balken, Fensterzeichen.
pub const SPIEGEL: Color32 = Color32::from_rgb(0x39, 0x87, 0xe5);

pub const GUT: Color32 = Color32::from_rgb(0x0c, 0xa3, 0x0c);
/// Markiert Einweg-Ordner (Ausnahme vom Spiegeln).
pub const WARNUNG: Color32 = Color32::from_rgb(0xfa, 0xb2, 0x19);
pub const KRITISCH: Color32 = Color32::from_rgb(0xd0, 0x3b, 0x3b);

pub const RUNDUNG: u8 = 9;

/// Grundeinstellung des Fensters. Einmal beim Start gesetzt.
pub fn anwenden(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = FLAECHE_0;
    visuals.window_fill = FLAECHE_1;
    visuals.extreme_bg_color = FLAECHE_2;
    visuals.override_text_color = Some(TEXT_2);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, LINIE);
    visuals.widgets.inactive.bg_fill = FLAECHE_2;
    visuals.widgets.inactive.weak_bg_fill = FLAECHE_2;
    visuals.widgets.hovered.bg_fill = LINIE;
    visuals.widgets.active.bg_fill = LINIE;
    visuals.selection.bg_fill = SPIEGEL.linear_multiply(0.35);
    // Keine Linie links neben eingerueckten Bereichen (Schritt-Karten).
    visuals.indent_has_left_vline = false;
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    ctx.set_style(style);
}

/// Kleine Überschrift über einem Abschnitt oder einer Kennzahl.
pub fn kopfzeile(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .size(10.0)
            .strong()
            .color(TEXT_LEISE),
    );
}

/// Rahmen für eine Karte — eine Stelle, damit alle gleich aussehen.
pub fn karte(fuellung: Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(fuellung)
        .stroke(egui::Stroke::new(1.0_f32, LINIE))
        .corner_radius(RUNDUNG)
        .inner_margin(egui::Margin::symmetric(14, 12))
}

/// Geteilter Balken: links Spiegel, rechts Archiv.
///
/// Kennzahl: Wert groß, Einheit klein, Beschriftung darüber.
///
/// Titel und Fußzeile werden bei zu wenig Platz GEKÜRZT statt umgebrochen.
/// Ein Umbruch würde die Spalte höher machen als ihre Nachbarn - und genau
/// dieses ungleiche Wachsen ließ die Kennzahlen vorher versetzt stehen.
pub fn kennzahl(ui: &mut egui::Ui, titel: &str, wert: &str, einheit: &str, fuss: Option<(Color32, &str)>) {
    ui.vertical(|ui| {
        ui.add(
            egui::Label::new(
                egui::RichText::new(titel.to_uppercase()).size(10.0).strong().color(TEXT_LEISE),
            )
            .truncate(),
        );
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(egui::RichText::new(wert).size(21.0).strong().color(TEXT));
            if !einheit.is_empty() {
                ui.label(egui::RichText::new(einheit).size(11.0).color(TEXT_LEISE));
            }
        });
        // Immer eine Zeile, auch wenn leer - sonst sind die Spalten
        // unterschiedlich hoch.
        let (farbe, text) = fuss.unwrap_or((TEXT_LEISE, " "));
        ui.add(
            egui::Label::new(egui::RichText::new(text).size(11.0).color(farbe)).truncate(),
        );
    });
}

/// Zeile in der Seitenleiste: Punkt, Text, rechts leise Zusatzangabe.
/// Selbst gezeichnet, damit Auswahl und Hover zur Karte passen und der
/// Text links bündig steht (SelectableLabel zentriert ihn).
pub fn eintrag(ui: &mut egui::Ui, aktiv: bool, text: &str, rechts: &str,
               punkt: Option<Color32>) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 30.0), egui::Sense::click());
    // Fuer Screenreader (und Bedienung per UI-Automation): Name + Zustand.
    resp.widget_info(|| egui::WidgetInfo::selected(
        egui::WidgetType::SelectableLabel, true, aktiv, text));
    let p = ui.painter();
    if aktiv {
        p.rect_filled(rect, 7.0, SPIEGEL.linear_multiply(0.18));
        p.rect_filled(egui::Rect::from_min_size(rect.min, egui::vec2(3.0, rect.height())),
                      1.5, SPIEGEL);
    } else if resp.hovered() {
        p.rect_filled(rect, 7.0, FLAECHE_2);
    }
    let y = rect.center().y;
    let mut x = rect.left() + 12.0;
    if let Some(f) = punkt {
        p.circle_filled(egui::pos2(x + 3.5, y), 3.5, f);
        x += 15.0;
    }
    // ponytail: lange Namen laufen in die Zusatzangabe, kuerzen falls das stoert
    p.text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, text,
           egui::FontId::proportional(13.0), if aktiv { TEXT } else { TEXT_2 });
    if !rechts.is_empty() {
        p.text(egui::pos2(rect.right() - 10.0, y), egui::Align2::RIGHT_CENTER, rechts,
               egui::FontId::proportional(11.5), TEXT_LEISE);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Auswahlkachel (Ziel des Profils): Titel und eine leise Zeile darunter.
pub fn kachel(ui: &mut egui::Ui, aktiv: bool, titel: &str, unter: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(150.0, 52.0), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(
        egui::WidgetType::SelectableLabel, true, aktiv, titel));
    let p = ui.painter();
    let (fuellung, rand) = if aktiv {
        (SPIEGEL.linear_multiply(0.12), SPIEGEL)
    } else if resp.hovered() {
        (FLAECHE_2, LINIE)
    } else {
        (FLAECHE_1, LINIE)
    };
    p.rect(rect, 9.0, fuellung, egui::Stroke::new(1.0_f32, rand), egui::StrokeKind::Inside);
    p.text(rect.center() - egui::vec2(0.0, 8.0), egui::Align2::CENTER_CENTER, titel,
           egui::FontId::proportional(13.0), TEXT);
    p.text(rect.center() + egui::vec2(0.0, 10.0), egui::Align2::CENTER_CENTER, unter,
           egui::FontId::proportional(11.0), if aktiv { TEXT_2 } else { TEXT_LEISE });
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Segment-Schalter „Spiegeln | Einweg" (Ben, 27.11.2026). Liefert die
/// neue Wahl (true = Einweg), wenn ein Teil angeklickt wurde.
pub fn segment(ui: &mut egui::Ui, einweg: bool) -> Option<bool> {
    let mut wahl = None;
    egui::Frame::new()
        .fill(FLAECHE_2)
        .corner_radius(7)
        .inner_margin(egui::Margin::same(2))
        .show(ui, |ui| {
            // Feste Breite und eigene Richtung: in einer right_to_left-Zeile
            // nahm der Rahmen sonst die ganze Restbreite ein.
            ui.allocate_ui_with_layout(egui::vec2(146.0, 24.0),
                                       egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (wert, text, farbe) in [(false, "Spiegeln", SPIEGEL), (true, "Einweg", WARNUNG)] {
                    let an = einweg == wert;
                    let knopf = egui::Button::new(egui::RichText::new(text).size(12.0)
                            .color(if an { FLAECHE_0 } else { TEXT_2 }).strong())
                        .fill(if an { farbe } else { FLAECHE_2 })
                        .corner_radius(5)
                        .min_size(egui::vec2(72.0, 24.0));
                    if ui.add(knopf).clicked() {
                        wahl = Some(wert);
                    }
                }
            });
        });
    wahl
}
