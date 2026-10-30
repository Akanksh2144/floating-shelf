use crate::paths::export_cache_dir;
use crate::state::ShelfItem;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRecord {
    pub id: String,
    pub item_id: String,
    pub path: PathBuf,
    pub created_at: chrono::DateTime<Utc>,
    pub last_dragged_at: chrono::DateTime<Utc>,
}

/// Create an immutable Copy-mode export for outbound drag.
/// Closing a shelf must never rename/delete this path during retention.
pub async fn materialize_export(
    root: &PathBuf,
    item: &ShelfItem,
) -> Result<(PathBuf, ExportRecord), String> {
    if item.status != crate::state::ItemStatus::Ready {
        return Err("item is not Ready yet".into());
    }
    if !item.active_path.exists() {
        return Err(format!(
            "active path missing: {}",
            item.active_path.display()
        ));
    }

    let export_id = Uuid::new_v4().to_string();
    let dir = export_cache_dir(root).join(&export_id);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("create export dir: {e}"))?;

    let dest = dir.join(&item.name);
    tokio::fs::copy(&item.active_path, &dest)
        .await
        .map_err(|e| format!("copy to export cache: {e}"))?;

    let now = Utc::now();
    let record = ExportRecord {
        id: export_id,
        item_id: item.id.clone(),
        path: dest.clone(),
        created_at: now,
        last_dragged_at: now,
    };

    let meta_path = dir.join("meta.json");
    let json = serde_json::to_string_pretty(&record).map_err(|e| e.to_string())?;
    tokio::fs::write(meta_path, json)
        .await
        .map_err(|e| format!("write export meta: {e}"))?;

    Ok((dest, record))
}
