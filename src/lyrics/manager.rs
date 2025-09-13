use std::time::{Duration, Instant};
use std::collections::HashMap;
use tokio::sync::{watch, RwLock};
use tracing::{debug, info, warn};

use crate::lyrics::{LyricsData, LyricsService, SongInfo};
use crate::system::{PlaybackEvent, MediaEvent};

/// 歌词事件
#[derive(Debug, Clone)]
pub enum LyricsEvent {
    /// 歌词加载开始
    LoadingStarted {
        song_info: SongInfo,
    },
    /// 歌词加载完成
    LoadingCompleted {
        song_info: SongInfo,
        lyrics: LyricsData,
    },
    /// 歌词加载失败
    LoadingFailed {
        song_info: SongInfo,
        error: String,
    },
    /// 当前歌词行更新
    CurrentLineUpdated {
        line: Option<String>,
        position: Duration,
    },
    /// 歌词清空
    Cleared,
}

/// 实时歌词状态
#[derive(Debug, Clone)]
pub struct LyricsState {
    /// 当前歌曲信息
    pub current_song: Option<SongInfo>,
    /// 当前歌词数据
    pub current_lyrics: Option<LyricsData>,
    /// 是否正在加载
    pub is_loading: bool,
    /// 当前歌词行
    pub current_line: Option<String>,
    /// 当前播放位置
    pub current_position: Duration,
    /// 最后更新时间
    pub last_updated: Instant,
}

impl Default for LyricsState {
    fn default() -> Self {
        Self {
            current_song: None,
            current_lyrics: None,
            is_loading: false,
            current_line: None,
            current_position: Duration::ZERO,
            last_updated: Instant::now(),
        }
    }
}

/// 歌词管理器
/// 负责歌词获取、缓存和实时匹配
pub struct LyricsManager {
    /// 歌词服务
    lyrics_service: LyricsService,
    /// 当前状态
    state: RwLock<LyricsState>,
    /// 事件发送器
    event_sender: watch::Sender<LyricsEvent>,
    /// 解析后的歌词缓存 (歌曲信息 -> 时间戳歌词列表)
    parsed_lyrics_cache: RwLock<HashMap<SongInfo, Vec<(u64, String)>>>,
}

impl LyricsManager {
    /// 创建新的歌词管理器
    pub fn new(lyrics_service: LyricsService) -> (Self, watch::Receiver<LyricsEvent>) {
        let (event_sender, event_receiver) = watch::channel(LyricsEvent::Cleared);
        
        let manager = Self {
            lyrics_service,
            state: RwLock::new(LyricsState::default()),
            event_sender,
            parsed_lyrics_cache: RwLock::new(HashMap::new()),
        };
        
        (manager, event_receiver)
    }

    /// 处理播放事件
    pub async fn handle_playback_event(&self, event: PlaybackEvent) {
        match event {
            PlaybackEvent::SongChanged { title, artist, .. } => {
                let song_info = SongInfo::new(title, artist);
                info!("播放事件：歌曲切换 -> {}", song_info);
                
                // 检查是否与当前歌曲相同（避免重复加载）
                let state = self.state.read().await;
                let is_different_song = state.current_song.as_ref() != Some(&song_info);
                drop(state);
                
                if is_different_song {
                    self.load_lyrics_for_song(song_info).await;
                } else {
                    debug!("播放事件：歌曲无变化，跳过歌词加载");
                }
            }
            PlaybackEvent::PositionUpdate { position } => {
                // 仅在播放时更新位置和歌词行
                self.update_current_position(position).await;
            }
            PlaybackEvent::PlayStateChanged { position, is_playing } => {
                // 更新位置，但不在此处更新歌词行（由上层应用控制）
                {
                    let mut state = self.state.write().await;
                    state.current_position = position;
                    state.last_updated = Instant::now();
                }
                
                // 只有在开始播放时才更新歌词行
                if is_playing {
                    self.update_current_lyrics_line(position).await;
                }
            }
            PlaybackEvent::Reset => {
                info!("播放事件：重置");
                self.clear_lyrics().await;
            }
        }
    }

