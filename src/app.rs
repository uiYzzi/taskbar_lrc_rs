use crate::*;
use crate::widget::TaskbarWidget;
use crate::system::set_widget_pointer;
use crate::lyrics::{LyricsManager, LyricsServiceBuilder, LyricsEvent, LyricsState};
use crate::system::{MediaInfo, MediaMonitor, MediaEvent, PlaybackTimer, PlaybackEvent};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::{watch, RwLock};

/// 应用程序状态
#[derive(Debug, Clone)]
pub struct AppState {
    /// 当前媒体信息
    pub media_info: Option<MediaInfo>,
    /// 歌词状态
    pub lyrics_state: LyricsState,
    /// 当前播放位置（实时计算）
    pub current_position: Duration,
    /// 最后更新时间
    pub last_updated: Instant,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            media_info: None,
            lyrics_state: LyricsState::default(),
            current_position: Duration::ZERO,
            last_updated: Instant::now(),
        }
    }
}

/// 应用程序主结构体
pub struct App {
    pub widget: TaskbarWidget,
    last_redraw_time: Instant,
    
    // 核心组件（用于实时获取播放位置）
    playback_timer: Arc<PlaybackTimer>,
    
    // 状态更新通知
    state_update_receiver: watch::Receiver<AppState>,
    
    // 当前应用状态缓存
    current_state: AppState,
    
    // 歌词管理器引用（用于获取下一句歌词时间）
    lyrics_manager: Option<Arc<LyricsManager>>,
}

impl App {
    pub fn new() -> Self {
        // 创建播放定时器
        let (playback_timer, playback_event_receiver) = PlaybackTimer::new();
        let playback_timer = Arc::new(playback_timer);
        
        // 创建状态更新通道
        let (state_update_sender, state_update_receiver) = watch::channel(AppState::default());
        let app_state = Arc::new(RwLock::new(AppState::default()));
        
        let app = Self {
            widget: TaskbarWidget::new(),
            last_redraw_time: Instant::now(),
            playback_timer: playback_timer.clone(),
            state_update_receiver,
            current_state: AppState::default(),
            lyrics_manager: None, // 将在后台服务启动后设置
        };
        
        // 启动后台服务
        app.start_background_services(
            playback_timer,
            app_state,
            state_update_sender,
            playback_event_receiver,
        );
        
        app
    }
    
    /// 设置歌词管理器引用（在后台服务启动后调用）
    pub fn set_lyrics_manager(&mut self, lyrics_manager: Arc<LyricsManager>) {
        self.lyrics_manager = Some(lyrics_manager);
    }
    
    /// 根据播放状态获取合适的更新间隔
    fn get_update_interval(playback_timer: &Arc<PlaybackTimer>) -> Duration {
        if playback_timer.is_playing() {
            Duration::from_millis(50) // 播放时高频更新
        } else {
            Duration::from_millis(500) // 非播放时低频更新
        }
    }
    
    /// 启动后台服务
    fn start_background_services(
        &self,
        playback_timer: Arc<PlaybackTimer>,
        app_state: Arc<RwLock<AppState>>,
        state_update_sender: watch::Sender<AppState>,
        playback_event_receiver: watch::Receiver<PlaybackEvent>,
    ) {
        // 启动事件处理循环
        thread::spawn(move || {
            Self::run_event_loop(
                playback_timer,
                app_state,
                state_update_sender,
                playback_event_receiver,
            );
        });
    }
    
