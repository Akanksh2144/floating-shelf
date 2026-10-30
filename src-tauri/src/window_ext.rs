use tauri::WebviewWindow;

/// Apply non-activating, always-on-top, tool-window styles so dropping
/// onto the shelf never steals focus from Explorer / other apps.
#[cfg(windows)]
pub fn apply_shelf_window_styles(window: &WebviewWindow) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        WS_EX_TOPMOST,
    };

    let hwnd_raw = window.hwnd().map_err(|e| format!("hwnd: {e}"))?;
    let hwnd = HWND(hwnd_raw.0);

    unsafe {
        let mut ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        ex |= WS_EX_NOACTIVATE.0 as isize;
        ex |= WS_EX_TOOLWINDOW.0 as isize;
        ex |= WS_EX_TOPMOST.0 as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex);

        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn apply_shelf_window_styles(_window: &WebviewWindow) -> Result<(), String> {
    // macOS NSPanel wiring lands in a later pass.
    Ok(())
}
