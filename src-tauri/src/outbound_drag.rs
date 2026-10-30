use crate::export_cache::materialize_export;
use crate::state::{AppState, ItemStatus};
use drag::{DragItem, DragMode, DragResult, Image, Options, start_drag};
use std::path::PathBuf;
use tauri::{AppHandle, State, WebviewWindow};

/// Start a native OS drag using immutable export-cache copies (Copy mode).
#[tauri::command]
pub async fn start_outbound_drag(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    shelf_id: String,
    item_ids: Vec<String>,
) -> Result<(), String> {
    if item_ids.is_empty() {
        return Err("no items selected".into());
    }

    let root = state
        .data_root
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| "data root not initialized".to_string())?;

    let paths: Vec<PathBuf> = {
        let shelves = state.shelves.lock().map_err(|e| e.to_string())?;
        let shelf = shelves
            .get(&shelf_id)
            .ok_or_else(|| "shelf not found".to_string())?;

        let mut collected = Vec::new();
        for id in &item_ids {
            let item = shelf
                .items
                .iter()
                .find(|i| &i.id == id)
                .ok_or_else(|| format!("item not found: {id}"))?;
            if item.status != ItemStatus::Ready {
                return Err(format!("item {} is not Ready", item.name));
            }
            collected.push(item.clone());
        }
        drop(shelves);

        let mut out = Vec::new();
        for item in collected {
            let (path, _) = materialize_export(&root, &item).await?;
            out.push(path);
        }
        out
    };

    // Tiny 1x1 PNG placeholder drag preview (polish later).
    let preview = Image::Raw(vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ]);

    start_drag(
        &window,
        DragItem::Files(paths),
        preview,
        move |result, _cursor_position| {
            // Copy-out contract: never remove shelf items on drop (§5.1).
            match result {
                DragResult::Dropped => eprintln!("outbound drag dropped (copy)"),
                DragResult::Cancel => eprintln!("outbound drag canceled"),
            }
        },
        Options {
            mode: DragMode::Copy,
            ..Default::default()
        },
    )
    .map_err(|e| format!("start_drag: {e}"))?;

    // Keep shelf items; Copy-out never removes from shelf (contract §5.1).
    let _ = app;
    Ok(())
}