    /// 处理媒体事件
    pub async fn handle_media_event(&self, event: MediaEvent) {
        match event {
            MediaEvent::InfoUpdated(media_info) => {
                if !media_info.title.is_empty() && !media_info.artist.is_empty() {
                    let song_info = SongInfo::new(&media_info.title, &media_info.artist);
                    
                    // 检查是否是新歌曲
                    let state = self.state.read().await;
                    let is_new_song = state.current_song.as_ref() != Some(&song_info);
                    let old_song = state.current_song.clone();
                    drop(state);
                    
                    if is_new_song {
                        info!("检测到歌曲切换: {:?} -> {:?}", old_song, song_info);
                        
                        // 立即设置为加载状态，并更新歌曲信息，这样界面会立即显示歌曲信息
                        {
                            let mut state = self.state.write().await;
                            state.current_song = Some(song_info.clone());
                            state.is_loading = true; // 设置为加载中
                            state.current_line = None;
                            state.current_lyrics = None;
                            state.current_position = Duration::ZERO;
                            state.last_updated = Instant::now();
                        }
                        
                        // 清空旧歌曲的缓存（如果存在）
                        if let Some(old_song_info) = old_song {
                            self.parsed_lyrics_cache.write().await.remove(&old_song_info);
                        }
                        
                        // 发送加载开始事件，让界面立即显示歌曲信息
                        let _ = self.event_sender.send(LyricsEvent::LoadingStarted {
                            song_info: song_info.clone(),
                        });
                        
                        // 加载新歌词
                        self.load_lyrics_for_song(song_info).await;
                    }
                    
                    // 更新播放位置
                    if let Some(position) = media_info.position {
                        self.update_current_position(position).await;
                    }
                } else {
                    // 媒体信息为空时，清空歌词
                    self.clear_lyrics().await;
                }
            }
            MediaEvent::Error(_) | MediaEvent::Stopped => {
                self.clear_lyrics().await;
            }
        }
    }

    /// 为指定歌曲加载歌词
    async fn load_lyrics_for_song(&self, song_info: SongInfo) {
        info!("开始加载歌词: {}", song_info);
        
        // 检查是否已经设置为加载状态，如果没有则设置
        {
            let mut state = self.state.write().await;
            if state.current_song != Some(song_info.clone()) || !state.is_loading {
                state.current_song = Some(song_info.clone());
                state.is_loading = true;
                state.current_lyrics = None;
                state.current_line = None;
                state.current_position = Duration::ZERO;
                state.last_updated = Instant::now();
                
                // 发送加载开始事件
                let _ = self.event_sender.send(LyricsEvent::LoadingStarted {
                    song_info: song_info.clone(),
                });
            }
        }
        
        // 异步加载歌词
        match self.lyrics_service.search_and_get_lyrics(&song_info).await {
            Ok(lyrics_data) => {
                info!("成功加载歌词: {}", song_info);
                
                // 解析歌词并缓存
                if let Some(original_lyrics) = &lyrics_data.original {
                    let parsed_lyrics = self.parse_lyrics_to_timestamps(original_lyrics);
                    self.parsed_lyrics_cache.write().await.insert(song_info.clone(), parsed_lyrics);
                }
                
                // 更新状态
                {
                    let mut state = self.state.write().await;
                    state.current_lyrics = Some(lyrics_data.clone());
                    state.is_loading = false;
                    state.last_updated = Instant::now();
                }
                
                // 发送加载完成事件
                let _ = self.event_sender.send(LyricsEvent::LoadingCompleted {
                    song_info,
                    lyrics: lyrics_data,
                });
                
                // 立即更新当前歌词行
                let current_position = self.state.read().await.current_position;
                self.update_current_lyrics_line(current_position).await;
            }
            Err(e) => {
                warn!("加载歌词失败: {} - {}", song_info, e);
                
                // 更新状态
                {
                    let mut state = self.state.write().await;
                    state.is_loading = false;
                    state.last_updated = Instant::now();
                }
                
                // 发送加载失败事件
                let _ = self.event_sender.send(LyricsEvent::LoadingFailed {
                    song_info,
                    error: e.to_string(),
                });
            }
        }
    }

    /// 更新当前播放位置（仅在播放时更新歌词行）
    async fn update_current_position(&self, position: Duration) {
        {  
            let mut state = self.state.write().await;
            state.current_position = position;
            state.last_updated = Instant::now();
        }
        
        // 由于此方法只在播放事件中被调用，而播放事件只在播放时触发，
        // 所以可以安全地更新歌词行
        self.update_current_lyrics_line(position).await;
    }

    /// 更新当前歌词行
    async fn update_current_lyrics_line(&self, position: Duration) {
        let (song_info, is_loading) = {
            let state = self.state.read().await;
            match &state.current_song {
                Some(song) => (song.clone(), state.is_loading),
                None => return,
            }
        };
        
        // 如果正在加载歌词，不进行歌词行匹配，避免使用旧缓存
        if is_loading {
            return;
        }
        
        // 从缓存获取解析后的歌词
        let parsed_lyrics = {
            let cache = self.parsed_lyrics_cache.read().await;
            cache.get(&song_info).cloned()
        };
        
        let current_line = if let Some(lyrics_list) = parsed_lyrics {
            self.find_current_lyrics_line(&lyrics_list, position)
        } else {
            None
        };
        
        // 更新状态中的当前歌词行
        {
            let mut state = self.state.write().await;
            let line_changed = state.current_line != current_line;
            state.current_line = current_line.clone();
            state.current_position = position; // 同步更新播放位置
            
            // 只有在歌词行改变时才发送事件
            if line_changed {
                drop(state);
                let _ = self.event_sender.send(LyricsEvent::CurrentLineUpdated {
                    line: current_line,
                    position,
                });
            }
        }
    }

