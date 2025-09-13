use crate::*;
use crate::window::WindowManager;
use crate::graphics::Renderer;
use crate::font::FontManager;
use crate::system::SystemManager;
use crate::window::ensure_taskbar_hidden;

use crate::lyrics::LyricsData;
use crate::system::MediaInfo;

use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};

/// 任务栏小组件的核心结构体
pub struct TaskbarWidget {
    pub window_manager: WindowManager,
    pub renderer: Renderer,
    pub font_manager: FontManager,
    pub system_manager: SystemManager,
    pub window_width: u32,
    pub window_height: u32,
    pub show_on_left: bool,
    pub last_position_update: Instant,
    pub position_update_pending: bool,
    pub last_taskbar_rect: RECT,
    pub last_notify_rect: RECT,
    pub current_lyrics: Option<LyricsData>,
    pub current_media: Option<MediaInfo>,
    pub last_lyrics_update: Instant,
    pub lyrics_loading: bool,
    pub current_lyrics_line: Option<String>,
    pub last_rendered_content: String,
    pub content_changed: bool,
    
    // 滚动相关字段
    pub scroll_offset: f32,
    pub scroll_speed: f32,
    pub scroll_target_time: Option<Duration>,
    pub scroll_start_time: Option<Instant>,
    pub text_width: f32,
    pub is_scrolling: bool,
}

impl TaskbarWidget {
    pub fn new() -> Self {
        Self {
            window_manager: WindowManager::new(),
            renderer: Renderer::new(),
            font_manager: FontManager::new(),
            system_manager: SystemManager::new(),
            window_width: 280,
            window_height: 40,
            show_on_left: false,
            last_position_update: Instant::now(),
            position_update_pending: false,
            last_taskbar_rect: RECT::default(),
            last_notify_rect: RECT::default(),
            current_lyrics: None,
            current_media: None,
            last_lyrics_update: Instant::now(),
            lyrics_loading: false,
            current_lyrics_line: None,
            last_rendered_content: String::new(),
            content_changed: true, // 初始时需要绘制
            
            // 滚动相关字段初始化
            scroll_offset: 0.0,
            scroll_speed: 0.0,
            scroll_target_time: None,
            scroll_start_time: None,
            text_width: 0.0,
            is_scrolling: false,
        }
    }

    /// 初始化小组件
    pub fn initialize(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) -> std::result::Result<(), String> {
        // 查找任务栏并获取高度
        self.system_manager.find_taskbar_handle()?;
        
        // 根据任务栏高度调整窗口高度
        let taskbar_height = self.system_manager.get_taskbar_height();
        self.window_height = taskbar_height.max(32).min(100);
        
        // 创建窗口
        self.window_manager.create_window(
            event_loop, 
            self.window_width, 
            self.window_height
        )?;
        
        // 初始化渲染器
        if let Some(window) = self.window_manager.get_window() {
            self.renderer.initialize(window)?;
        }
        
        // 保存初始任务栏和通知区域位置
        self.last_taskbar_rect = self.system_manager.get_taskbar_rect();
        self.last_notify_rect = self.system_manager.get_notify_area_rect();
        
        // 调整窗口位置
        self.adjust_window_position()?;
        
        // 设置系统事件钩子
        self.system_manager.setup_event_hook()?;
        
        // 确保窗口在最上层
        self.ensure_topmost();
        
        Ok(())
    }

    /// 调整窗口位置
    pub fn adjust_window_position(&mut self) -> std::result::Result<(), String> {
        let _window = self.window_manager.get_window()
            .ok_or("窗口未创建")?;
        
        // 重新获取任务栏信息，确保使用最新数据
        let _ = self.system_manager.find_taskbar_handle();
            
        let taskbar_rect = self.system_manager.get_taskbar_rect();
        let notify_rect = self.system_manager.get_notify_area_rect();
        
        // 检查任务栏或通知区域位置是否真的有变化
        let taskbar_changed = self.last_taskbar_rect.left != taskbar_rect.left ||
                             self.last_taskbar_rect.top != taskbar_rect.top ||
                             self.last_taskbar_rect.right != taskbar_rect.right ||
                             self.last_taskbar_rect.bottom != taskbar_rect.bottom;
        
        let notify_changed = self.last_notify_rect.left != notify_rect.left ||
                            self.last_notify_rect.top != notify_rect.top ||
                            self.last_notify_rect.right != notify_rect.right ||
                            self.last_notify_rect.bottom != notify_rect.bottom;
        
        if !taskbar_changed && !notify_changed && self.last_taskbar_rect.left != 0 {
            return Ok(());
        }
        
        self.last_taskbar_rect = taskbar_rect;
        self.last_notify_rect = notify_rect;
        
        // 计算窗口位置
        let new_x = if self.show_on_left {
            taskbar_rect.left + 60
        } else {
            if notify_rect.left != 0 {
                notify_rect.left - self.window_width as i32 - 5
            } else {
                taskbar_rect.right - self.window_width as i32 - 60
            }
        };
        
        let new_y = taskbar_rect.top;
        
        // 使用窗口管理器设置位置
        self.window_manager.set_position(new_x, new_y, self.window_width, self.window_height)?;
        
        // 位置更新后立即确保最上层，防止被其他窗口遮挡
        self.ensure_topmost();
        
        // 确保任务栏图标隐藏（位置变更可能影响窗口样式）
        if let Some(window) = self.window_manager.get_window() {
            ensure_taskbar_hidden(window);
        }
        
        Ok(())
    }