    /// 事件处理循环
    fn run_event_loop(
        playback_timer: Arc<PlaybackTimer>,
        app_state: Arc<RwLock<AppState>>,
        state_update_sender: watch::Sender<AppState>,
        mut playback_event_receiver: watch::Receiver<PlaybackEvent>,
    ) {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        
        rt.block_on(async {
            // 初始化媒体监控
            let (mut media_monitor, mut media_event_receiver) = MediaMonitor::new();
            if let Err(_) = media_monitor.initialize().await {
                return;
            }
            
            // 初始化歌词服务
            let lyrics_service = match LyricsServiceBuilder::new()
                .enable_netease(true)
                .enable_qqmusic(true)
                .with_search_timeout(30)
                .build()
            {
                Ok(service) => service,
                Err(_) => {
                    return;
                }
            };
            
            // 创建歌词管理器
            let (lyrics_manager, mut lyrics_event_receiver) = LyricsManager::new(lyrics_service);
            let lyrics_manager = Arc::new(lyrics_manager);
            
            // 启动媒体监控
            let _media_monitor_handle = {
                let mut monitor = media_monitor;
                tokio::spawn(async move {
                    let _ = monitor.start_monitoring(Duration::from_secs(3)).await;
                })
            };
            
            // 启动播放位置更新循环（按需启动/停止）
            let _position_update_handle = {
                let timer = playback_timer.clone();
                tokio::spawn(async move {
                    let mut last_playing_state = false;
                    let mut update_task: Option<tokio::task::JoinHandle<()>> = None;
                    
                    // 状态检查循环，频率较低
                    let mut state_check_interval = tokio::time::interval(Duration::from_millis(500));
                    
                    loop {
                        state_check_interval.tick().await;
                        let is_playing = timer.is_playing();
                        
                        // 播放状态变化时启动或停止更新任务
                        if is_playing != last_playing_state {
                            if is_playing {
                                // 开始播放，启动高频更新任务
                                if update_task.is_none() {
                                    let timer_clone = timer.clone();
                                    update_task = Some(tokio::spawn(async move {
                                        let mut interval = tokio::time::interval(Duration::from_millis(100));
                                        
                                        while timer_clone.is_playing() {
                                            interval.tick().await;
                                            timer_clone.update_internal_position();
                                        }
                                    }));
                                }
                            } else {
                                // 停止播放，取消更新任务
                                if let Some(task) = update_task.take() {
                                    task.abort();
                                }
                            }
                            last_playing_state = is_playing;
                        }
                    }
                })
            };
            
            // 主事件循环
            loop {
                tokio::select! {
                    // 处理播放事件
                    result = playback_event_receiver.changed() => {
                        if result.is_ok() {
                            let event = playback_event_receiver.borrow().clone();
                            lyrics_manager.handle_playback_event(event).await;
                        }
                    }
                    
                    // 处理媒体事件
                    result = media_event_receiver.changed() => {
                        if result.is_ok() {
                            let event = media_event_receiver.borrow().clone();
                            match &event {
                                MediaEvent::InfoUpdated(media_info) => {
                                    // 同步播放位置到定时器
                                    playback_timer.sync_with_media(media_info).await;
                                    
                                    // 更新应用状态
                                    {
                                        let mut state = app_state.write().await;
                                        state.media_info = Some(media_info.clone());
                                        state.current_position = playback_timer.get_current_position();
                                        state.last_updated = Instant::now();
                                    }
                                }
                                _ => {}
                            }
                            
                            // 传递给歌词管理器
                            lyrics_manager.handle_media_event(event).await;
                        }
                    }
                    
                    // 处理歌词事件
                    result = lyrics_event_receiver.changed() => {
                        if result.is_ok() {
                            let event = lyrics_event_receiver.borrow().clone();
                            match event {
                                LyricsEvent::LoadingStarted { song_info: _ } => {
                                    let mut state = app_state.write().await;
                                    state.lyrics_state.is_loading = true;
                                    state.last_updated = Instant::now();
                                }
                                LyricsEvent::LoadingCompleted { song_info: _, ref lyrics } => {
                                    let mut state = app_state.write().await;
                                    state.lyrics_state.current_lyrics = Some(lyrics.clone());
                                    state.lyrics_state.is_loading = false;
                                    state.last_updated = Instant::now();
                                }
                                LyricsEvent::LoadingFailed { song_info: _, error: _ } => {
                                    let mut state = app_state.write().await;
                                    state.lyrics_state.is_loading = false;
                                    state.last_updated = Instant::now();
                                }
                                LyricsEvent::CurrentLineUpdated { ref line, position } => {
                                    let mut state = app_state.write().await;
                                    state.lyrics_state.current_line = line.clone();
                                    state.current_position = position;
                                    state.last_updated = Instant::now();
                                }
                                LyricsEvent::Cleared => {
                                    let mut state = app_state.write().await;
                                    state.lyrics_state = LyricsState::default();
                                    state.last_updated = Instant::now();
                                }
                            }
                        }
                    }
                    
                    // 智能状态更新（根据播放状态调整更新频率）
                    _ = tokio::time::sleep(Self::get_update_interval(&playback_timer)) => {
                        let current_state = {
                            let mut state = app_state.write().await;
                            
                            let is_playing = playback_timer.is_playing();
                            
                            // 只有在播放时才更新播放位置和歌词行
                            if is_playing {
                                state.current_position = playback_timer.get_current_position();
                                
                                // 实时更新歌词行（仅在播放时）
                                if state.lyrics_state.current_lyrics.is_some() {
                                    if let Some(ref lyrics) = state.lyrics_state.current_lyrics {
                                        let current_line = crate::lyrics::LyricsData::get_current_lyrics_line(
                                            lyrics, 
                                            state.current_position
                                        );
                                        
                                        // 只有在歌词行变化时才更新
                                        if state.lyrics_state.current_line != current_line {
                                            state.lyrics_state.current_line = current_line;
                                        }
                                    }
                                }
                                
                                state.last_updated = Instant::now();
                            }
                            // 暂停或停止时不更新位置和歌词，保持当前状态
                            
                            state.clone()
                        };
                        
                        // 发送状态更新
                        let _ = state_update_sender.send(current_state);
                    }
                }
            }
        });
    }
    
