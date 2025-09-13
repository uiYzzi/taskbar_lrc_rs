use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::{watch, RwLock};
use tracing::{info, debug};
use crate::system::MediaInfo;

/// 播放状态事件
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackEvent {
    /// 歌曲变更
    SongChanged {
        title: String,
        artist: String,
        duration: Option<Duration>,
    },
    /// 播放状态变更
    PlayStateChanged {
        is_playing: bool,
        position: Duration,
    },
    /// 播放位置更新
    PositionUpdate {
        position: Duration,
    },
    /// 播放停止/重置
    Reset,
}

/// 精确的播放位置跟踪器
/// 使用原子操作和精确定时器实现高性能实时跟踪
#[derive(Debug)]
pub struct PlaybackTimer {
    /// 是否正在播放（原子变量，避免锁争用）
    is_playing: AtomicBool,
    /// 基准播放位置（毫秒，原子变量）
    base_position_ms: AtomicU64,
    /// 歌曲总时长（毫秒，原子变量）
    duration_ms: AtomicU64,
    /// 上次更新时的系统时间戳（毫秒，原子变量）
    last_update_timestamp: AtomicU64,
    /// 当前歌曲信息（读写锁保护）
    current_song: RwLock<Option<(String, String)>>,
    /// 事件发送器
    event_sender: watch::Sender<PlaybackEvent>,
    /// 内部启动时间（用于计算相对时间戳）
    start_time: Instant,
}

impl PlaybackTimer {
    pub fn new() -> (Self, watch::Receiver<PlaybackEvent>) {
        let start_time = Instant::now();
        let (event_sender, event_receiver) = watch::channel(PlaybackEvent::Reset);
        
        let timer = Self {
            is_playing: AtomicBool::new(false),
            base_position_ms: AtomicU64::new(0),
            duration_ms: AtomicU64::new(0),
            last_update_timestamp: AtomicU64::new(Self::current_timestamp_ms(start_time)),
            current_song: RwLock::new(None),
            event_sender,
            start_time,
        };
        
        (timer, event_receiver)
    }

    /// 获取当前时间戳（毫秒）
    fn current_timestamp_ms(start_time: Instant) -> u64 {
        start_time.elapsed().as_millis() as u64
    }

    /// 获取当前实时播放位置（无锁，高性能）
    pub fn get_current_position(&self) -> Duration {
        let is_playing = self.is_playing.load(Ordering::Relaxed);
        let base_position_ms = self.base_position_ms.load(Ordering::Relaxed);
        
        if !is_playing {
            return Duration::from_millis(base_position_ms);
        }
        
        // 计算当前位置
        let last_update_timestamp = self.last_update_timestamp.load(Ordering::Relaxed);
        let current_timestamp = Self::current_timestamp_ms(self.start_time);
        let elapsed_ms = current_timestamp.saturating_sub(last_update_timestamp);
        let current_position_ms = base_position_ms + elapsed_ms;
        
        // 检查时长限制
        let duration_ms = self.duration_ms.load(Ordering::Relaxed);
        if duration_ms > 0 && current_position_ms > duration_ms {
            Duration::from_millis(duration_ms)
        } else {
            Duration::from_millis(current_position_ms)
        }
    }
    
    /// 更新内部播放位置（定期调用以保持精度）
    /// 只有在播放时才会被调用，避免不必要的计算
    pub fn update_internal_position(&self) {
        if !self.is_playing.load(Ordering::Relaxed) {
            return;
        }
        
        let current_timestamp = Self::current_timestamp_ms(self.start_time);
        let last_update_timestamp = self.last_update_timestamp.load(Ordering::Relaxed);
        let elapsed_ms = current_timestamp.saturating_sub(last_update_timestamp);
        
        // 只有在时间间隔足够大时才更新，减少不必要的操作
        if elapsed_ms >= 50 { // 只有在超过50ms时才更新
            let old_position_ms = self.base_position_ms.load(Ordering::Relaxed);
            let new_position_ms = old_position_ms + elapsed_ms;
            
            // 检查时长限制
            let duration_ms = self.duration_ms.load(Ordering::Relaxed);
            let final_position_ms = if duration_ms > 0 && new_position_ms > duration_ms {
                duration_ms
            } else {
                new_position_ms
            };
            
            self.base_position_ms.store(final_position_ms, Ordering::Relaxed);
            self.last_update_timestamp.store(current_timestamp, Ordering::Relaxed);
            
            // 只有在位置有显著变化时才发送事件（减少事件频率）
            if elapsed_ms >= 100 { // 只有在超过100ms变化时才发送事件
                let _ = self.event_sender.send(PlaybackEvent::PositionUpdate {
                    position: Duration::from_millis(final_position_ms),
                });
            }
        }
    }

