use tauri::WebviewWindow;

/// Apply non-activating, always-on-top, tool-window styles so dropping
/// onto the shelf never steals focus from Explorer / other apps.
///
/// On Windows we also force a true client-only frame: no DWM border, no
/// Win11 rounded-rect chrome (that was the "rectangle around the pill"),
/// and a capsule/rounded region clip so square HWND corners cannot show.
#[cfg(windows)]
pub fn apply_shelf_window_styles(window: &WebviewWindow) -> Result<(), String> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMNCRP_DISABLED, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE,
        DWMWA_NCRENDERING_POLICY, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
        DWM_WINDOW_CORNER_PREFERENCE,
    };
    use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, GetWindowLongPtrW, GetWindowRect, SetWindowLongPtrW, SetWindowPos,
        GWL_EXSTYLE, GWL_STYLE, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE, SWP_SHOWWINDOW, WS_BORDER, WS_CAPTION, WS_DLGFRAME, WS_EX_CLIENTEDGE,
        WS_EX_NOACTIVATE, WS_EX_STATICEDGE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_WINDOWEDGE,
        WS_THICKFRAME,
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
        ex |= WS_EX_NOACTIVATE.0 as isize;
        ex |= WS_EX_TOOLWINDOW.0 as isize;
        ex |= WS_EX_TOPMOST.0 as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex);

        // No 1px Win11 system contour.
        let border = DWMWA_COLOR_NONE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &border as *const _ as *const std::ffi::c_void,
            std::mem::size_of_val(&border) as u32,
        );

        // Critical: do NOT use DWMWCP_ROUND — that draws the soft rectangle
        // around a more-rounded CSS pill (exactly the artifact in screenshots).
        let corner: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_DONOTROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const std::ffi::c_void,
            std::mem::size_of_val(&corner) as u32,
        );

        let ncrp = DWMNCRP_DISABLED;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY,
            &ncrp as *const _ as *const std::ffi::c_void,
            std::mem::size_of_val(&ncrp) as u32,
        );

        // Clip using window (not client) size — region is relative to the window
        // including any residual non-client, so GetWindowRect deltas are safer.
        let mut window_rect = RECT::default();
        let mut client_rect = RECT::default();
        let _ = GetWindowRect(hwnd, &mut window_rect);
        let _ = GetClientRect(hwnd, &mut client_rect);
        let w = client_rect.right - client_rect.left;
        let h = client_rect.bottom - client_rect.top;
        if w > 0 && h > 0 {
            // Vertical edge bar → capsule on width; short horizontal → capsule on height;
            // expanded panel → soft ~22px logical radius.
            let ellipse = if w <= h && w <= 24 {
                w
            } else if h <= w && h <= 80 {
                h
            } else {
                (22.0 * window.scale_factor().unwrap_or(1.0)).round() as i32 * 2
            };
            let region = CreateRoundRectRgn(0, 0, w + 1, h + 1, ellipse, ellipse);
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
