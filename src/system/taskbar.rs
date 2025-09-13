use crate::*;

/// 查找任务栏窗口并获取其信息
pub fn find_taskbar() -> std::result::Result<(HWND, RECT), String> {
    // Windows 10/11 主任务栏
    let taskbar = unsafe { FindWindowW(w!("Shell_TrayWnd"), None) };
    
    match taskbar {
        Ok(hwnd) if !hwnd.0.is_null() => {
            let mut rect = RECT::default();
            unsafe { 
                if let Err(_) = GetWindowRect(hwnd, &mut rect) {
                    return Err("无法获取任务栏区域".to_string());
                }
            };
            
            Ok((hwnd, rect))
        }
        _ => Err("找不到任务栏窗口".to_string())
    }
}

/// 获取通知区域的矩形
pub fn get_notification_area_rect(taskbar_hwnd: HWND) -> RECT {
    let notify_hwnd = unsafe { 
        FindWindowExW(Some(taskbar_hwnd), None, w!("TrayNotifyWnd"), None)
            .unwrap_or_default()
    };
    
    let mut notify_rect = RECT::default();
    if !notify_hwnd.0.is_null() {
        unsafe { 
            let _ = GetWindowRect(notify_hwnd, &mut notify_rect);
        };
    }
    
    notify_rect
}

/// 检查窗口是否与任务栏相关
pub fn is_taskbar_related(hwnd: HWND) -> bool {
    if hwnd.0.is_null() {
        return false;
    }

    unsafe {
        let mut class_name = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut class_name);
        
        if len > 0 {
            let class_name_str = String::from_utf16_lossy(&class_name[..len as usize]);
            
            // 检查是否是任务栏相关的窗口类
            let is_related = class_name_str.contains("Shell_TrayWnd") || 
                           class_name_str.contains("TrayNotifyWnd") ||
                           class_name_str.contains("TrayClockWClass") ||
                           class_name_str.contains("SysPager") ||
                           class_name_str.contains("ToolbarWindow32") ||
                           class_name_str.contains("NotifyIconOverflowWindow") ||
                           class_name_str.contains("TopLevelWindowForOverflowXamlIsland");
            
            // 静默检测，不输出日志
            return is_related;
        }
    }
    // 静默检测失败
    false
}
