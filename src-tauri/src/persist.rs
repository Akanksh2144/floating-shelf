use crate::paths::{ensure_layout, shelves_dir, state_file};
use crate::staging::load_shelf_meta;
use crate::state::{AppState, Shelf};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PersistedState {
    pub active_shelf_id: Option<String>,
    pub shelf_ids: Vec<String>,
}

pub fn init_storage(root: &PathBuf) -> Result<(), String> {
    ensure_layout(root)
}

pub fn save_app_state(root: &PathBuf, state: &AppState) -> Result<(), String> {
    let shelves = state.shelves.lock().map_err(|e| e.to_string())?;
    let active = state
        .active_shelf_id
        .lock()
        .map_err(|e| e.to_string())?
        .clone();

    let persisted = PersistedState {
        active_shelf_id: active,
        shelf_ids: shelves.keys().cloned().collect(),
    };

    let path = state_file(root);
    let json =
        serde_json::to_string_pretty(&persisted).map_err(|e| format!("serialize state: {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("write state: {e}"))?;
    Ok(())
}

/// Restore committed shelves after crash / restart.
/// Items that were mid-copy (`Copying`) are marked Failed so the UI can retry.
pub fn restore_app_state(root: &PathBuf, state: &AppState) -> Result<Option<Shelf>, String> {
    let path = state_file(root);
    if !path.exists() {
        return Ok(None);
    }

    let raw = std::fs::read_to_string(&path).map_err(|e| format!("read state: {e}"))?;
    let persisted: PersistedState =
        serde_json::from_str(&raw).map_err(|e| format!("parse state: {e}"))?;

    let mut restored: HashMap<String, Shelf> = HashMap::new();
    for id in &persisted.shelf_ids {
        if let Some(mut shelf) = load_shelf_meta(root, id)? {
            for item in &mut shelf.items {
                if item.status == crate::state::ItemStatus::Copying
                    || item.status == crate::state::ItemStatus::Transforming
                {
                    // Interrupted ops are never Ready; leave last good version if present.
                    if item.active_path.exists()
                        && item.active_path != item.staged_input_path
                        || (item.staged_input_path.exists()
                            && item.active_path == item.staged_input_path
                            && item.progress >= 1.0)
                    {
                        // staged input fully written before crash — recover as Ready
                        if item.staged_input_path.exists() {
                            item.status = crate::state::ItemStatus::Ready;
                            item.progress = 1.0;
                            item.error = None;
                        } else {
                            item.status = crate::state::ItemStatus::Failed;
                            item.error = Some("Interrupted by crash; please re-add file".into());
                        }
                    } else if item.staged_input_path.exists() {
                        item.status = crate::state::ItemStatus::Ready;
                        item.progress = 1.0;
                        item.error = None;
                        item.active_path = item.staged_input_path.clone();
                    } else {
                        item.status = crate::state::ItemStatus::Failed;
                        item.error = Some("Interrupted by crash; please re-add file".into());
                    }
                }
            }
            restored.insert(shelf.id.clone(), shelf);
        }
    }

    // Also scan shelves dir for orphans not listed in state.json
    let shelves_path = shelves_dir(root);
    if shelves_path.exists() {
        for entry in std::fs::read_dir(&shelves_path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let id = entry.file_name().to_string_lossy().to_string();
            if !restored.contains_key(&id) {
                if let Some(shelf) = load_shelf_meta(root, &id)? {
                    restored.insert(shelf.id.clone(), shelf);
                }
            }
        }
    }

    let active_id = persisted
        .active_shelf_id
        .filter(|id| restored.contains_key(id))
        .or_else(|| restored.keys().next().cloned());

    let active_shelf = active_id
        .as_ref()
        .and_then(|id| restored.get(id).cloned());

    {
        let mut shelves = state.shelves.lock().map_err(|e| e.to_string())?;
        *shelves = restored;
    }
    {
        let mut active = state.active_shelf_id.lock().map_err(|e| e.to_string())?;
        *active = active_id;
    }

    Ok(active_shelf)
}
