use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewWindow};

/// Slim vertical handle — movable; click expands, drag repositions.
pub const COLLAPSED_W: f64 = 8.0;
pub const COLLAPSED_H: f64 = 88.0;
pub const EXPANDED_W: f64 = 340.0;
pub const EXPANDED_H: f64 = 460.0;
pub const EDGE_INSET: f64 = 0.0;

fn shelf_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window("shelf")
        .ok_or_else(|| "shelf window missing".into())
}

fn monitor_for(win: &WebviewWindow) -> Result<tauri::Monitor, String> {
    win.current_monitor()
        .map_err(|e| e.to_string())?
        .or_else(|| win.primary_monitor().ok().flatten())
        .ok_or_else(|| "no monitor".to_string())
}

/// Dock to the right edge, vertically centered (initial / Reveal).
pub fn dock_right_edge(win: &WebviewWindow, width: f64, height: f64) -> Result<(), String> {
    let monitor = monitor_for(win)?;
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

/// Resize in place: keep the right edge fixed (grows/shrinks leftward) and
/// vertically center relative to the previous frame. Clamps onto the monitor.
fn resize_keep_anchor(win: &WebviewWindow, width: f64, height: f64) -> Result<(), String> {
    let monitor = monitor_for(win)?;
    let scale = monitor.scale_factor();
    let mpos = monitor.position();
    let msize = monitor.size();

    let screen_w = msize.width as f64 / scale;
    let screen_h = msize.height as f64 / scale;
    let origin_x = mpos.x as f64 / scale;
    let origin_y = mpos.y as f64 / scale;

    let old_pos = win.outer_position().map_err(|e| e.to_string())?;
    let old_size = win.outer_size().map_err(|e| e.to_string())?;
    let old_x = old_pos.x as f64 / scale;
    let old_y = old_pos.y as f64 / scale;
    let old_w = old_size.width as f64 / scale;
    let old_h = old_size.height as f64 / scale;

    let mut x = old_x + old_w - width;
    let mut y = old_y + (old_h - height) / 2.0;

    x = x.clamp(origin_x, origin_x + screen_w - width);
    y = y.clamp(origin_y, origin_y + screen_h - height);

    win.set_size(tauri::Size::Logical(LogicalSize::new(width, height)))
        .map_err(|e| e.to_string())?;
    win.set_position(tauri::Position::Logical(LogicalPosition::new(x, y)))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Resize the island.
/// - `dock: true` → snap to right edge (Reveal / first launch)
/// - `dock: false` → keep current place (morph / after user dragged the bar)
#[tauri::command]
pub fn set_island_bounds(
    app: AppHandle,
    width: f64,
    height: f64,
    restyle: Option<bool>,
    dock: Option<bool>,
) -> Result<(), String> {
    let win = shelf_window(&app)?;
    let w = width.clamp(6.0, EXPANDED_W + 40.0);
    let h = height.clamp(40.0, EXPANDED_H + 40.0);
    let _ = win.set_resizable(false);

    let _ = crate::window_ext::clear_window_region(&win);

    if dock.unwrap_or(false) {
        dock_right_edge(&win, w, h)?;
    } else {
        resize_keep_anchor(&win, w, h)?;
    }

    let _ = win.show();
    if restyle.unwrap_or(true) {
        let _ = crate::window_ext::apply_shelf_window_styles(&win);
    } else {
        let _ = crate::window_ext::clear_window_region(&win);
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
    // Keep user position when collapsing from the panel.
    set_island_bounds(app.clone(), w, h, Some(true), Some(false))?;
    let _ = app.emit("island:mode", if expanded { "expanded" } else { "collapsed" });
    Ok(())
}

#[tauri::command]
pub fn reveal_island(app: AppHandle) -> Result<(), String> {
    let win = shelf_window(&app)?;
    let _ = win.set_resizable(false);
    dock_right_edge(&win, COLLAPSED_W, COLLAPSED_H)?;
    let _ = win.show();
    let _ = crate::window_ext::apply_shelf_window_styles(&win);
    let _ = app.emit("island:mode", "collapsed");
    Ok(())
}
