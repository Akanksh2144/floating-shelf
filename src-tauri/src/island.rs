use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewWindow};

pub const COLLAPSED_W: f64 = 172.0;
pub const COLLAPSED_H: f64 = 52.0;
pub const EXPANDED_W: f64 = 340.0;
pub const EXPANDED_H: f64 = 460.0;
pub const TOP_MARGIN: f64 = 12.0;

fn shelf_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window("shelf")
        .ok_or_else(|| "shelf window missing".into())
}

/// Place the island at top-center of the monitor that currently owns the window.
pub fn dock_top_center(win: &WebviewWindow, width: f64, height: f64) -> Result<(), String> {
    let monitor = win
        .current_monitor()
        .map_err(|e| e.to_string())?
        .or_else(|| win.primary_monitor().ok().flatten())
        .ok_or_else(|| "no monitor".to_string())?;

    let scale = monitor.scale_factor();
    let mpos = monitor.position();
    let msize = monitor.size();

    let screen_w = msize.width as f64 / scale;
    let origin_x = mpos.x as f64 / scale;
    let origin_y = mpos.y as f64 / scale;

    let x = origin_x + ((screen_w - width) / 2.0).max(0.0);
    let y = origin_y + TOP_MARGIN;

    win.set_size(tauri::Size::Logical(LogicalSize::new(width, height)))
        .map_err(|e| e.to_string())?;
    win.set_position(tauri::Position::Logical(LogicalPosition::new(x, y)))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Resize/reposition the island (used for Motion-driven morph frames).
#[tauri::command]
pub fn set_island_bounds(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    let win = shelf_window(&app)?;
    let w = width.clamp(120.0, EXPANDED_W + 40.0);
    let h = height.clamp(44.0, EXPANDED_H + 40.0);
    let _ = win.set_resizable(false);
    dock_top_center(&win, w, h)?;
    let _ = win.show();
    let _ = crate::window_ext::apply_shelf_window_styles(&win);
    Ok(())
}

#[tauri::command]
pub fn set_island_expanded(app: AppHandle, expanded: bool) -> Result<(), String> {
    let (w, h) = if expanded {
        (EXPANDED_W, EXPANDED_H)
    } else {
        (COLLAPSED_W, COLLAPSED_H)
    };
    set_island_bounds(app.clone(), w, h)?;
    let _ = app.emit("island:mode", if expanded { "expanded" } else { "collapsed" });
    Ok(())
}

#[tauri::command]
pub fn reveal_island(app: AppHandle) -> Result<(), String> {
    set_island_expanded(app, false)
}
