mod creation;
mod positioning;

pub use creation::*;
pub use positioning::*;

use crate::*;

/// 窗口管理器，负责窗口的创建、定位和属性管理
pub struct WindowManager {
    window: Option<Rc<Window>>,
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            window: None,
        }
    }

    /// 创建窗口
    pub fn create_window(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        width: u32,
        height: u32,
    ) -> std::result::Result<(), String> {
        let window = create_widget_window(event_loop, width, height)?;
        self.window = Some(window);
        Ok(())
    }

    /// 获取窗口引用
    pub fn get_window(&self) -> Option<&Rc<Window>> {
        self.window.as_ref()
    }

    /// 设置窗口位置
    pub fn set_position(&self, x: i32, y: i32, width: u32, height: u32) -> std::result::Result<(), String> {
        if let Some(window) = &self.window {
            set_window_position(window, x, y, width, height)
        } else {
            Err("窗口未创建".to_string())
        }
    }

    /// 确保窗口在最上层
    pub fn ensure_topmost(&self) {
        if let Some(window) = &self.window {
            ensure_window_topmost(window);
        }
    }

    /// 获取窗口的Windows句柄
    pub fn get_hwnd(&self) -> Option<HWND> {
        if let Some(window) = &self.window {
            get_window_hwnd(window)
        } else {
            None
        }
    }
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}
