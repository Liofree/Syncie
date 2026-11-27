// Jeder Text, den die Oberflaeche mit t("...") zeigt, hat eine englische
// und spanische Fassung - und dieselben Platzhalter (Ben, 27.11.2026).
use syncie_core::sprache::TEXTE;

/// Alle t("...")-Literale einer Quelldatei, unescaped.
fn schluessel(quelle: &str) -> Vec<String> {
    let b = quelle.as_bytes();
    let mut aus = Vec::new();
    let mut i = 0;
    while let Some(pos) = quelle[i..].find("t(") {
        let start = i + pos;
        i = start + 2;
        if start > 0 && (b[start - 1].is_ascii_alphanumeric() || b[start - 1] == b'_' || b[start - 1] == b'.') {
            continue;
        }
        let rest = quelle[i..].trim_start();
        if !rest.starts_with('"') {
            continue;
        }
        let mut s = String::new();
        let mut zeichen = rest[1..].chars();
        while let Some(c) = zeichen.next() {
            match c {
                '"' => break,
                '\\' => match zeichen.next() {
                    Some('n') => s.push('\n'),
                    Some(x) => s.push(x),
                    None => break,
                },
                _ => s.push(c),
            }
        }
        aus.push(s);
    }
    aus
}

fn platzhalter(s: &str) -> Vec<String> {
    let mut v: Vec<String> = s.split('{').skip(1).filter_map(|t| t.split_once('}')).map(|(p, _)| p.to_string()).collect();
    v.sort();
    v
}

#[test]
fn jeder_text_ist_uebersetzt() {
    let wurzel = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut fehlend = Vec::new();
    let mut gesehen = 0;
    for ordner in ["syncie-gui/src", "syncie-core/src", "syncie-cli/src"] {
        for datei in std::fs::read_dir(wurzel.join(ordner)).unwrap().flatten() {
            let p = datei.path();
            if p.extension().is_some_and(|e| e == "rs") && !p.ends_with("sprache.rs") {
                for k in schluessel(&std::fs::read_to_string(&p).unwrap()) {
                    gesehen += 1;
                    if !TEXTE.iter().any(|z| z.0 == k) {
                        fehlend.push(format!("{}: {k}", p.display()));
                    }
                }
            }
        }
    }
    // Dynamisch uebersetzt: Bereichsname und Spaltenkoepfe im Verlauf.
    for k in [syncie_core::BEREICH, "Start", "Profil", "Dauer", "Gesichert", "Gesperrt", "Ergebnis"] {
        if !TEXTE.iter().any(|z| z.0 == k) {
            fehlend.push(k.to_string());
        }
    }
    assert!(gesehen > 100, "Parser findet zu wenig t(...)-Aufrufe: {gesehen}");
    assert!(fehlend.is_empty(), "Ohne Uebersetzung:\n{}", fehlend.join("\n"));
}

#[test]
fn uebersetzungen_vollstaendig() {
    let mut doppelt = std::collections::HashSet::new();
    for (de, en, es) in TEXTE {
        assert!(doppelt.insert(de), "doppelt: {de}");
        assert!(!es.is_empty(), "leer (ES): {de}");
        assert!(!en.is_empty() || *de == "Uhr", "leer (EN): {de}");
        assert_eq!(platzhalter(de), platzhalter(en), "Platzhalter EN: {de}");
        assert_eq!(platzhalter(de), platzhalter(es), "Platzhalter ES: {de}");
    }
}