    /// 确保窗口始终在最上层
    pub fn ensure_topmost(&self) {
        self.window_manager.ensure_topmost();
        
        // 同时确保任务栏图标隐藏
        if let Some(window) = self.window_manager.get_window() {
            ensure_taskbar_hidden(window);
        }
    }

    /// 绘制内容
    pub fn draw_content(&mut self) -> std::result::Result<(), String> {
        // 先检查并更新窗口可见性
        self.update_window_visibility();
        
        // 如果窗口应该隐藏，则不需要绘制内容
        if !self.should_show_window() {
            return Ok(());
        }
        
        // 更新滚动状态
        self.update_scroll();
        
        // 获取要显示的歌词文本
        let text = self.get_display_lyrics();
        let margin = (self.window_height as f32 * 0.25) as u32;
        let font_size = (self.window_height as f32 * 0.4) as f32; // 稍微小一点适应歌词

        // 使用黑色
        let color = 0xFF000000;
        
        // 获取滚动偏移量
        let scroll_offset = self.get_scroll_offset();
        
        let result = self.renderer.draw_frame(
            &text,
            &self.font_manager,
            font_size,
            color,
            self.window_width,
            self.window_height,
            margin,
            scroll_offset,
        );
        
        // 绘制完成后标记重绘完成
        if result.is_ok() {
            self.mark_redraw_complete();
        }
        
        result
    }

    /// 检查是否需要重新计算位置（防抖逻辑）
    pub fn should_update_position(&mut self) -> bool {
        let now = Instant::now();
        let duration_since_last = now.duration_since(self.last_position_update);
        
        if duration_since_last >= Duration::from_millis(200) {
            self.last_position_update = now;
            true
        } else if self.position_update_pending {
            // 如果有pending标志，但防抖时间未到，暂时不更新，保持pending状态
            false
        } else {
            false
        }
    }

    /// 延迟位置更新
    pub fn schedule_position_update(&mut self) {
        if self.should_update_position() {
            // 清除pending标志，因为我们即将执行更新
            self.position_update_pending = false;
            let _ = self.adjust_window_position();
        }
    }

    /// 获取窗口句柄
    pub fn get_window_hwnd(&self) -> Option<HWND> {
        self.window_manager.get_hwnd()
    }

    /// 检查窗口是否与任务栏相关
    pub fn is_taskbar_related_window(&self, hwnd: HWND) -> bool {
        self.system_manager.is_taskbar_related_window(hwnd)
    }

    /// 显示窗口
    pub fn show_window(&self) {
        if let Some(window) = self.window_manager.get_window() {
            // 在显示窗口前确保任务栏图标隐藏
            ensure_taskbar_hidden(window);
            
            if let Some(hwnd) = self.get_window_hwnd() {
                unsafe {
                    // 使用 SW_SHOWNOACTIVATE 显示窗口，但不激活它
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                }
            }
            
            // 显示窗口后立即确保最上层并再次确认任务栏隐藏
            self.ensure_topmost();
            ensure_taskbar_hidden(window);
        }
    }

