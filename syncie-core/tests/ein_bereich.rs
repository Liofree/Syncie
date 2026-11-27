use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use syncie_core::profile::{Profile, Schedule};

fn schreibe(p: &PathBuf, inhalt: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, inhalt).unwrap();
}

#[test]
fn beide_quellordner_landen_im_spiegel() {
    let tmp = std::env::temp_dir().join(format!("syncie_test_{}", std::process::id()));
    // Zustand, Sperre und Warnungen nie ins echte APPDATA (27.11.2026).
    std::env::set_var("APPDATA", tmp.join("appdata"));
    std::env::set_var("HOME", tmp.join("appdata"));
    let _ = fs::remove_dir_all(&tmp);
    let q1 = tmp.join("Quelle_A");
    let q2 = tmp.join("Quelle_B");
    let ziel = tmp.join("Ziel");

    schreibe(&q1.join("eins.txt"), "A1");
    schreibe(&q1.join("unter/zwei.txt"), "A2");
    schreibe(&q2.join("drei.txt"), "B1");
    // Gleicher Dateiname in beiden Quellen - darf sich NICHT ueberschreiben
    schreibe(&q2.join("eins.txt"), "B-anders");

    let profil = Profile {
        name: "Test".into(),
        source_paths: vec![q1.clone(), q2.clone()],
        dest_path: ziel.clone(),
        retention_months: 6,
        schedule: Schedule::default(),
        drive_enabled: false,
        drive_folder_id: String::new(),
        drive_client_id: String::new(),
        drive_client_secret: String::new(),
        einweg: vec![],
    };

    let cancel = AtomicBool::new(false);
    let n = syncie_core::run_backup(&profil, &cancel).expect("Backup fehlgeschlagen");
    println!("kopiert insgesamt: {}", n.kopiert);

    let basis = ziel.join("Test");
    for datei in ["Quelle_A/eins.txt", "Quelle_A/unter/zwei.txt",
                  "Quelle_B/drei.txt", "Quelle_B/eins.txt"] {
        let p = basis.join("Spiegel").join(datei);
        assert!(p.exists(), "FEHLT: {datei}");
    }
    assert!(!basis.join("Archiv").exists(), "Der zweite Bereich ist Geschichte");
    // Namensgleiche Dateien bleiben getrennt
    assert_eq!(fs::read_to_string(basis.join("Spiegel/Quelle_A/eins.txt")).unwrap(), "A1");
    assert_eq!(fs::read_to_string(basis.join("Spiegel/Quelle_B/eins.txt")).unwrap(), "B-anders");
    println!("OK: beide Quellen im Spiegel, keine Kollision");

    // --- zweiter Lauf: eine Datei geloescht ---
    fs::remove_file(q1.join("eins.txt")).unwrap();
    syncie_core::run_backup(&profil, &cancel).expect("zweiter Lauf fehlgeschlagen");

    assert!(!basis.join("Spiegel/Quelle_A/eins.txt").exists(),
            "die geloeschte Datei darf nicht mehr im Spiegel stehen");
    assert!(basis.join("Spiegel/.syncie_trash/Quelle_A/eins.txt").exists(),
            "Papierkorb muesste die Datei aufbewahren");
    println!("OK: aus dem Spiegel raus, im Papierkorb erhalten");

    // --- Warnung: Frist kuenstlich fast abgelaufen ---
    // 175 Tage alt, Aufbewahrung 6 Monate (180 Tage) -> noch nicht faellig,
    // aber innerhalb der 7-Tage-Vorwarnung.
    let index = basis.join("Spiegel/.syncie_trash.json");
    let alt = (chrono::Utc::now() - chrono::Duration::days(175))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    fs::write(&index, format!(
        "{{\"Quelle_A/eins.txt\":{{\"deleted_at\":\"{alt}\",\"size\":2}}}}")).unwrap();

    let e = syncie_core::run_backup(&profil, &cancel).expect("dritter Lauf fehlgeschlagen");
    assert_eq!(e.warnungen.len(), 1, "genau eine Datei muesste warnen");
    assert_eq!(e.warnungen[0].pfad, "Quelle_A/eins.txt");
    assert!(e.warnungen[0].tage <= 7 && e.warnungen[0].tage >= 0,
            "Restfrist muesste 0..7 Tage sein, ist {}", e.warnungen[0].tage);
    println!("OK: Warnung {} Tage vor dem endgueltigen Loeschen",
             e.warnungen[0].tage);

    // --- Zurueckholen: Datei landet wieder im QUELLordner ---
    assert!(!q1.join("eins.txt").exists(), "Vorbedingung: lokal geloescht");
    let meldung = syncie_core::papierkorb::zurueckholen(&profil, "Quelle_A/eins.txt")
        .expect("Zurueckholen fehlgeschlagen");
    println!("{meldung}");
    assert_eq!(fs::read_to_string(q1.join("eins.txt")).unwrap(), "A1",
               "Inhalt muss exakt stimmen - der SYNCIE_TRASH-Kopf muss ab sein");
    assert!(!basis.join("Spiegel/.syncie_trash/Quelle_A/eins.txt").exists(),
            "nach dem Zurueckholen darf die Papierkorb-Kopie weg sein");

    // Vorhandene lokale Datei wird NIE ueberbuegelt.
    fs::write(q1.join("zwei_neu.txt"), "neu").unwrap();
    let fehler = syncie_core::papierkorb::zurueckholen(&profil, "Quelle_A/zwei_neu.txt");
    assert!(fehler.is_err(), "existierende lokale Datei darf nicht ueberschrieben werden");
    assert_eq!(fs::read_to_string(q1.join("zwei_neu.txt")).unwrap(), "neu");
    println!("OK: zurueckgeholt, Kopf entfernt, nichts ueberschrieben");

    let _ = fs::remove_dir_all(&tmp);
}
