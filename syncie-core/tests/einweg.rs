// Einweg-Sicherung je Ordner (Ben, 27.11.2026): lokal Geloeschtes bleibt im
// Ziel, waehrend ein Spiegel-Ordner im selben Profil sich verhaelt wie immer.
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use syncie_core::profile::Profile;

fn schreibe(p: &PathBuf, inhalt: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, inhalt).unwrap();
}

#[test]
fn einweg_behaelt_geloeschtes_spiegel_wirft_es_in_den_papierkorb() {
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
