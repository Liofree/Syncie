use chrono::{Utc, Duration};
use crate::dest::Destination;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VersionEntry {
    hash: String,
    ts: chrono::DateTime<Utc>,
}

pub async fn cleanup(dest: &dyn Destination, retention_months: u32) -> Result<usize, String> {
    let data = dest.read("index.json").await.unwrap_or_else(|_| b"{}".to_vec());
    let s = String::from_utf8_lossy(&data);
    let mut index: HashMap<String, Vec<VersionEntry>> =
        serde_json::from_str(&s).unwrap_or_default();
    if index.is_empty() {
        return Ok(0);
    }

    let cutoff = Utc::now() - Duration::days(retention_months as i64 * 30);
    let mut used_blobs: HashMap<String, bool> = HashMap::new();
    let mut removed = 0usize;

    for (_, versions) in index.iter_mut() {
        let len_before = versions.len();
        if let Some(newest) = versions.iter().enumerate().max_by_key(|(_, v)| v.ts) {
            let newest_ts = newest.1.ts;
            versions.retain(|v| v.ts >= cutoff || v.ts == newest_ts);
        }
        removed += len_before - versions.len();
        for v in versions.iter() {
            used_blobs.insert(v.hash.clone(), true);
        }
    }

    let blob_list = dest.list("blob/").await.unwrap_or_default();
    for bp in blob_list {
        let hash = bp.strip_prefix("blob/").unwrap_or(&bp);
        if !used_blobs.contains_key(hash) {
            let _ = dest.delete(&bp).await;
        }
    }

    let new_index = serde_json::to_string_pretty(&index).map_err(|e| e.to_string())?;
    dest.write("index.json", new_index.into_bytes()).await?;
    Ok(removed)
}
