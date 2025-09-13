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
            // 获取当前扩展样式
            let mut ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            
            // 添加 WS_EX_TOOLWINDOW 样式来隐藏任务栏图标
            ex_style |= WS_EX_TOOLWINDOW.0;
            
            // 设置新的扩展样式
            SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style as i32);
            
            // 强制系统重新评估窗口的任务栏显示状态
            // 这是必需的，因为窗口样式改变后需要刷新才能生效
            let _ = ShowWindow(hwnd, SW_HIDE);
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE); // 使用 SW_SHOWNOACTIVATE 避免激活窗口
        }
    }
}
