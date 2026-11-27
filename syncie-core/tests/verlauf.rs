// Lauf-Verlauf (Ben, 27.11.2026): jeder Lauf landet im Verlauf, auch ein
// gescheiterter; die Meldung bleibt ohne Einstellung aus.
use std::fs;
use std::sync::atomic::AtomicBool;
use syncie_core::profile::Profile;
use syncie_core::verlauf::{self, Melden};

#[test]
fn laeufe_werden_mitgeschrieben_und_gekuerzt() {
    let tmp = std::env::temp_dir().join(format!("syncie_verlauf_{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    // Nie ins echte APPDATA.
    std::env::set_var("APPDATA", tmp.join("appdata"));
    std::env::set_var("HOME", tmp.join("appdata"));

    let quelle = tmp.join("Q");
    fs::create_dir_all(&quelle).unwrap();
    fs::write(quelle.join("a.txt"), "a").unwrap();
    let p = Profile {
        name: "VL".into(),
        source_paths: vec![quelle.clone()],
        dest_path: tmp.join("Ziel"),
        ..Default::default()
    };
    let cancel = AtomicBool::new(false);

    assert_eq!(verlauf::einstellungen_lesen().melden, Melden::Aus, "Standard: keine Meldung");

    syncie_core::run_backup(&p, &cancel).unwrap();
    fs::remove_dir_all(&quelle).unwrap();
    assert!(syncie_core::run_backup(&p, &cancel).is_err());

    let l = verlauf::lesen();
    assert_eq!(l.len(), 2);
    assert_eq!((l[0].profil.as_str(), l[0].kopiert, l[0].fehler.is_none()), ("VL", 1, true));
    assert!(l[1].fehler.is_some(), "fehlende Quelle muss als Fehler im Verlauf stehen");
    assert!(l[1].hat_probleme() && !l[0].hat_probleme());

    for _ in 0..verlauf::BEHALTEN + 5 {
        verlauf::eintragen(&l[0]);
    }
    assert_eq!(verlauf::lesen().len(), verlauf::BEHALTEN, "Verlauf bleibt begrenzt");

    // Einstellung ueberlebt Speichern und Laden.
    verlauf::einstellungen_speichern(&verlauf::Einstellungen { melden: Melden::NurProbleme, ..Default::default() });
    assert_eq!(verlauf::einstellungen_lesen().melden, Melden::NurProbleme);

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn wann_gemeldet_wird() {
    use verlauf::soll_melden;
    assert!(!soll_melden(Melden::Aus, true));
    assert!(!soll_melden(Melden::NurProbleme, false));
    assert!(soll_melden(Melden::NurProbleme, true));
    assert!(soll_melden(Melden::Immer, false));
}
