// Prueft den Vertrag zwischen Syncie (schreibt papierkorb.json) und LUNA
// (liest sie in luna_gui.py / syncie_papierkorb_abrufen).
//
// Ein umbenanntes Feld faellt sonst NICHT auf: das HUD zeigt einfach eine
// leere Liste, der Warnknopf bleibt grau, und die Datei verschwindet
// trotzdem. Deshalb sind die Namen hier festgenagelt.
#[test]
fn papierkorb_json_hat_die_felder_die_luna_liest() {
    // Nie in die echte Statusdatei schreiben (27.11.2026: ein Testlauf
    // ersetzte die Warnungen im LUNA-HUD durch foo.txt).
    let tmp = std::env::temp_dir().join("syncie-test-status");
    std::env::set_var("APPDATA", &tmp);
    std::env::set_var("HOME", &tmp);
    let e = vec![syncie_core::PapierkorbEintrag {
        profil: "Drive Backup".into(),
        pfad: "LUNA/data/foo.txt".into(),
        tage: 4,
        groesse: 1234,
    }];
    syncie_core::papierkorb_status_aktualisieren(&["Drive Backup".to_string()], &e);

    let roh = std::fs::read_to_string(syncie_core::papierkorb_status_pfad()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&roh).unwrap();

    assert!(v["stand"].is_string(), "LUNA erwartet 'stand'");
    let liste = v["eintraege"].as_array().expect("LUNA erwartet 'eintraege'");
    assert_eq!(liste.len(), 1);
    for feld in ["profil", "pfad", "tage", "groesse"] {
        assert!(!liste[0][feld].is_null(), "LUNA erwartet Feld '{feld}'");
    }

    // Mischen: ein Lauf mit nur EINEM Profil darf fremde Eintraege nicht
    // wegwerfen - sonst verschwaende im HUD eine echte Warnung.
    syncie_core::papierkorb_status_aktualisieren(&["Anderes".to_string()], &[]);
    assert_eq!(syncie_core::papierkorb_status_lesen().len(), 1,
               "Eintrag eines nicht gelaufenen Profils muss stehenbleiben");
}
