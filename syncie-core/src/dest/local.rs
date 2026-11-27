use async_trait::async_trait;
use std::path::PathBuf;
use super::Destination;

pub struct LocalDest {
    root: PathBuf,
}

impl LocalDest {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn full(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }
}

#[async_trait]
impl Destination for LocalDest {
    async fn write(&self, path: &str, data: Vec<u8>) -> Result<(), String> {
        let full = self.full(path);
        if let Some(p) = full.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        tokio::fs::write(&full, data).await.map_err(|e| e.to_string())
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        tokio::fs::read(self.full(path)).await.map_err(|e| e.to_string())
    }

    async fn delete(&self, path: &str) -> Result<(), String> {
        let full = self.full(path);
        if full.exists() {
            tokio::fs::remove_file(full).await.map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Rekursiv — sonst sieht protect() nichts, was in Unterordnern liegt,
    /// und Papierkorb/Aufbewahrung greifen nie.
    async fn list(&self, prefix: &str) -> Result<Vec<String>, String> {
        let full = self.full(prefix);
        if !full.exists() {
            return Ok(vec![]);
        }
        let mut entries = vec![];
        for e in walkdir::WalkDir::new(&full).follow_links(false).into_iter().flatten() {
            if !e.file_type().is_file() {
                continue;
            }
            let rel = e
                .path()
                .strip_prefix(&self.root)
                .unwrap_or(e.path())
                .to_string_lossy()
                .replace('\\', "/");
            entries.push(rel);
        }
        Ok(entries)
    }
}