    /// 更新UI状态（从状态通道获取最新状态）
    fn update_ui_state(&mut self) {
        // 检查是否有状态更新
        if self.state_update_receiver.has_changed().unwrap_or(false) {
            self.current_state = self.state_update_receiver.borrow().clone();
        }
        
        // 获取当前播放状态
        let is_playing = if let Some(ref media_info) = self.current_state.media_info {
            use crate::system::PlaybackStatus;
            matches!(media_info.playback_status, PlaybackStatus::Playing)
        } else {
            false
        };
        
        // 只有在播放时才实时更新播放位置
        if is_playing {
            if let Some(ref mut media_info) = self.current_state.media_info {
                let new_position = self.playback_timer.get_current_position();
                media_info.position = Some(new_position);
                self.current_state.current_position = new_position;
            }
        }

        // 更新widget状态
        let old_lyrics_line = self.widget.current_lyrics_line.clone();
        let old_media = self.widget.current_media.clone();
        let old_loading = self.widget.lyrics_loading;
        
        self.widget.current_media = self.current_state.media_info.clone();
        self.widget.current_lyrics = self.current_state.lyrics_state.current_lyrics.clone();
        self.widget.lyrics_loading = self.current_state.lyrics_state.is_loading;
        self.widget.current_lyrics_line = self.current_state.lyrics_state.current_line.clone();
        
        // 检查内容是否发生变化
        let content_changed = old_lyrics_line != self.widget.current_lyrics_line ||
                             old_media != self.widget.current_media ||
                             old_loading != self.widget.lyrics_loading;
        
        if content_changed {
            self.widget.mark_content_changed();
            
            // 内容变化时确保窗口始终在最上层
            self.widget.ensure_topmost();
            
            // 只有在歌词内容真正变化时才重新初始化滚动
            let should_init_scroll = if let Some(current_line) = &self.widget.current_lyrics_line {
                // 检查是否是新的歌词行（避免重复初始化）
                old_lyrics_line.as_ref() != Some(current_line)
            } else {
                false
            };
            
            if should_init_scroll {
                let current_line = self.widget.current_lyrics_line.clone().unwrap();
                // 使用固定时间作为滚动时间（后续可以优化为动态获取）
                let time_to_next_line = Some(Duration::from_secs(8)); // 8秒滚动时间
                self.widget.init_scroll_for_text(&current_line, time_to_next_line);
            }
        }
        
        // 根据播放状态更新窗口可见性
        self.widget.update_window_visibility();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        
        // 初始化小组件
        if let Err(_) = self.widget.initialize(event_loop) {
            return;
        }

        // 在小组件初始化后设置全局指针供事件钩子使用
        set_widget_pointer(&self.widget);

        // 立即触发重绘以显示内容
        self.widget.request_redraw();
        
        // 设置事件循环为持续运行模式，20fps更新频率
        let next_frame_time = Instant::now() + Duration::from_millis(50);
        event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(next_frame_time));
    }

    fn window_event(
        &mut self, 
        event_loop: &winit::event_loop::ActiveEventLoop, 
        _window_id: winit::window::WindowId, 
        event: WindowEvent
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.widget.cleanup();
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                let _ = self.widget.draw_content();
            }
            WindowEvent::MouseInput { .. } => {
                // 处理鼠标点击
                self.widget.ensure_topmost();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let now = Instant::now();
        
        // 更新UI状态（从状态通道获取）
        self.update_ui_state();
        
        // 如果widget正在滚动，需要更新滚动状态
        if self.widget.is_scrolling {
            self.widget.update_scroll();
        }
        
        // 获取当前播放状态
        let is_playing = self.playback_timer.is_playing();
        
        // 只有在窗口应该显示时才进行重绘和其他更新
        if self.widget.should_show_window() {
            // 检查是否需要重绘：内容变化、位置更新或正在滚动
            let should_redraw = self.widget.should_redraw() || 
                               self.widget.position_update_pending || 
                               self.widget.is_scrolling;
            
            if should_redraw {
                self.widget.request_redraw();
                self.last_redraw_time = now;
            }

            // 检查位置更新
            if self.widget.position_update_pending {
                self.widget.schedule_position_update();
            }
            
            // 根据播放状态和滚动状态调整更新频率
            let next_frame_time = if is_playing || self.widget.is_scrolling {
                now + Duration::from_millis(50) // 播放或滚动时使用高频率（20fps）等待下次更新
            } else {
                now + Duration::from_millis(500) // 暂停且未滚动时使用低频率（2fps）
            };
            event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(next_frame_time));
        } else {
            // 窗口隐藏时，降低更新频率，减少资源消耗
            let next_frame_time = now + Duration::from_secs(1); // 1fps更新频率，仅用于状态检查
            event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(next_frame_time));
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}