    /// 同步媒体信息（由媒体监测器定期调用）
    pub async fn sync_with_media(&self, media: &MediaInfo) {
        let current_song = self.current_song.read().await.clone();
        let new_song = if media.title.is_empty() || media.artist.is_empty() {
            None
        } else {
            Some((media.title.clone(), media.artist.clone()))
        };
        
        // 检查是否是新歌曲
        let is_new_song = current_song != new_song;
        
        if is_new_song {
            info!("播放定时器检测到歌曲切换: {:?} -> {:?}", current_song, new_song);
            
            // 更新歌曲信息
            *self.current_song.write().await = new_song.clone();
            
            // 重置所有状态
            let new_position = media.position.unwrap_or(Duration::ZERO);
            let new_duration = media.duration.unwrap_or(Duration::ZERO);
            let new_playing = matches!(media.playback_status, crate::system::PlaybackStatus::Playing);
            
            self.base_position_ms.store(new_position.as_millis() as u64, Ordering::Relaxed);
            self.duration_ms.store(new_duration.as_millis() as u64, Ordering::Relaxed);
            self.is_playing.store(new_playing, Ordering::Relaxed);
            self.last_update_timestamp.store(Self::current_timestamp_ms(self.start_time), Ordering::Relaxed);
            
            // 发送歌曲变更事件
            if let Some((title, artist)) = new_song {
                let _ = self.event_sender.send(PlaybackEvent::SongChanged {
                    title,
                    artist,
                    duration: media.duration,
                });
            } else {
                let _ = self.event_sender.send(PlaybackEvent::Reset);
            }
        } else if current_song.is_some() {
            // 同一首歌，校准播放状态和位置
            let new_playing = matches!(media.playback_status, crate::system::PlaybackStatus::Playing);
            let old_playing = self.is_playing.load(Ordering::Relaxed);
            
            // 如果播放状态发生变化
            if old_playing != new_playing {
                self.is_playing.store(new_playing, Ordering::Relaxed);
                self.last_update_timestamp.store(Self::current_timestamp_ms(self.start_time), Ordering::Relaxed);
                
                let current_position = self.get_current_position();
                
                // 发送播放状态变更事件
                let _ = self.event_sender.send(PlaybackEvent::PlayStateChanged {
                    is_playing: new_playing,
                    position: current_position,
                });
            }
            
            // 校准播放位置（如果有实际位置信息）
            if let Some(actual_position) = media.position {
                let current_pos = self.get_current_position();
                let position_diff = if actual_position > current_pos {
                    actual_position - current_pos
                } else {
                    current_pos - actual_position
                };
                
                // 如果位置偏差超过1秒，进行校正
                if position_diff > Duration::from_secs(1) {
                    debug!("校正播放位置偏差: {:?} -> {:?} (偏差: {:?})", current_pos, actual_position, position_diff);
                    self.base_position_ms.store(actual_position.as_millis() as u64, Ordering::Relaxed);
                    self.last_update_timestamp.store(Self::current_timestamp_ms(self.start_time), Ordering::Relaxed);
                    
                    // 发送位置更新事件
                    let _ = self.event_sender.send(PlaybackEvent::PositionUpdate {
                        position: actual_position,
                    });
                }
            }
        }
    }

    /// 获取当前歌曲信息（异步版本）
    pub async fn get_current_song(&self) -> Option<(String, String)> {
        self.current_song.read().await.clone()
    }

    /// 获取当前歌曲信息（同步版本，非阻塞）
    pub fn try_get_current_song(&self) -> Option<(String, String)> {
        self.current_song.try_read().ok()?.clone()
    }

    /// 获取歌曲总时长
    pub fn get_duration(&self) -> Option<Duration> {
        let duration_ms = self.duration_ms.load(Ordering::Relaxed);
        if duration_ms > 0 {
            Some(Duration::from_millis(duration_ms))
        } else {
            None
        }
    }

    /// 检查是否正在播放
    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }

    /// 重置定时器
    pub async fn reset(&self) {
        self.base_position_ms.store(0, Ordering::Relaxed);
        self.duration_ms.store(0, Ordering::Relaxed);
        self.is_playing.store(false, Ordering::Relaxed);
        self.last_update_timestamp.store(Self::current_timestamp_ms(self.start_time), Ordering::Relaxed);
        *self.current_song.write().await = None;
        
        // 发送重置事件
        let _ = self.event_sender.send(PlaybackEvent::Reset);
    }

    /// 获取事件接收器（用于监听播放事件）
    pub fn subscribe(&self) -> watch::Receiver<PlaybackEvent> {
        self.event_sender.subscribe()
    }
}
