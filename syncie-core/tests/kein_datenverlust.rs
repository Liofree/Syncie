// Die zwei Zusagen, an denen ein Backup haengt (Ben, 23.08.2026):
//   1. Ein Lauf ohne Aenderung kopiert NICHTS - sonst laedt jede Nacht
//      alles neu hoch, und die gemeldete Zahl sagt nichts aus.
//   2. Ein fehlender Quellordner bricht ab. Vorher galt er als leer, und
//      der Papierkorb schluckte das komplette Backup.
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use syncie_core::profile::{Profile, Schedule};

fn schreibe(p: &PathBuf, inhalt: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, inhalt).unwrap();
}

fn profil(name: &str, quellen: Vec<PathBuf>, ziel: PathBuf) -> Profile {
    Profile {
        name: name.into(),
        source_paths: quellen,
        dest_path: ziel,
        retention_months: 6,
        schedule: Schedule::default(),
        drive_enabled: false,
        drive_folder_id: String::new(),
        drive_client_id: String::new(),
        drive_client_secret: String::new(),
        einweg: vec![],
    }
}

#[test]
fn zweiter_lauf_kopiert_nichts_und_fehlende_quelle_bricht_ab() {
    let tmp = std::env::temp_dir().join(format!("syncie_kdv_{}", std::process::id()));
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

    let p = profil("KDV", vec![q1.clone(), q2.clone()], ziel.clone());
    let cancel = AtomicBool::new(false);

    let erst = syncie_core::run_backup(&p, &cancel).unwrap();
    assert_eq!(erst.kopiert, 3, "erster Lauf kopiert alle drei");

    let zweit = syncie_core::run_backup(&p, &cancel).unwrap();
    assert_eq!(zweit.kopiert, 0, "unveraendert = nichts zu tun");

    // Geaenderter Inhalt (andere Groesse) muss durch.
    fs::write(q1.join("eins.txt"), "A1 geaendert").unwrap();
    assert_eq!(syncie_core::run_backup(&p, &cancel).unwrap().kopiert, 1);

    // --- fehlende Quelle ---
    let basis = ziel.join("KDV").join("Spiegel");
    fs::rename(&q2, tmp.join("Quelle_B_weg")).unwrap();
    let fehler = syncie_core::run_backup(&p, &cancel);
    assert!(fehler.is_err(), "fehlender Quellordner muss abbrechen");
    assert!(basis.join("Quelle_A/eins.txt").exists(), "Backup muss unberuehrt bleiben");
    assert!(basis.join("Quelle_B/drei.txt").exists(),
            "nichts darf wegen der fehlenden Platte im Papierkorb landen");
    assert!(!basis.join(".syncie_trash").exists(), "Papierkorb muss leer bleiben");

    let _ = fs::remove_dir_all(&tmp);
}
