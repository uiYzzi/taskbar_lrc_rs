use crate::*;

/// 创建任务栏小组件窗口
pub fn create_widget_window(
    event_loop: &winit::event_loop::ActiveEventLoop,
    width: u32,
    height: u32,
) -> std::result::Result<Rc<Window>, String> {
    let window_attributes = Window::default_attributes()
        .with_title("Taskbar LRC Widget")
        .with_inner_size(PhysicalSize::new(width, height))
        .with_decorations(false)
        .with_transparent(true)
        .with_window_level(WindowLevel::AlwaysOnTop)
        .with_resizable(false)
        .with_visible(false); // 初始创建时隐藏窗口

    let window = event_loop
        .create_window(window_attributes)
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    // 立即设置窗口扩展样式，隐藏任务栏图标
    hide_from_taskbar(&window);

    Ok(Rc::new(window))
}

/// 获取窗口的Windows句柄
pub fn get_window_hwnd(window: &Window) -> Option<HWND> {
    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
            return Some(HWND(win32_handle.hwnd.get() as *mut _));
        }
    }
    None
}

/// 隐藏窗口在任务栏上的图标
pub fn hide_from_taskbar(window: &Window) {
    if let Some(hwnd) = get_window_hwnd(window) {
        unsafe {
            // 先隐藏窗口
            let _ = ShowWindow(hwnd, SW_HIDE);
            
            // 获取当前扩展样式
            let mut ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            
            // 添加 WS_EX_TOOLWINDOW 样式来隐藏任务栏图标
            // 同时移除可能干扰的样式
            ex_style |= WS_EX_TOOLWINDOW.0;
            ex_style &= !WS_EX_APPWINDOW.0; // 确保移除 APPWINDOW 样式
            
            // 设置新的扩展样式
            SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style as i32);
            
            // 获取当前窗口样式并确保正确设置
            let mut style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
            style &= !WS_VISIBLE.0; // 确保窗口初始不可见
            SetWindowLongW(hwnd, GWL_STYLE, style as i32);
            
            // 强制更新窗口的Z序和任务栏状态
            // 使用更强制的方法确保样式生效
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED
            );
        }
    }
}

/// 确保窗口样式持续有效（在显示窗口时调用）
pub fn ensure_taskbar_hidden(window: &Window) {
    if let Some(hwnd) = get_window_hwnd(window) {
        unsafe {
            // 检查并确保扩展样式正确
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            
            // 如果样式不正确，重新设置
            if (ex_style & WS_EX_TOOLWINDOW.0) == 0 || (ex_style & WS_EX_APPWINDOW.0) != 0 {
                let mut new_ex_style = ex_style;
                new_ex_style |= WS_EX_TOOLWINDOW.0;
                new_ex_style &= !WS_EX_APPWINDOW.0;
                
                SetWindowLongW(hwnd, GWL_EXSTYLE, new_ex_style as i32);
                
                // 强制刷新窗口状态
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    0, 0, 0, 0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED
                );
            }
        }
    }
}
