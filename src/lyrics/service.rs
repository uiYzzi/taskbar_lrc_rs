use tracing::{debug, info, warn, error};

use crate::lyrics::{
    LyricsResult, LyricsError, LyricsData, LyricsSource, SongInfo,
    http_client::{HttpClient, HttpClientConfig},
    api::{NetEaseApi, QQMusicApi},
    cache::{LyricsCache, CacheConfig, CacheStats},
};

/// 歌词服务配置
#[derive(Debug, Clone)]
pub struct LyricsServiceConfig {
    /// HTTP客户端配置
    pub http_config: HttpClientConfig,
    /// 缓存配置
    pub cache_config: CacheConfig,
    /// 是否启用网易云音乐
    pub enable_netease: bool,
    /// 是否启用QQ音乐
    pub enable_qqmusic: bool,
    /// 搜索超时时间（秒）
    pub search_timeout_secs: u64,
}

impl Default for LyricsServiceConfig {
    fn default() -> Self {
        Self {
            http_config: HttpClientConfig::default(),
            cache_config: CacheConfig::default(),
            enable_netease: true,
            enable_qqmusic: true,
            search_timeout_secs: 30,
        }
    }
}

/// 歌词服务
pub struct LyricsService {
    config: LyricsServiceConfig,
    netease_api: Option<NetEaseApi>,
    qqmusic_api: Option<QQMusicApi>,
    cache: LyricsCache,
}

impl LyricsService {
    /// 创建新的歌词服务
    pub fn new(config: LyricsServiceConfig) -> LyricsResult<Self> {
        // 创建HTTP客户端
        let http_client = HttpClient::new(config.http_config.clone())?;
        
        // 创建API实例
        let netease_api = if config.enable_netease {
            Some(NetEaseApi::new(http_client.clone()))
        } else {
            None
        };
        
        let qqmusic_api = if config.enable_qqmusic {
            Some(QQMusicApi::new(http_client))
        } else {
            None
        };
        
        // 创建缓存
        let cache = LyricsCache::new(config.cache_config.clone())?;
        
        info!("歌词服务初始化完成 - 网易云: {}, QQ音乐: {}", 
              config.enable_netease, config.enable_qqmusic);
        
        Ok(Self {
            config,
            netease_api,
            qqmusic_api,
            cache,
        })
    }

    /// 创建默认歌词服务
    pub fn default() -> LyricsResult<Self> {
        Self::new(LyricsServiceConfig::default())
    }

    /// 搜索并获取歌词
    pub async fn search_and_get_lyrics(&self, song_info: &SongInfo) -> LyricsResult<LyricsData> {
        if !song_info.is_valid() {
            return Err(LyricsError::InvalidSongInfo);
        }

        info!("开始搜索歌词: {}", song_info);

        // 1. 首先检查缓存
        if let Some(cached_lyrics) = self.cache.get(song_info).await {
            info!("从缓存获取歌词: {}", song_info);
            return Ok(cached_lyrics);
        }

        // 2. 从API获取歌词
        let lyrics_data = self.fetch_lyrics_from_apis(song_info).await?;

        // 3. 存储到缓存
        if let Err(e) = self.cache.put(song_info.clone(), lyrics_data.clone()).await {
            warn!("缓存歌词失败: {}", e);
        }

        info!("成功获取歌词: {} (来源: {:?})", song_info, lyrics_data.source);
        Ok(lyrics_data)
    }

    /// 从API获取歌词
    async fn fetch_lyrics_from_apis(&self, song_info: &SongInfo) -> LyricsResult<LyricsData> {
        let mut last_error = None;

        // 尝试网易云音乐
        if let Some(netease_api) = &self.netease_api {
            debug!("尝试从网易云音乐获取歌词");
            
            match tokio::time::timeout(
                std::time::Duration::from_secs(self.config.search_timeout_secs),
                netease_api.search_and_get_lyrics(song_info)
            ).await {
                Ok(Ok(lyrics_data)) => {
                    if lyrics_data.has_any_content() {
                        info!("从网易云音乐成功获取歌词");
                        return Ok(lyrics_data);
                    }
                }
                Ok(Err(e)) => {
                    warn!("网易云音乐获取歌词失败: {}", e);
                    last_error = Some(e);
                }
                Err(_) => {
                    warn!("网易云音乐请求超时");
                    last_error = Some(LyricsError::Timeout);
                }
            }
        }

        // 尝试QQ音乐
        if let Some(qqmusic_api) = &self.qqmusic_api {
            debug!("尝试从QQ音乐获取歌词");
            
            match tokio::time::timeout(
                std::time::Duration::from_secs(self.config.search_timeout_secs),
                qqmusic_api.search_and_get_lyrics(song_info)
            ).await {
                Ok(Ok(lyrics_data)) => {
                    if lyrics_data.has_any_content() {
                        info!("从QQ音乐成功获取歌词");
                        return Ok(lyrics_data);
                    }
                }
                Ok(Err(e)) => {
                    warn!("QQ音乐获取歌词失败: {}", e);
                    last_error = Some(e);
                }
                Err(_) => {
                    warn!("QQ音乐请求超时");
                    last_error = Some(LyricsError::Timeout);
                }
            }
        }

        // 如果所有来源都失败
        error!("所有API都无法获取歌词: {}", song_info);
        Err(last_error.unwrap_or(LyricsError::LyricsNotFound))
    }