    /// 隐藏窗口
    pub fn hide_window(&self) {
        if let Some(hwnd) = self.get_window_hwnd() {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
    }

    /// 检查是否应该显示窗口（根据播放状态）
    pub fn should_show_window(&self) -> bool {
        if let Some(media) = &self.current_media {
            use crate::system::PlaybackStatus;
            match media.playback_status {
                PlaybackStatus::Playing => true,
                PlaybackStatus::Paused | PlaybackStatus::Stopped | PlaybackStatus::Unknown => false,
            }
        } else {
            false // 没有媒体信息时隐藏窗口
        }
    }

    /// 更新窗口显示状态（根据播放状态自动显示或隐藏）
    pub fn update_window_visibility(&self) {
        if self.should_show_window() {
            self.show_window();
        } else {
            self.hide_window();
        }
    }

    /// 检查并更新内容变化状态
    pub fn check_content_changed(&mut self) -> bool {
        let current_content = self.get_display_lyrics();
        let content_changed = current_content != self.last_rendered_content;
        
        if content_changed {
            self.last_rendered_content = current_content;
            self.content_changed = true;
        }
        
        content_changed
    }

    /// 检查是否需要重绘（内容有变化或窗口可见性有变化）
    pub fn should_redraw(&mut self) -> bool {
        let visibility_changed = self.should_show_window();
        let content_changed = self.check_content_changed();
        
        // 如果窗口应该显示且内容有变化，或者可见性有变化，则需要重绘
        (visibility_changed && content_changed) || self.content_changed
    }

    /// 标记内容发生变化（在更新歌词或媒体信息时调用）
    pub fn mark_content_changed(&mut self) {
        self.content_changed = true;
    }

    /// 标记重绘完成
    pub fn mark_redraw_complete(&mut self) {
        self.content_changed = false;
    }
    
    /// 获取要显示的歌词文本
    pub fn get_display_lyrics(&self) -> String {
        if self.lyrics_loading {
            return "正在加载歌词...".to_string();
        }
        
        if let Some(media) = &self.current_media {
            // 优先使用预计算的当前歌词行
            if let Some(ref current_line) = self.current_lyrics_line {
                if !current_line.trim().is_empty() {
                    return current_line.clone();
                }
            }
            
            // 如果有歌词数据但没有当前行，显示无歌词提示
            if self.current_lyrics.is_some() {
                return "♪ 暂无歌词 ♪".to_string();
            } else {
                // 正在播放但没有歌词，显示歌曲信息
                return format!("{} - {}", media.artist, media.title);
            }
        }
        
        "等待播放音乐...".to_string()
    }

    /// 计算文本宽度
    pub fn calculate_text_width(&mut self, text: &str) -> f32 {
        let font_size = (self.window_height as f32 * 0.4) as f32;
        
        if let Some(font) = self.font_manager.get_font() {
            use crate::font::get_layout_text_width;
            get_layout_text_width(font, text, font_size)
        } else {
            // 使用像素字体的计算
            let char_width = (font_size * 8.0 / 12.0) as u32;
            use crate::font::get_pixel_text_width;
            get_pixel_text_width(text, char_width) as f32
        }
    }

    /// 初始化滚动（当歌词内容变化时调用）
    pub fn init_scroll_for_text(&mut self, text: &str, time_to_next_line: Option<Duration>) {
        self.text_width = self.calculate_text_width(text);
        let available_width = self.window_width as f32 - (self.window_height as f32 * 0.5); // 左右留出一些边距
        
        // 只有在状态变化时才输出调试信息
        let _was_scrolling = self.is_scrolling;
        
        if self.text_width > available_width {
            self.is_scrolling = true;
            self.scroll_offset = 0.0;
            self.scroll_start_time = Some(Instant::now());
            
            if let Some(duration_to_next) = time_to_next_line {
                let total_scroll_distance = self.text_width - available_width + 50.0;
                let duration_seconds = duration_to_next.as_secs_f32().max(1.0);
                self.scroll_speed = total_scroll_distance / duration_seconds;
                self.scroll_target_time = time_to_next_line;
            } else {
                self.scroll_speed = 20.0;
                self.scroll_target_time = None;
            }
        } else {
            self.is_scrolling = false;
            self.scroll_offset = 0.0;
            self.scroll_speed = 0.0;
            self.scroll_start_time = None;
            self.scroll_target_time = None;
        }
    }

    /// 更新滚动位置（在每帧调用）
    pub fn update_scroll(&mut self) {
        if !self.is_scrolling {
            return;
        }
        
        let now = Instant::now();
        if let Some(start_time) = self.scroll_start_time {
            let elapsed = now.duration_since(start_time).as_secs_f32();
            
            if let Some(target_time) = self.scroll_target_time {
                if elapsed >= target_time.as_secs_f32() {
                    let available_width = self.window_width as f32 - (self.window_height as f32 * 0.5);
                    self.scroll_offset = (self.text_width - available_width + 50.0).max(0.0);
                    self.is_scrolling = false;
                    return;
                }
            }
            
            let _old_offset = self.scroll_offset;
            self.scroll_offset = elapsed * self.scroll_speed;
            
            // 防止过度滚动
            let available_width = self.window_width as f32 - (self.window_height as f32 * 0.5);
            let max_scroll = (self.text_width - available_width + 50.0).max(0.0);
            if self.scroll_offset >= max_scroll {
                self.scroll_offset = max_scroll;
                self.is_scrolling = false;
            }
        }
    }

    /// 获取当前滚动偏移量
    pub fn get_scroll_offset(&self) -> f32 {
        // 始终返回当前的滚动偏移量，无论是否正在滚动
        self.scroll_offset
    }

    /// 清理资源
    pub fn cleanup(&mut self) {
        self.system_manager.cleanup();
    }

    /// 请求重绘
    pub fn request_redraw(&self) {
        if let Some(window) = self.window_manager.get_window() {
            window.request_redraw();
        }
    }
}

impl Default for TaskbarWidget {
    fn default() -> Self {
        Self::new()
    }
}
