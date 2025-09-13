use crate::*;
use crate::font::FontManager;

/// 图形渲染器，负责处理所有的绘制操作
pub struct Renderer {
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    context: Option<Context<Rc<Window>>>,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            surface: None,
            context: None,
        }
    }

    /// 初始化渲染器
    pub fn initialize(&mut self, window: &Rc<Window>) -> std::result::Result<(), String> {
        let context = Context::new(window.clone())
            .map_err(|e| format!("创建渲染上下文失败: {}", e))?;
            
        let surface = Surface::new(&context, window.clone())
            .map_err(|e| format!("创建渲染表面失败: {}", e))?;

        self.context = Some(context);
        self.surface = Some(surface);

        Ok(())
    }

    /// 绘制一帧内容
    pub fn draw_frame(
        &mut self,
        text: &str,
        font_manager: &FontManager,
        font_size: f32,
        color: u32,
        window_width: u32,
        window_height: u32,
        margin: u32,
        scroll_offset: f32,
    ) -> std::result::Result<(), String> {
        let surface = self.surface.as_mut()
            .ok_or("渲染表面未初始化")?;

        let width = NonZeroU32::new(window_width).unwrap();
        let height = NonZeroU32::new(window_height).unwrap();
        
        // 调整缓冲区大小
        surface.resize(width, height)
            .map_err(|e| format!("调整缓冲区失败: {}", e))?;

        // 获取缓冲区
        let mut buffer = surface.buffer_mut()
            .map_err(|e| format!("获取缓冲区失败: {}", e))?;

        // 清空背景为透明
        buffer.fill(0x00000000);

        // 绘制文本
        Self::draw_text_impl(
            &mut buffer,
            text,
            font_manager,
            font_size,
            color,
            window_width,
            window_height,
            margin,
            scroll_offset,
        );

        // 呈现缓冲区
        buffer.present().map_err(|e| format!("呈现缓冲区失败: {}", e))?;

        Ok(())
    }


    /// 绘制文本
    fn draw_text_impl(
        buffer: &mut [u32],
        text: &str,
        font_manager: &FontManager,
        font_size: f32,
        color: u32,
        window_width: u32,
        window_height: u32,
        margin: u32,
        scroll_offset: f32,
    ) {
        if let Some(font) = font_manager.get_font() {
            // 使用真实字体渲染
            Self::draw_text_with_font(
                buffer,
                text,
                font,
                font_size,
                color,
                window_width,
                window_height,
                margin,
                scroll_offset,
            );
        } else {
            // 使用像素字体备选方案
            let char_height = font_size as u32;
            let char_width = (char_height as f32 * 8.0 / 12.0) as u32;
            
            let available_height = window_height - (margin * 2);
            let text_y = if available_height > char_height {
                margin + (available_height - char_height) / 2
            } else {
                margin
            };
            
            Self::draw_pixel_text(
                buffer,
                text,
                margin,
                text_y,
                color,
                window_width,
                window_height,
                char_width,
                char_height,
                scroll_offset,
            );
        }
    }

    /// 使用真实字体渲染文本（使用 Layout API）
    fn draw_text_with_font(
        buffer: &mut [u32],
        text: &str,
        font: &Font,
        font_size: f32,
        color: u32,
        window_width: u32,
        window_height: u32,
        margin: u32,
        scroll_offset: f32,
    ) {
        use crate::font::layout_text;
        
        let (glyphs, text_width, text_height) = layout_text(font, text, font_size);
        
        if glyphs.is_empty() {
            return;
        }
        
        // 计算文本的整体位置
        let available_width = window_width as f32 - (margin as f32 * 2.0);
        let text_x = if text_width <= available_width {
            // 文本小于窗口宽度，居中显示
            ((window_width as f32 - text_width) / 2.0) as i32
        } else {
            // 文本超出窗口宽度，应用滚动偏移
            (margin as f32 - scroll_offset) as i32
        };
        
        // 计算垂直位置（居中）
        let available_height = window_height as f32 - (margin as f32 * 2.0);
        let text_y = if text_height <= available_height {
            margin as f32 + (available_height - text_height) / 2.0
        } else {
            margin as f32
        };
        
        // 渲染每个字符（只渲染在窗口内的字符）
        for glyph in glyphs {
            let char_x = text_x + glyph.x as i32;
            let char_y = text_y as i32 + glyph.y as i32;
            
            // 检查字符是否在窗口范围内
            if char_x + glyph.width as i32 >= 0 && char_x < window_width as i32 {
                // 使用 parent 字符和 px 尺寸来获取字符的位图数据
                let (metrics, bitmap) = font.rasterize(glyph.parent, glyph.key.px);
                Self::draw_character_bitmap(
                    buffer,
                    &bitmap,
                    &metrics,
                    char_x,
                    char_y,
                    color,
                    window_width,
                    window_height,
                );
            }
        }
    }

    /// 绘制字符位图
    fn draw_character_bitmap(
        buffer: &mut [u32],
        bitmap: &[u8],
        metrics: &fontdue::Metrics,
        char_x: i32,
        char_y: i32,
        color: u32,
        window_width: u32,
        window_height: u32,
    ) {
        for y in 0..metrics.height {
            for x in 0..metrics.width {
                let pixel_x = char_x + x as i32;
                let pixel_y = char_y + y as i32;
                
                if pixel_x >= 0 && pixel_x < window_width as i32 && 
                   pixel_y >= 0 && pixel_y < window_height as i32 {
                    let bitmap_index = y * metrics.width + x;
                    if bitmap_index < bitmap.len() {
                        let alpha = bitmap[bitmap_index];
                        if alpha > 0 {
                            let buffer_index = (pixel_y as u32 * window_width + pixel_x as u32) as usize;
                            if buffer_index < buffer.len() {
                                buffer[buffer_index] = color;
                            }
                        }
                    }
                }
            }
        }
    }

    /// 使用像素字体绘制文本（备选方案）
    fn draw_pixel_text(
        buffer: &mut [u32],
        text: &str,
        x: u32,
        y: u32,
        color: u32,
        window_width: u32,
        window_height: u32,
        char_width: u32,
        char_height: u32,
        scroll_offset: f32,
    ) {
        let chars = text.chars().collect::<Vec<_>>();
        let total_text_width = chars.len() as f32 * char_width as f32;
        let available_width = window_width as f32 - (x as f32 * 2.0);
        
        let start_x = if total_text_width <= available_width {
            // 文本小于窗口宽度，居中显示
            ((window_width as f32 - total_text_width) / 2.0) as u32
        } else {
            // 文本超出窗口宽度，应用滚动偏移
            (x as f32 - scroll_offset).max(0.0) as u32
        };
        
        for (i, ch) in chars.iter().enumerate() {
            let char_x = start_x + (i as u32 * char_width);
            
            // 只绘制在窗口范围内的字符
            if char_x < window_width {
                Self::draw_pixel_char(
                    buffer,
                    *ch,
                    char_x,
                    y,
                    color,
                    window_width,
                    window_height,
                    char_width,
                    char_height,
                );
            }
        }
    }

    /// 绘制单个像素字符
    fn draw_pixel_char(
        buffer: &mut [u32],
        ch: char,
        x: u32,
        y: u32,
        color: u32,
        window_width: u32,
        window_height: u32,
        char_width: u32,
        char_height: u32,
    ) {
        // 简单的8x12像素字体定义
        let pattern = match ch {
            'D' => [
                0b11111000, 0b10000100, 0b10000010, 0b10000010, 0b10000010, 0b10000010,
                0b10000010, 0b10000010, 0b10000010, 0b10000100, 0b11111000, 0b00000000,
            ],
            'E' => [
                0b11111110, 0b10000000, 0b10000000, 0b10000000, 0b11111100, 0b10000000,
                0b10000000, 0b10000000, 0b10000000, 0b10000000, 0b11111110, 0b00000000,
            ],
            'M' => [
                0b10000010, 0b11000110, 0b10101010, 0b10010010, 0b10000010, 0b10000010,
                0b10000010, 0b10000010, 0b10000010, 0b10000010, 0b10000010, 0b00000000,
            ],
            'O' => [
                0b01111100, 0b10000010, 0b10000010, 0b10000010, 0b10000010, 0b10000010,
                0b10000010, 0b10000010, 0b10000010, 0b10000010, 0b01111100, 0b00000000,
            ],
            _ => [0u8; 12], // 空字符
        };

        // 计算缩放比例
        let base_width = 8u32;
        let base_height = 12u32;
        let scale_x = char_width as f32 / base_width as f32;
        let scale_y = char_height as f32 / base_height as f32;

        // 绘制字符像素（带缩放）
        for (row, &bits) in pattern.iter().enumerate() {
            for col in 0..8 {
                if (bits >> (7 - col)) & 1 == 1 {
                    let base_px = col as f32;
                    let base_py = row as f32;
                    
                    let scaled_x_start = (base_px * scale_x) as u32;
                    let scaled_y_start = (base_py * scale_y) as u32;
                    let scaled_x_end = ((base_px + 1.0) * scale_x) as u32;
                    let scaled_y_end = ((base_py + 1.0) * scale_y) as u32;
                    
                    // 绘制缩放后的像素块
                    for py in scaled_y_start..scaled_y_end {
                        for px in scaled_x_start..scaled_x_end {
                            let final_px = x + px;
                            let final_py = y + py;
                            
                            if final_px < window_width && final_py < window_height {
                                let index = (final_py * window_width + final_px) as usize;
                                if index < buffer.len() {
                                    buffer[index] = color;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}
