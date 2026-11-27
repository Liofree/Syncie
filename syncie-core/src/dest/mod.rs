pub mod local;
pub mod drive;

use async_trait::async_trait;

#[async_trait]
pub trait Destination: Send + Sync {
    async fn write(&self, path: &str, data: Vec<u8>) -> Result<(), String>;
    async fn read(&self, path: &str) -> Result<Vec<u8>, String>;
    async fn delete(&self, path: &str) -> Result<(), String>;
    async fn list(&self, prefix: &str) -> Result<Vec<String>, String>;
    /// Wie write, aber der Aufrufer weiss aus der Ziel-Liste: die Datei
    /// liegt dort noch nicht. Drive spart sich damit die Suche nach einer
    /// vorhandenen Datei - eine Anfrage statt zwei (Ben, 19.10.2026).
    async fn write_neu(&self, path: &str, data: Vec<u8>) -> Result<(), String> {
        self.write(path, data).await
    }
}

/// Legt einen Unterordner ueber ein bestehendes Ziel (Ben, 05.08.2026).
///
/// Damit teilen sich Spiegel und Archiv EIN Ziel, ohne dass Mirror oder
/// Protect etwas davon wissen muessen: Beide arbeiten weiter mit Pfaden
/// relativ zu ihrem eigenen Bereich, der Wrapper setzt den Ordner davor
/// und schneidet ihn beim Auflisten wieder ab.
///
/// Der Filter in `list` ist NICHT nur Kosmetik: Ohne ihn saehe der
/// Spiegel-Durchlauf die Dateien des Archivs und wuerde sie als geloescht
/// behandeln.
pub struct PrefixDest<'a> {
    inner: &'a (dyn Destination + 'a),
    prefix: String,
}

impl<'a> PrefixDest<'a> {
    pub fn new(inner: &'a (dyn Destination + 'a), prefix: &str) -> Self {
        Self { inner, prefix: prefix.trim_matches('/').to_string() }
    }

    fn full(&self, path: &str) -> String {
        let p = path.trim_start_matches('/');
        if p.is_empty() {
            self.prefix.clone()
        } else {
            format!("{}/{}", self.prefix, p)
        }
    }
}

#[async_trait]
impl<'a> Destination for PrefixDest<'a> {
    async fn write(&self, path: &str, data: Vec<u8>) -> Result<(), String> {
        self.inner.write(&self.full(path), data).await
    }

    async fn write_neu(&self, path: &str, data: Vec<u8>) -> Result<(), String> {
        self.inner.write_neu(&self.full(path), data).await
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        self.inner.read(&self.full(path)).await
    }

    async fn delete(&self, path: &str) -> Result<(), String> {
        self.inner.delete(&self.full(path)).await
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>, String> {
        let voll = self.full(prefix);
        let liste = self.inner.list(&voll).await?;
        let schnitt = format!("{}/", self.prefix);
        Ok(liste
            .into_iter()
            .filter_map(|p| p.strip_prefix(&schnitt).map(|s| s.to_string()))
            .collect())
    }
}
