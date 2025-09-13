pub mod widget;
pub mod window;
pub mod graphics;
pub mod font;
pub mod system;
pub mod app;
pub mod lyrics;

// 导出主要的公共类型
pub use widget::TaskbarWidget;
pub use app::App;

// 重新导出常用的 Windows API 类型
pub use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::UI::Accessibility::*,
};

// 重新导出 winit 相关类型
pub use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::EventLoop,
    window::{Window, WindowLevel},
    dpi::{PhysicalPosition, PhysicalSize},
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
};

// 重新导出其他常用类型
pub use softbuffer::{Context, Surface};
pub use fontdue::{Font, FontSettings};
pub use std::{ptr, rc::Rc, time::{Duration, Instant}, num::NonZeroU32};

// 重新导出异步运行时和序列化相关类型
pub use tokio;
pub use serde::{Deserialize, Serialize};