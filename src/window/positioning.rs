use crate::*;

/// 设置窗口位置
pub fn set_window_position(
    window: &Window,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> std::result::Result<(), String> {
    // 使用winit设置窗口位置
    window.set_outer_position(PhysicalPosition::new(x, y));
    
    // 通过Windows API直接设置位置确保精确
    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
            let hwnd = HWND(win32_handle.hwnd.get() as *mut _);
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    x,
                    y,
                    width as i32,
                    height as i32,
                    SWP_SHOWWINDOW | SWP_NOACTIVATE,
                );
            }
        }
    }
    
    Ok(())
}

/// 确保窗口始终在最上层
pub fn ensure_window_topmost(window: &Window) {
    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
            let hwnd = HWND(win32_handle.hwnd.get() as *mut _);
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    0, 0, 0, 0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
    }
}
