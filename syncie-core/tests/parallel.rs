// Parallele Uploads (Ben, 19.10.2026): alles kommt an, neue Dateien gehen
// ueber write_neu (eine Drive-Anfrage), vorhandene ueber write, und es
// laufen wirklich mehrere gleichzeitig.
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use async_trait::async_trait;
use syncie_core::dest::Destination;

#[derive(Default)]
struct Speicher {
    dateien: Mutex<HashMap<String, Vec<u8>>>,
    neu: AtomicUsize,
    alt: AtomicUsize,
    gleichzeitig: AtomicUsize,
    hoechstens: AtomicUsize,
}

impl Speicher {
    async fn ablegen(&self, p: &str, d: Vec<u8>) {
        let jetzt = self.gleichzeitig.fetch_add(1, Ordering::SeqCst) + 1;
        self.hoechstens.fetch_max(jetzt, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        self.dateien.lock().unwrap().insert(p.into(), d);
        self.gleichzeitig.fetch_sub(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl Destination for Speicher {
    async fn write(&self, p: &str, d: Vec<u8>) -> Result<(), String> {
        self.alt.fetch_add(1, Ordering::SeqCst); self.ablegen(p, d).await; Ok(())
    }
    async fn write_neu(&self, p: &str, d: Vec<u8>) -> Result<(), String> {
        self.neu.fetch_add(1, Ordering::SeqCst); self.ablegen(p, d).await; Ok(())
    }
    async fn read(&self, p: &str) -> Result<Vec<u8>, String> {
        self.dateien.lock().unwrap().get(p).cloned().ok_or("fehlt".into())
    }
    async fn delete(&self, p: &str) -> Result<(), String> {
        self.dateien.lock().unwrap().remove(p); Ok(())
    }
    async fn list(&self, _: &str) -> Result<Vec<String>, String> {
        Ok(self.dateien.lock().unwrap().keys().cloned().collect())
    }
}

#[tokio::test]
async fn viele_dateien_parallel_und_eine_anfrage_fuer_neue() {
    let tmp = std::env::temp_dir().join(format!("syncie_par_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let q = tmp.join("Q");
    std::fs::create_dir_all(q.join("a")).unwrap();
    for i in 0..1200 { std::fs::write(q.join(format!("a/{i}.txt")), format!("{i}")).unwrap(); }

    let ziel = Speicher::default();
    // eine Datei liegt schon im Ziel -> muss ueber write (mit Suche) gehen
    ziel.dateien.lock().unwrap().insert("Q/a/0.txt".into(), b"alt".to_vec());

    std::env::set_var("APPDATA", tmp.join("appdata"));
    let fort = syncie_core::fortschritt::Fortschritt::neu("Par");
    let mut zustand = syncie_core::zustand::Zustand::laden("Par");
    let cancel = AtomicBool::new(false);
    let (kopiert, _, ueb) = syncie_core::protect::protect(
        &[q.clone()], &ziel, 6, &[], &[], &cancel, &fort, &mut zustand).await.unwrap();

    assert_eq!(kopiert, 1200);
    assert!(ueb.is_empty());
    assert_eq!(ziel.neu.load(Ordering::SeqCst), 1199);
    // die vorhandene Datei + der Papierkorb-Index am Ende
    assert_eq!(ziel.alt.load(Ordering::SeqCst), 2);
    assert_eq!(ziel.dateien.lock().unwrap().get("Q/a/7.txt").unwrap(), b"7");
    assert!(ziel.hoechstens.load(Ordering::SeqCst) > 1, "laeuft nicht parallel");
    // Zwischenstand liegt schon auf der Platte, bevor der Lauf ihn am Ende speichert
    assert!(tmp.join("appdata/syncie/zustand-Par.json").exists());
    let _ = std::fs::remove_dir_all(&tmp);
}
