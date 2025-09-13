use std::time::{Duration, Instant};
use tokio::sync::watch;
use serde::{Deserialize, Serialize};

use windows::{
    core::*,
    Media::Control::*,
};

/// 媒体播放状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlaybackStatus {
    Unknown,
    Playing,
    Paused,
    Stopped,
}

impl From<GlobalSystemMediaTransportControlsSessionPlaybackStatus> for PlaybackStatus {
    fn from(status: GlobalSystemMediaTransportControlsSessionPlaybackStatus) -> Self {
        match status {
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing => PlaybackStatus::Playing,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused => PlaybackStatus::Paused,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped => PlaybackStatus::Stopped,
            _ => PlaybackStatus::Unknown,
        }
    }
}

impl Default for PlaybackStatus {
    fn default() -> Self {
        PlaybackStatus::Unknown
    }
}

/// 媒体信息
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MediaInfo {
    pub app_name: String,
    pub title: String,
    pub artist: String,
    pub duration: Option<Duration>,
    pub position: Option<Duration>,
    pub playback_status: PlaybackStatus,
    #[serde(skip)]
    pub last_updated: Option<Instant>,
}

/// 媒体事件
#[derive(Debug, Clone)]
pub enum MediaEvent {
    /// 媒体信息更新
    InfoUpdated(MediaInfo),
    /// 媒体监控错误
    Error(String),
    /// 媒体监控停止
    Stopped,
}

/// 优化的媒体监测器
/// 使用事件驱动架构，提供实时媒体信息更新
pub struct MediaMonitor {
    session_manager: Option<GlobalSystemMediaTransportControlsSessionManager>,
    event_sender: watch::Sender<MediaEvent>,
    is_running: bool,
}

impl MediaMonitor {
    /// 创建新的媒体监测器
    pub fn new() -> (Self, watch::Receiver<MediaEvent>) {
        let (event_sender, event_receiver) = watch::channel(MediaEvent::Stopped);
        
        let monitor = Self {
            session_manager: None,
            event_sender,
            is_running: false,
        };
        
        (monitor, event_receiver)
    }

    /// 异步初始化媒体监测器
    pub async fn initialize(&mut self) -> Result<()> {
        match GlobalSystemMediaTransportControlsSessionManager::RequestAsync() {
            Ok(async_op) => {
                match async_op.await {
                    Ok(manager) => {
                        self.session_manager = Some(manager);
                        Ok(())
                    }
                    Err(e) => {
                        let error_msg = format!("获取媒体会话管理器失败: {:?}", e);
                        let _ = self.event_sender.send(MediaEvent::Error(error_msg));
                        Err(e)
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("请求媒体会话管理器失败: {:?}", e);
                let _ = self.event_sender.send(MediaEvent::Error(error_msg));
                Err(e)
            }
        }
    }

    /// 开始媒体信息同步循环
    pub async fn start_monitoring(&mut self, interval: Duration) -> Result<()> {
        if self.session_manager.is_none() {
            let error_msg = "媒体监测器未初始化".to_string();
            let _ = self.event_sender.send(MediaEvent::Error(error_msg.clone()));
            return Err(Error::from_hresult(HRESULT(-1)));
        }

        self.is_running = true;

        while self.is_running {
            match self.get_current_media_info().await {
                Some(media_info) => {
                    // 发送媒体信息更新事件
                    let _ = self.event_sender.send(MediaEvent::InfoUpdated(media_info));
                }
                None => {
                    // 发送空媒体信息
                    let _ = self.event_sender.send(MediaEvent::InfoUpdated(MediaInfo::default()));
                }
            }

            tokio::time::sleep(interval).await;
        }

        let _ = self.event_sender.send(MediaEvent::Stopped);
        Ok(())
    }

    /// 停止监控循环
    pub fn stop(&mut self) {
        self.is_running = false;
        let _ = self.event_sender.send(MediaEvent::Stopped);
    }

    /// 检查是否已初始化
    pub fn is_initialized(&self) -> bool {
        self.session_manager.is_some()
    }

    /// 异步获取当前媒体信息
    pub async fn get_current_media_info(&self) -> Option<MediaInfo> {
        if !self.is_initialized() {
            return None;
        }

        let manager = self.session_manager.as_ref()?;

        // 获取当前会话
        let session = match manager.GetCurrentSession() {
            Ok(session) => session,
            Err(_) => return None,
        };

        // 获取媒体属性
        let session_properties = match session.TryGetMediaPropertiesAsync() {
            Ok(props_async) => {
                match props_async.await {
                    Ok(props) => props,
                    Err(_) => return None,
                }
            }
            Err(_) => return None,
        };

        // 获取基本信息
        let title = session_properties.Title().ok()?.to_string();
        let artist = session_properties.Artist().ok()?.to_string();

        // 检查歌曲信息是否有效
        if title.trim().is_empty() || artist.trim().is_empty() {
            return None;
        }

        // 获取播放状态和时间信息
        let playback_info = session.GetPlaybackInfo().ok()?;
        let timeline_props = session.GetTimelineProperties().ok()?;

        let playback_status: PlaybackStatus = playback_info.PlaybackStatus().ok()?.into();
        
        let end_time = timeline_props.EndTime().ok()?;
        let position = timeline_props.Position().ok()?;
        
        let duration = Duration::from_nanos(end_time.Duration as u64 * 100);
        let current_position = Duration::from_nanos(position.Duration as u64 * 100);

        Some(MediaInfo {
            app_name: String::new(),
            title: title.trim().to_string(), // 去除首尾空格
            artist: artist.trim().to_string(), // 去除首尾空格
            duration: Some(duration),
            position: Some(current_position),
            playback_status,
            last_updated: Some(Instant::now()),
        })
    }

    /// 订阅媒体事件
    pub fn subscribe(&self) -> watch::Receiver<MediaEvent> {
        self.event_sender.subscribe()
    }
}
