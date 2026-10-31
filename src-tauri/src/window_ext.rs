use tauri::WebviewWindow;

/// Always-on-top tool window styles.
///
/// Do **not** set WS_EX_NOACTIVATE — combined with live HWND resizes during
/// OLE drag callbacks it can leave Explorer's DoDragDrop loop wedged so
/// file dragging fails system-wide until this process exits.
///
/// Do **not** use SetWindowRgn — stale regions break drop hit-testing.
#[cfg(windows)]
pub fn apply_shelf_window_styles(window: &WebviewWindow) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE,
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWM_WINDOW_CORNER_PREFERENCE,
    };
    use windows::Win32::Graphics::Gdi::SetWindowRgn;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, GWL_STYLE, HWND_TOPMOST,
        SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, WS_BORDER,
        WS_CAPTION, WS_DLGFRAME, WS_EX_CLIENTEDGE, WS_EX_NOACTIVATE, WS_EX_STATICEDGE,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_WINDOWEDGE, WS_THICKFRAME,
    };

    let hwnd_raw = window.hwnd().map_err(|e| format!("hwnd: {e}"))?;
    let hwnd = HWND(hwnd_raw.0);

    unsafe {
        let mut style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        style &= !(WS_CAPTION.0 as isize);
        style &= !(WS_THICKFRAME.0 as isize);
        style &= !(WS_BORDER.0 as isize);
        style &= !(WS_DLGFRAME.0 as isize);
        SetWindowLongPtrW(hwnd, GWL_STYLE, style);

        let mut ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        ex &= !(WS_EX_CLIENTEDGE.0 as isize);
        ex &= !(WS_EX_WINDOWEDGE.0 as isize);
        ex &= !(WS_EX_STATICEDGE.0 as isize);
        // Explicitly clear NOACTIVATE if a previous build set it.
        ex &= !(WS_EX_NOACTIVATE.0 as isize);
        ex |= WS_EX_TOOLWINDOW.0 as isize;
        ex |= WS_EX_TOPMOST.0 as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex);

        let border = DWMWA_COLOR_NONE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &border as *const _ as *const std::ffi::c_void,
            std::mem::size_of_val(&border) as u32,
        );

        let corner: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_DONOTROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const std::ffi::c_void,
            std::mem::size_of_val(&corner) as u32,
        );

        let _ = SetWindowRgn(hwnd, None, true);

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

#[cfg(windows)]
pub fn clear_window_region(window: &WebviewWindow) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::SetWindowRgn;

    let hwnd_raw = window.hwnd().map_err(|e| format!("hwnd: {e}"))?;
    let hwnd = HWND(hwnd_raw.0);
    unsafe {
        let _ = SetWindowRgn(hwnd, None, true);
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn apply_shelf_window_styles(_window: &WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn clear_window_region(_window: &WebviewWindow) -> Result<(), String> {
    Ok(())
}
