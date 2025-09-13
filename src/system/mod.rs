mod taskbar;
mod events;
mod media;
mod playback_timer;

pub use taskbar::*;
pub use events::*;
pub use media::*;
pub use playback_timer::*;

use crate::*;

/// 系统管理器，负责与Windows系统的交互（简化版本）
/// 媒体监控和歌词服务现在由App直接管理
pub struct SystemManager {
    pub taskbar_hwnd: HWND,
    taskbar_rect: RECT,
    event_hook: HWINEVENTHOOK,
}

impl SystemManager {
    pub fn new() -> Self {
        Self {
            taskbar_hwnd: HWND::default(),
            taskbar_rect: RECT::default(),
            event_hook: HWINEVENTHOOK::default(),
        }
    }

    /// 查找任务栏句柄
    pub fn find_taskbar_handle(&mut self) -> std::result::Result<HWND, String> {
        let (hwnd, rect) = find_taskbar()?;
        self.taskbar_hwnd = hwnd;
        self.taskbar_rect = rect;
        
        let _taskbar_height = (rect.bottom - rect.top) as u32;
        
        Ok(hwnd)
    }

    /// 获取任务栏高度
    pub fn get_taskbar_height(&self) -> u32 {
        (self.taskbar_rect.bottom - self.taskbar_rect.top) as u32
    }

    /// 获取任务栏矩形区域
    pub fn get_taskbar_rect(&self) -> RECT {
        self.taskbar_rect
    }

    /// 获取通知区域矩形
    pub fn get_notify_area_rect(&self) -> RECT {
        get_notification_area_rect(self.taskbar_hwnd)
    }

    /// 设置系统事件钩子
    pub fn setup_event_hook(&mut self) -> std::result::Result<(), String> {
        let hook = setup_system_event_hook()?;
        self.event_hook = hook;
        Ok(())
    }

    /// 检查窗口是否与任务栏相关
    pub fn is_taskbar_related_window(&self, hwnd: HWND) -> bool {
        is_taskbar_related(hwnd)
    }

    /// 清理资源
    pub fn cleanup(&mut self) {
        // 清理事件钩子
        cleanup_event_hook(self.event_hook);
        self.event_hook = HWINEVENTHOOK::default();
    }
}

impl Default for SystemManager {
    fn default() -> Self {
        Self::new()
    }
}
