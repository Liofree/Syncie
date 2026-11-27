use std::path::PathBuf;
use std::time::SystemTime;
use walkdir::WalkDir;

pub struct ScannedFile {
    /// Zielpfad inkl. Quell-Ordnername als oberste Ebene, z.B. "Fotos/2024/a.jpg"
    pub relative_path: String,
    /// Vollständiger Quellpfad — erspart das Raten über alle source_paths
    pub full_path: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
}

/// Ordnername als Zielpräfix; bei Laufwerks-Wurzeln (D:\) gibt es keinen
/// file_name, dann aus dem Pfad einen dateisystemtauglichen Namen bauen.
pub(crate) fn root_label(root: &PathBuf) -> String {
    root.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| {
            root.to_string_lossy()
                .trim_end_matches(['\\', '/'])
                .replace([':', '\\', '/'], "_")
        })
}

/// Zielpraefix eines Einweg-Ordners, so wie der Scanner die Pfade benennt
/// (Ben, 27.11.2026): ein Quell-Ordner selbst -> "LUNA", ein Unterordner
/// beliebiger Tiefe -> "LUNA/data/videos". None, wenn der Ordner unter
/// keiner Quelle liegt (z.B. Quelle inzwischen entfernt).
pub fn einweg_praefix(quellen: &[PathBuf], ordner: &std::path::Path) -> Option<String> {
    let quelle = quellen.iter()
        .filter(|q| ordner.starts_with(q))
        .max_by_key(|q| q.components().count())?;
    let rel = ordner.strip_prefix(quelle).ok()?.to_string_lossy().replace('\\', "/");
    let label = root_label(quelle);
    Some(if rel.is_empty() { label } else { format!("{label}/{rel}") })
}

/// Ein Muster trifft entweder den GANZEN Namen (".git") oder, mit '*'
/// davor, sein Ende ("*token.json" fasst auch spotify_token.json).
///
/// Vorher war es ein contains-Match, und das schluckte still mit: ".git"
/// warf auch .gitignore und .gitattributes weg, "target" jede Datei mit
/// dem Wort im Namen.
fn ausgeschlossen(name: &str, exclude: &[String]) -> bool {
    exclude.iter().any(|pat| match pat.strip_prefix('*') {
        Some(ende) => name.ends_with(ende),
        None => name == pat,
    })
}

pub fn scan(paths: &[PathBuf], exclude: &[String]) -> Vec<ScannedFile> {
    let mut files = Vec::new();
    for root in paths {
        if !root.exists() {
            continue;
        }
        let label = root_label(root);
        let walker = WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !ausgeschlossen(&e.file_name().to_string_lossy(), exclude));
        for entry in walker.flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(root)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .replace('\\', "/");
            let meta = entry.metadata().unwrap();
            let size = meta.len();
            let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            // Bewusst NICHT gelesen: der Vergleich laeuft ueber Groesse und
            // Aenderungszeit (siehe zustand.rs). Frueher wurde hier jede
            // Datei komplett eingelesen und gehasht - bei jedem Lauf, und
            // wegen der zwei Bereiche sogar zweimal.
            files.push(ScannedFile {
                relative_path: format!("{label}/{rel}"),
                full_path: entry.path().to_path_buf(),
                size,
                modified,
            });
        }
    }
    files
}