    /// 预加载歌词（异步）
    pub async fn preload_lyrics(&self, song_info: &SongInfo) {
        if !song_info.is_valid() {
            return;
        }

        // 检查是否已缓存
        if self.cache.get(song_info).await.is_some() {
            return;
        }

        debug!("预加载歌词: {}", song_info);

        // 直接获取歌词，不启动新任务
        let song_info_clone = song_info.clone();
        if let Err(e) = self.search_and_get_lyrics(&song_info_clone).await {
            debug!("预加载歌词失败: {} - {}", song_info_clone, e);
        }
    }

    /// 批量预加载歌词
    pub async fn preload_batch(&self, songs: Vec<SongInfo>) {
        info!("开始批量预加载 {} 首歌曲的歌词", songs.len());

        for song in songs.into_iter().filter(|song| song.is_valid()) {
            if self.cache.get(&song).await.is_none() {
                if let Err(e) = self.search_and_get_lyrics(&song).await {
                    debug!("批量预加载歌词失败: {} - {}", song, e);
                }
            }
        }

        info!("批量预加载完成");
    }

    /// 清理过期缓存
    pub async fn cleanup_cache(&self) -> LyricsResult<()> {
        info!("开始清理过期缓存");
        self.cache.cleanup_expired().await?;
        info!("缓存清理完成");
        Ok(())
    }

    /// 清空所有缓存
    pub async fn clear_cache(&self) -> LyricsResult<()> {
        info!("清空所有缓存");
        self.cache.clear().await?;
        info!("缓存已清空");
        Ok(())
    }

    /// 获取缓存统计信息
    pub async fn get_cache_stats(&self) -> CacheStats {
        self.cache.get_stats().await
    }

    /// 检查歌词是否可用（仅检查缓存）
    pub async fn is_lyrics_cached(&self, song_info: &SongInfo) -> bool {
        self.cache.get(song_info).await.is_some()
    }

    /// 获取支持的歌词源
    pub fn get_supported_sources(&self) -> Vec<LyricsSource> {
        let mut sources = Vec::new();
        
        if self.netease_api.is_some() {
            sources.push(LyricsSource::NetEase);
        }
        
        if self.qqmusic_api.is_some() {
            sources.push(LyricsSource::QQMusic);
        }
        
        sources
    }

    /// 测试服务连通性
    pub async fn test_connectivity(&self) -> Vec<(LyricsSource, bool)> {
        let mut results = Vec::new();
        
        // 测试歌曲
        let test_song = SongInfo::new("测试", "测试");
        
        // 测试网易云
        if let Some(netease_api) = &self.netease_api {
            let result = netease_api.search_song(&test_song).await.is_ok();
            results.push((LyricsSource::NetEase, result));
        }
        
        // 测试QQ音乐
        if let Some(qqmusic_api) = &self.qqmusic_api {
            let result = qqmusic_api.search_song(&test_song).await.is_ok();
            results.push((LyricsSource::QQMusic, result));
        }
        
        results
    }
}

/// 歌词服务构建器
pub struct LyricsServiceBuilder {
    config: LyricsServiceConfig,
}

impl LyricsServiceBuilder {
    pub fn new() -> Self {
        Self {
            config: LyricsServiceConfig::default(),
        }
    }

    pub fn with_http_config(mut self, config: HttpClientConfig) -> Self {
        self.config.http_config = config;
        self
    }

    pub fn with_cache_config(mut self, config: CacheConfig) -> Self {
        self.config.cache_config = config;
        self
    }

    pub fn enable_netease(mut self, enable: bool) -> Self {
        self.config.enable_netease = enable;
        self
    }

    pub fn enable_qqmusic(mut self, enable: bool) -> Self {
        self.config.enable_qqmusic = enable;
        self
    }

    pub fn with_search_timeout(mut self, timeout_secs: u64) -> Self {
        self.config.search_timeout_secs = timeout_secs;
        self
    }

    pub fn build(self) -> LyricsResult<LyricsService> {
        LyricsService::new(self.config)
    }
}

impl Default for LyricsServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lyrics_service_creation() {
        let service = LyricsService::default().unwrap();
        
        let sources = service.get_supported_sources();
        assert!(sources.contains(&LyricsSource::NetEase));
        assert!(sources.contains(&LyricsSource::QQMusic));
    }

    #[tokio::test]
    async fn test_lyrics_service_builder() {
        let service = LyricsServiceBuilder::new()
            .enable_netease(true)
            .enable_qqmusic(false)
            .with_search_timeout(10)
            .build()
            .unwrap();
        
        let sources = service.get_supported_sources();
        assert!(sources.contains(&LyricsSource::NetEase));
        assert!(!sources.contains(&LyricsSource::QQMusic));
    }

    #[test]
    fn test_song_info_validation() {
        let valid_song = SongInfo::new("Valid Title", "Valid Artist");
        assert!(valid_song.is_valid());
        
        let invalid_song = SongInfo::new("", "");
        assert!(!invalid_song.is_valid());
    }
}
