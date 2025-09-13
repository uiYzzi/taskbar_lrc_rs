mod loader;

pub use loader::*;

use crate::*;

/// 字体管理器，负责字体的加载和管理
pub struct FontManager {
    font: Option<Font>,
}

impl FontManager {
    pub fn new() -> Self {
        let font = load_system_font();
        Self { font }
    }

    /// 获取字体引用
    pub fn get_font(&self) -> Option<&Font> {
        self.font.as_ref()
    }

    /// 检查是否有可用字体
    pub fn has_font(&self) -> bool {
        self.font.is_some()
    }

    /// 重新加载字体
    pub fn reload_font(&mut self) {
        self.font = load_system_font();
    }
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
    }
}
