#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod theme;
mod dashboard;
mod settings;
mod verlauf;
mod ueber;

/// Fenster- und Taskleistenzeichen: derselbe Ring wie in der Kopfzeile,
/// gerechnet statt als Bilddatei - ohne Icon zeigt eframe sein eigenes
/// "e", das mit Syncie nichts zu tun hat (Ben, 23.08.2026).
fn zeichen() -> egui::IconData {
    const K: usize = 64;
    let [r, g, b, _] = theme::SPIEGEL.to_array();
    let mitte = (K as f32 - 1.0) / 2.0;
    let (radius, dicke) = (22.0_f32, 7.0_f32);
    let mut rgba = vec![0u8; K * K * 4];
    for y in 0..K {
        for x in 0..K {
            let abstand = ((x as f32 - mitte).powi(2) + (y as f32 - mitte).powi(2)).sqrt();
            // Weicher Rand ueber eine Pixelbreite: ohne ihn ist der Ring in
            // den 16 Pixeln der Taskleiste eine ausgefranste Treppe.
            let deckung = (1.0 - ((abstand - radius).abs() - dicke / 2.0)).clamp(0.0, 1.0);
            let i = (y * K + x) * 4;
            rgba[i] = r;
            rgba[i + 1] = g;
            rgba[i + 2] = b;
            rgba[i + 3] = (deckung * 255.0) as u8;
        }
    }
    egui::IconData { rgba, width: K as u32, height: K as u32 }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 680.0])
            .with_min_inner_size([760.0, 480.0])
            .with_title("Syncie Backup")
            .with_icon(zeichen()),
        ..Default::default()
    };
    eframe::run_native(
        "Syncie",
        options,
        Box::new(|_cc| Ok(Box::new(app::App::default()))),
    )
}
