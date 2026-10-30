use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// App-managed storage layout (file-safety contract §5.1):
///
/// ```text
/// {app_data}/floating-shelf/
///   shelves/{shelf_id}/items/{item_id}/
///     input/          # immutable staged copy of source
///     versions/       # transform outputs (v1, v2, ...)
///     meta.json
///   export-cache/{export_id}/
///   recently-closed/
///   state.json
/// ```
pub fn data_root(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?;
    let root = base.join("floating-shelf");
    std::fs::create_dir_all(&root).map_err(|e| format!("create data root: {e}"))?;
    Ok(root)
}

pub fn shelves_dir(root: &PathBuf) -> PathBuf {
    root.join("shelves")
}

pub fn shelf_dir(root: &PathBuf, shelf_id: &str) -> PathBuf {
    shelves_dir(root).join(shelf_id)
}

pub fn item_dir(root: &PathBuf, shelf_id: &str, item_id: &str) -> PathBuf {
    shelf_dir(root, shelf_id).join("items").join(item_id)
}

pub fn item_input_dir(root: &PathBuf, shelf_id: &str, item_id: &str) -> PathBuf {
    item_dir(root, shelf_id, item_id).join("input")
}

pub fn item_versions_dir(root: &PathBuf, shelf_id: &str, item_id: &str) -> PathBuf {
    item_dir(root, shelf_id, item_id).join("versions")
}

pub fn export_cache_dir(root: &PathBuf) -> PathBuf {
    root.join("export-cache")
}

pub fn recently_closed_dir(root: &PathBuf) -> PathBuf {
    root.join("recently-closed")
}

pub fn state_file(root: &PathBuf) -> PathBuf {
    root.join("state.json")
}

pub fn ensure_layout(root: &PathBuf) -> Result<(), String> {
    for dir in [
        shelves_dir(root),
        export_cache_dir(root),
        recently_closed_dir(root),
    ] {
        std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    Ok(())
}
