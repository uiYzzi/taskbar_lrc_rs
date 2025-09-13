use crate::*;
use crate::widget::TaskbarWidget;
use std::ptr;

// 全局静态指针存储widget (用于系统事件钩子)
static mut WIDGET_PTR: *const TaskbarWidget = ptr::null();

/// 设置全局widget指针（用于事件钩子回调）
pub fn set_widget_pointer(widget: &TaskbarWidget) {
    unsafe {
        WIDGET_PTR = widget as *const TaskbarWidget;
    }
}

/// 设置系统事件钩子
pub fn setup_system_event_hook() -> std::result::Result<HWINEVENTHOOK, String> {
    unsafe {
        let hook = SetWinEventHook(
            EVENT_OBJECT_CREATE,         // 最小事件类型 - 监听窗口创建
            EVENT_OBJECT_REORDER,        // 最大事件类型 - 监听窗口重新排序（包括Z-order变化）
            None,                        // 模块句柄
            Some(win_event_proc),        // 回调函数
            0,                           // 进程ID (0表示所有进程)
            0,                           // 线程ID (0表示所有线程)
            WINEVENT_OUTOFCONTEXT,       // 标志
        );

        if !hook.0.is_null() {
            Ok(hook)
        } else {
            Err("设置事件钩子失败".to_string())
        }
    }
}

/// 清理事件钩子
pub fn cleanup_event_hook(hook: HWINEVENTHOOK) {
    if !hook.0.is_null() {
        unsafe {
            let _ = UnhookWinEvent(hook);
        }
    }
}

/// 系统事件钩子回调函数
unsafe extern "system" fn win_event_proc(
    _hevent: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _idobject: i32,
    _idchild: i32,
    _ideventthread: u32,
    _dwmseventtime: u32,
) {
    unsafe {
        if WIDGET_PTR.is_null() {
            return;
        }

        let widget = &mut *(WIDGET_PTR as *mut TaskbarWidget);

        // 只在处理相关事件时获取窗口类名
        let is_taskbar_related = widget.is_taskbar_related_window(hwnd) || 
                                 hwnd == widget.system_manager.taskbar_hwnd;

    match event {
        EVENT_OBJECT_FOCUS => {
            // 当其他窗口获得焦点时，立即确保我们的窗口保持在最上层
            if let Some(our_hwnd) = widget.get_window_hwnd() {
                if hwnd != our_hwnd {
                    widget.ensure_topmost();
                }
            }
        }
        EVENT_OBJECT_CREATE | EVENT_OBJECT_DESTROY => {
            // 窗口创建或销毁时，检查是否是任务栏相关窗口
            if is_taskbar_related {
                widget.position_update_pending = true;
                // 窗口创建/销毁可能影响层级，立即确保最上层
                widget.ensure_topmost();
            }
        }
        EVENT_OBJECT_LOCATIONCHANGE => {
            // 监听任务栏及其重要子窗口的位置变化
            if hwnd == widget.system_manager.taskbar_hwnd || is_taskbar_related {
                widget.position_update_pending = true;
            }
        }
        EVENT_OBJECT_SHOW | EVENT_OBJECT_HIDE => {
            // 窗口显示/隐藏状态变化
            if is_taskbar_related {
                widget.position_update_pending = true;
            }
        }
        EVENT_OBJECT_REORDER => {
            // 窗口Z-order变化，可能影响任务栏布局和窗口层级
            if is_taskbar_related {
                widget.position_update_pending = true;
            }
            // 任何窗口重新排序都可能影响我们的层级，立即确保最上层
            widget.ensure_topmost();
        }
        EVENT_OBJECT_STATECHANGE => {
            // 窗口状态变化（最小化、最大化等）
            if is_taskbar_related {
                widget.position_update_pending = true;
            }
        }
        _ => {}
        }
    }
}
