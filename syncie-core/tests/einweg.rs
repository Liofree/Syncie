// Einweg-Sicherung je Ordner (Ben, 27.11.2026): lokal Geloeschtes bleibt im
// Ziel, waehrend ein Spiegel-Ordner im selben Profil sich verhaelt wie immer.
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use syncie_core::profile::Profile;

// Beide Tests setzen APPDATA und nehmen die Lauf-Sperre - parallel im selben
// Prozess kaemen sie sich in die Quere. Deshalb nacheinander.
static NACHEINANDER: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn schreibe(p: &PathBuf, inhalt: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, inhalt).unwrap();
}

#[test]
fn einweg_behaelt_geloeschtes_spiegel_wirft_es_in_den_papierkorb() {
    let _reihe = NACHEINANDER.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = std::env::temp_dir().join(format!("syncie_einweg_{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    // Zustand, Sperre und Statusdateien nie ins echte APPDATA.
    std::env::set_var("APPDATA", tmp.join("appdata"));
    std::env::set_var("HOME", tmp.join("appdata"));

    let einweg = tmp.join("Fotos");
    let spiegel = tmp.join("Arbeit");
    schreibe(&einweg.join("a.jpg"), "foto");
    schreibe(&spiegel.join("b.txt"), "text");

    let p = Profile {
        name: "EW".into(),
        source_paths: vec![einweg.clone(), spiegel.clone()],
        dest_path: tmp.join("Ziel"),
        einweg: vec![einweg.clone()],
        ..Default::default()
    };
    let cancel = AtomicBool::new(false);
    assert_eq!(syncie_core::run_backup(&p, &cancel).unwrap().kopiert, 2);

    fs::remove_file(einweg.join("a.jpg")).unwrap();
    fs::remove_file(spiegel.join("b.txt")).unwrap();
    syncie_core::run_backup(&p, &cancel).unwrap();

    let basis = tmp.join("Ziel/EW/Spiegel");
    assert!(basis.join("Fotos/a.jpg").exists(), "Einweg: Datei muss im Ziel bleiben");
    assert!(!basis.join(".syncie_trash/Fotos/a.jpg").exists(), "Einweg: nicht in den Papierkorb");
    assert!(!basis.join("Arbeit/b.txt").exists(), "Spiegel: Datei verschwindet aus dem Ziel");
    assert!(basis.join(".syncie_trash/Arbeit/b.txt").exists(), "Spiegel: Datei liegt im Papierkorb");

    // Zurueck auf Spiegeln: jetzt greift der Papierkorb auch fuer den alten Einweg-Ordner.
    let p = Profile { einweg: vec![], ..p };
    syncie_core::run_backup(&p, &cancel).unwrap();
    assert!(!basis.join("Fotos/a.jpg").exists());
    assert!(basis.join(".syncie_trash/Fotos/a.jpg").exists());

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn unterordner_einweg_in_gespiegeltem_ordner() {
    let _reihe = NACHEINANDER.lock().unwrap_or_else(|e| e.into_inner());
    // Ben, 27.11.2026: LUNA wird gespiegelt, nur Videos (und ein tiefer
    // Archivordner) bleiben einweg.
    let tmp = std::env::temp_dir().join(format!("syncie_einweg_sub_{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    std::env::set_var("APPDATA", tmp.join("appdata"));
    std::env::set_var("HOME", tmp.join("appdata"));

    let luna = tmp.join("LUNA");
    schreibe(&luna.join("notiz.txt"), "n");
    schreibe(&luna.join("Videos/clip.mp4"), "v");
    schreibe(&luna.join("Videos/2025/alt.mp4"), "a");
    schreibe(&luna.join("data/archiv/2025/x.csv"), "x");
    schreibe(&luna.join("data/y.csv"), "y");

    let p = Profile {
        name: "SUB".into(),
        source_paths: vec![luna.clone()],
        dest_path: tmp.join("Ziel"),
        einweg: vec![luna.join("Videos"), luna.join("data").join("archiv")],
        ..Default::default()
    };
    let cancel = AtomicBool::new(false);
    assert_eq!(syncie_core::run_backup(&p, &cancel).unwrap().kopiert, 5, "jede Datei genau einmal");

    for f in ["notiz.txt", "Videos/clip.mp4", "Videos/2025/alt.mp4", "data/archiv/2025/x.csv", "data/y.csv"] {
        fs::remove_file(luna.join(f)).unwrap();
    }
    syncie_core::run_backup(&p, &cancel).unwrap();

    let b = tmp.join("Ziel/SUB/Spiegel");
    for bleibt in ["LUNA/Videos/clip.mp4", "LUNA/Videos/2025/alt.mp4", "LUNA/data/archiv/2025/x.csv"] {
        assert!(b.join(bleibt).exists(), "Einweg-Unterordner: {bleibt} muss bleiben");
        assert!(!b.join(".syncie_trash").join(bleibt).exists());
    }
    for weg in ["LUNA/notiz.txt", "LUNA/data/y.csv"] {
        assert!(!b.join(weg).exists(), "Spiegel: {weg} muss raus");
        assert!(b.join(".syncie_trash").join(weg).exists(), "Spiegel: {weg} im Papierkorb");
    }
    // "Videos2" darf nicht als Teil von "Videos" gelten.
    assert_eq!(syncie_core::scanner::einweg_praefix(&[luna.clone()], &luna.join("Videos")).as_deref(),
               Some("LUNA/Videos"));
    assert_eq!(syncie_core::scanner::einweg_praefix(&[luna.clone()], &tmp.join("anders")), None);

    let _ = fs::remove_dir_all(&tmp);
}
