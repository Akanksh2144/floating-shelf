use tauri::WebviewWindow;

/// Apply non-activating, always-on-top, tool-window styles so dropping
/// onto the shelf never steals focus from Explorer / other apps.
/// Also strip the Win11 1px window border and clip the HWND to a rounded
/// region so a square frame cannot appear around the pill / panel.
#[cfg(windows)]
pub fn apply_shelf_window_styles(window: &WebviewWindow) -> Result<(), String> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE,
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DWM_WINDOW_CORNER_PREFERENCE,
    };
    use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE,
        GWL_STYLE, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SWP_SHOWWINDOW, WS_BORDER, WS_CAPTION, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        WS_EX_TOPMOST, WS_THICKFRAME,
    };

    let hwnd_raw = window.hwnd().map_err(|e| format!("hwnd: {e}"))?;
    let hwnd = HWND(hwnd_raw.0);

    unsafe {
        let mut style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        style &= !(WS_CAPTION.0 as isize);
        style &= !(WS_THICKFRAME.0 as isize);
        style &= !(WS_BORDER.0 as isize);
        SetWindowLongPtrW(hwnd, GWL_STYLE, style);

        let mut ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        ex |= WS_EX_NOACTIVATE.0 as isize;
        ex |= WS_EX_TOOLWINDOW.0 as isize;
        ex |= WS_EX_TOPMOST.0 as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex);

        // Kill the thin Win11 system border that draws a rectangle around the client.
        let border = DWMWA_COLOR_NONE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &border as *const _ as *const std::ffi::c_void,
            std::mem::size_of_val(&border) as u32,
        );

        let corner: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const std::ffi::c_void,
            std::mem::size_of_val(&corner) as u32,
        );

        let mut rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut rect);
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        if w > 0 && h > 0 {
            // Pill when short; soft panel radius when expanded (~22px logical @ 1–1.5x DPI).
            let ellipse = if h <= 72 { h } else { (22.0 * window.scale_factor().unwrap_or(1.0)).round() as i32 * 2 };
            let region = CreateRoundRectRgn(0, 0, w + 1, h + 1, ellipse, ellipse);
            // SetWindowRgn takes ownership of the HRGN.
            let _ = SetWindowRgn(hwnd, Some(region), true);
        }

        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
        );
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn apply_shelf_window_styles(_window: &WebviewWindow) -> Result<(), String> {
    // macOS NSPanel wiring lands in a later pass.
    Ok(())
}
