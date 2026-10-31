use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewWindow};

/// Vertical edge bar — wide enough to hit with Explorer drag-drop.
pub const COLLAPSED_W: f64 = 20.0;
pub const COLLAPSED_H: f64 = 96.0;
pub const EXPANDED_W: f64 = 340.0;
pub const EXPANDED_H: f64 = 460.0;
pub const EDGE_INSET: f64 = 6.0;

fn shelf_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window("shelf")
        .ok_or_else(|| "shelf window missing".into())
}

/// Dock the island to the right edge of the monitor, vertically centered.
/// Expand/collapse grows inward from this edge (x stays right-aligned).
pub fn dock_right_edge(win: &WebviewWindow, width: f64, height: f64) -> Result<(), String> {
    let monitor = win
        .current_monitor()
        .map_err(|e| e.to_string())?
        .or_else(|| win.primary_monitor().ok().flatten())
        .ok_or_else(|| "no monitor".to_string())?;

    let scale = monitor.scale_factor();
    let mpos = monitor.position();
    let msize = monitor.size();

    let screen_w = msize.width as f64 / scale;
    let screen_h = msize.height as f64 / scale;
    let origin_x = mpos.x as f64 / scale;
    let origin_y = mpos.y as f64 / scale;

    let x = origin_x + screen_w - width - EDGE_INSET;
    let y = origin_y + ((screen_h - height) / 2.0).max(0.0);

    win.set_size(tauri::Size::Logical(LogicalSize::new(width, height)))
        .map_err(|e| e.to_string())?;
    win.set_position(tauri::Position::Logical(LogicalPosition::new(x, y)))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Resize/reposition the island (used for Motion-driven morph frames).
/// When `restyle` is false, skip SetWindowRgn/DWM — mid-morph restyles break OLE drag-drop.
#[tauri::command]
pub fn set_island_bounds(
    app: AppHandle,
    width: f64,
    height: f64,
    restyle: Option<bool>,
) -> Result<(), String> {
    let win = shelf_window(&app)?;
    let w = width.clamp(8.0, EXPANDED_W + 40.0);
    let h = height.clamp(40.0, EXPANDED_H + 40.0);
    let _ = win.set_resizable(false);
    dock_right_edge(&win, w, h)?;
    let _ = win.show();
    if restyle.unwrap_or(true) {
        let _ = crate::window_ext::apply_shelf_window_styles(&win);
    }
    Ok(())
}

#[tauri::command]
pub fn set_island_expanded(app: AppHandle, expanded: bool) -> Result<(), String> {
    let (w, h) = if expanded {
        (EXPANDED_W, EXPANDED_H)
    } else {
        (COLLAPSED_W, COLLAPSED_H)
    };
    set_island_bounds(app.clone(), w, h, Some(true))?;
    let _ = app.emit("island:mode", if expanded { "expanded" } else { "collapsed" });
    Ok(())
}

#[tauri::command]
pub fn reveal_island(app: AppHandle) -> Result<(), String> {
    set_island_expanded(app, false)
}