    /// 解析歌词为时间戳列表
    fn parse_lyrics_to_timestamps(&self, lyrics: &str) -> Vec<(u64, String)> {
        let mut lyrics_lines = Vec::new();
        
        for line in lyrics.lines() {
            let line = line.trim();
            if !line.is_empty() && line.starts_with('[') {
                if let Some(close_bracket) = line.find(']') {
                    let time_part = &line[1..close_bracket];
                    let lyrics_content = &line[close_bracket + 1..].trim();
                    
                    // 解析时间戳
                    if let Some(timestamp_ms) = LyricsData::parse_lrc_timestamp(time_part) {
                        lyrics_lines.push((timestamp_ms, lyrics_content.to_string()));
                    }
                }
            }
        }
        
        // 按时间排序
        lyrics_lines.sort_by_key(|&(time, _)| time);
        lyrics_lines
    }

    /// 根据当前播放时间查找对应的歌词行
    fn find_current_lyrics_line(&self, lyrics_list: &[(u64, String)], position: Duration) -> Option<String> {
        let current_ms = position.as_millis() as u64;
        
        let mut current_lyrics = None;
        for (timestamp, lyrics_text) in lyrics_list {
            if *timestamp <= current_ms {
                if !lyrics_text.is_empty() {
                    current_lyrics = Some(lyrics_text.clone());
                }
            } else {
                break;
            }
        }
        
        current_lyrics
    }

    /// 获取下一句歌词的开始时间（用于计算滚动速度）
    pub async fn get_next_lyrics_time(&self, current_position: Duration) -> Option<Duration> {
        let state = self.state.read().await;
        let song_info = state.current_song.as_ref()?.clone();
        drop(state);
        
        // 从缓存获取解析后的歌词
        let cache = self.parsed_lyrics_cache.read().await;
        let lyrics_list = cache.get(&song_info)?.clone();
        drop(cache);
        
        let current_ms = current_position.as_millis() as u64;
        
        // 找到下一句歌词的时间戳
        for (timestamp, lyrics_text) in lyrics_list {
            if timestamp > current_ms && !lyrics_text.is_empty() {
                return Some(Duration::from_millis(timestamp));
            }
        }
        
        None
    }

    /// 清空歌词
    async fn clear_lyrics(&self) {
        info!("清空歌词状态");
        
        {
            let mut state = self.state.write().await;
            let old_song = state.current_song.clone();
            
            state.current_song = None;
            state.current_lyrics = None;
            state.is_loading = false;
            state.current_line = None;
            state.current_position = Duration::ZERO;
            state.last_updated = Instant::now();
            
            if let Some(song) = old_song {
                debug!("清空歌曲: {}", song);
            }
        }
        
        // 清空缓存
        let cache_size = {
            let mut cache = self.parsed_lyrics_cache.write().await;
            let size = cache.len();
            cache.clear();
            size
        };
        
        if cache_size > 0 {
            debug!("清空歌词缓存，共 {} 项", cache_size);
        }
        
        // 发送清空事件
        let _ = self.event_sender.send(LyricsEvent::Cleared);
    }

    /// 获取当前状态
    pub async fn get_current_state(&self) -> LyricsState {
        self.state.read().await.clone()
    }

    /// 获取当前歌词行（快速访问）
    pub async fn get_current_line(&self) -> Option<String> {
        self.state.read().await.current_line.clone()
    }

    /// 检查是否正在加载
    pub async fn is_loading(&self) -> bool {
        self.state.read().await.is_loading
    }

    /// 订阅歌词事件
    pub fn subscribe(&self) -> watch::Receiver<LyricsEvent> {
        self.event_sender.subscribe()
    }

    /// 手动刷新歌词（强制重新加载）
    pub async fn refresh_lyrics(&self) {
        let song_info = {
            let state = self.state.read().await;
            state.current_song.clone()
        };
        
        if let Some(song_info) = song_info {
            // 清除缓存
            self.parsed_lyrics_cache.write().await.remove(&song_info);
            // 重新加载
            self.load_lyrics_for_song(song_info).await;
        }
    }

    /// 预加载歌词
    pub async fn preload_lyrics(&self, songs: Vec<SongInfo>) {
        for song_info in songs {
            if let Err(e) = self.lyrics_service.search_and_get_lyrics(&song_info).await {
                debug!("预加载歌词失败: {} - {}", song_info, e);
            }
        }
    }
}
