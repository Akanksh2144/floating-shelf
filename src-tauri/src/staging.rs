use crate::paths::{item_input_dir, shelf_dir};
use crate::state::{ItemStatus, Shelf, ShelfItem};
use chrono::Utc;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageProgressEvent {
    pub shelf_id: String,
    pub item_id: String,
    pub progress: f32,
    pub status: ItemStatus,
    pub error: Option<String>,
    pub item: Option<ShelfItem>,
}

fn classify_kind(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "ico" | "tif" | "tiff" | "heic" | "avif"
        | "svg" => "image",
        "mp4" | "mov" | "webm" | "mkv" | "avi" => "video",
        "pdf" => "pdf",
        "zip" | "7z" | "rar" | "tar" | "gz" => "archive",
        "txt" | "md" | "json" | "csv" | "log" | "rs" | "ts" | "tsx" | "js" | "py" | "html" | "css" => {
            "document"
        }
        _ => "file",
    }
}

fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    if cleaned.trim().is_empty() {
        "untitled".into()
    } else {
        cleaned
    }
}

/// Copy a local file into shelf staging. Never moves or mutates the source.
pub async fn stage_local_file(
    app: &AppHandle,
    root: &PathBuf,
    shelf: &mut Shelf,
    source: &Path,
) -> Result<ShelfItem, String> {
    if !source.is_file() {
        return Err(format!("not a file: {}", source.display()));
    }

    let item_id = Uuid::new_v4().to_string();
    let original_name = source
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled")
        .to_string();
    let name = sanitize_filename(&original_name);
    let extension = source
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let kind = classify_kind(&extension).to_string();

    let meta = tokio::fs::metadata(source)
        .await
        .map_err(|e| format!("stat source: {e}"))?;
    let total_size = meta.len();

    let input_dir = item_input_dir(root, &shelf.id, &item_id);
    tokio::fs::create_dir_all(&input_dir)
        .await
        .map_err(|e| format!("create item dir: {e}"))?;
    tokio::fs::create_dir_all(item_input_dir(root, &shelf.id, &item_id).parent().unwrap().join("versions"))
        .await
        .ok();

    let dest = input_dir.join(&name);

    let mut item = ShelfItem {
        id: item_id.clone(),
        name: name.clone(),
        extension: extension.clone(),
        kind,
        size_bytes: total_size,
        status: ItemStatus::Copying,
        progress: 0.0,
        error: None,
        active_path: dest.clone(),
        staged_input_path: dest.clone(),
        original_source_path: Some(source.to_path_buf()),
        created_at: Utc::now(),
    };

    shelf.items.push(item.clone());
    shelf.updated_at = Utc::now();

    let _ = app.emit(
        "shelf:item-progress",
        StageProgressEvent {
            shelf_id: shelf.id.clone(),
            item_id: item_id.clone(),
            progress: 0.0,
            status: ItemStatus::Copying,
            error: None,
            item: Some(item.clone()),
        },
    );

    // Chunked copy so large files can report progress without blocking UI.
    let copy_result = copy_with_progress(app, &shelf.id, &item_id, source, &dest, total_size).await;

    match copy_result {
        Ok(()) => {
            // Verify destination exists and matches size before Ready.
            let staged_meta = tokio::fs::metadata(&dest)
                .await
                .map_err(|e| format!("verify staged file: {e}"))?;
            item.size_bytes = staged_meta.len();
            item.status = ItemStatus::Ready;
            item.progress = 1.0;
            item.error = None;

            if let Some(slot) = shelf.items.iter_mut().find(|i| i.id == item_id) {
                *slot = item.clone();
            }
            shelf.updated_at = Utc::now();

            let _ = app.emit(
                "shelf:item-progress",
                StageProgressEvent {
                    shelf_id: shelf.id.clone(),
                    item_id: item_id.clone(),
                    progress: 1.0,
                    status: ItemStatus::Ready,
                    error: None,
                    item: Some(item.clone()),
                },
            );

            // Persist shelf meta after each successful stage.
            persist_shelf_meta(root, shelf)?;
            Ok(item)
        }
        Err(e) => {
            item.status = ItemStatus::Failed;
            item.error = Some(e.clone());
            if let Some(slot) = shelf.items.iter_mut().find(|i| i.id == item_id) {
                *slot = item.clone();
            }
            let _ = app.emit(
                "shelf:item-progress",
                StageProgressEvent {
                    shelf_id: shelf.id.clone(),
                    item_id,
                    progress: item.progress,
                    status: ItemStatus::Failed,
                    error: Some(e.clone()),
                    item: Some(item),
                },
            );
            Err(e)
        }
    }
}

async fn copy_with_progress(
    app: &AppHandle,
    shelf_id: &str,
    item_id: &str,
    source: &Path,
    dest: &Path,
    total_size: u64,
) -> Result<(), String> {
    let mut reader = tokio::fs::File::open(source)
        .await
        .map_err(|e| format!("open source: {e}"))?;
    let mut writer = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("create staged file: {e}"))?;

    let mut buf = vec![0u8; 1024 * 256];
    let mut copied: u64 = 0;
    let mut last_emit = 0.0f32;

    loop {
        let n = tokio::io::AsyncReadExt::read(&mut reader, &mut buf)
            .await
            .map_err(|e| format!("read source: {e}"))?;
        if n == 0 {
            break;
        }
        writer
            .write_all(&buf[..n])
            .await
            .map_err(|e| format!("write staged: {e}"))?;
        copied += n as u64;

        let progress = if total_size == 0 {
            1.0
        } else {
            (copied as f32 / total_size as f32).min(0.99)
        };

        // Throttle UI events (~every 5%).
        if progress - last_emit >= 0.05 || progress >= 0.99 {
            last_emit = progress;
            let _ = app.emit(
                "shelf:item-progress",
                StageProgressEvent {
                    shelf_id: shelf_id.to_string(),
                    item_id: item_id.to_string(),
                    progress,
                    status: ItemStatus::Copying,
                    error: None,
                    item: None,
                },
            );
        }
    }

    writer
        .flush()
        .await
        .map_err(|e| format!("flush staged: {e}"))?;
    Ok(())
}

pub fn persist_shelf_meta(root: &PathBuf, shelf: &Shelf) -> Result<(), String> {
    let dir = shelf_dir(root, &shelf.id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create shelf dir: {e}"))?;
    let path = dir.join("meta.json");
    let json = serde_json::to_string_pretty(shelf).map_err(|e| format!("serialize shelf: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write shelf meta: {e}"))?;
    Ok(())
}

pub fn load_shelf_meta(root: &PathBuf, shelf_id: &str) -> Result<Option<Shelf>, String> {
    let path = shelf_dir(root, shelf_id).join("meta.json");
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("read shelf meta: {e}"))?;
    let shelf: Shelf = serde_json::from_str(&raw).map_err(|e| format!("parse shelf meta: {e}"))?;
    Ok(Some(shelf))
}
