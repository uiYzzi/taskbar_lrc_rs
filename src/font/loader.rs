use crate::*;

/// 加载系统字体
pub fn load_system_font() -> Option<Font> {
    // 尝试多个可能的中文字体路径（优先正常字重）
    let font_paths = [
        r"C:\Windows\Fonts\msyhl.ttc",     // 微软雅黑 Light
        r"C:\Windows\Fonts\simhei.ttf",    // 黑体 (备选)
        r"C:\Windows\Fonts\simsun.ttc",    // 宋体 (备选)
    ];

    for path in &font_paths {
        if let Some(font) = try_load_font_from_path(path) {
            return Some(font);
        }
    }

    None
}

/// 尝试从指定路径加载字体
fn try_load_font_from_path(path: &str) -> Option<Font> {
    if let Ok(font_data) = std::fs::read(path) {
        let settings = FontSettings {
            collection_index: 0,
            scale: 40.0,
            load_substitutions: true,
        };
        
        if let Ok(font) = Font::from_bytes(font_data, settings) {
            return Some(font);
        }
    }
    None
}

/// 使用像素字体计算文本宽度（备选方案）
pub fn get_pixel_text_width(text: &str, char_width: u32) -> u32 {
    text.chars().count() as u32 * char_width
}

/// 使用 Layout API 渲染文本，返回字符信息和整体布局信息
pub fn layout_text(font: &Font, text: &str, font_size: f32) -> (Vec<fontdue::layout::GlyphPosition>, f32, f32) {
    let fonts = &[font];
    let mut layout = fontdue::layout::Layout::new(fontdue::layout::CoordinateSystem::PositiveYDown);
    
    layout.reset(&fontdue::layout::LayoutSettings {
        x: 0.0,
        y: 0.0,
        max_width: None,
        max_height: None,
        horizontal_align: fontdue::layout::HorizontalAlign::Left,
        vertical_align: fontdue::layout::VerticalAlign::Top,
        line_height: 1.0,
        wrap_style: fontdue::layout::WrapStyle::Word,
        wrap_hard_breaks: true,
    });
    
    layout.append(fonts, &fontdue::layout::TextStyle::new(text, font_size, 0));
    
    let glyphs = layout.glyphs().to_vec();
    let height = layout.height();
    

    let width = glyphs.iter()
        .map(|g| g.x + g.width as f32)
        .fold(0.0, f32::max);
    
    (glyphs, width, height)
}

/// 使用 Layout API 计算文本宽度
pub fn get_layout_text_width(font: &Font, text: &str, font_size: f32) -> f32 {
    let (_, width, _) = layout_text(font, text, font_size);
    width
}
