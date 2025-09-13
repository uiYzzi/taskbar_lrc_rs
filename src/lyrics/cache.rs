use std::path::PathBuf;
use std::fs;
use chrono::{DateTime, Utc, Duration as ChronoDuration};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn, error};

use crate::lyrics::{LyricsResult, LyricsError, LyricsData, SongInfo};

/// 缓存条目
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    song_info: SongInfo,
    lyrics_data: LyricsData,
    expires_at: DateTime<Utc>,
}

impl CacheEntry {
    fn new(song_info: SongInfo, lyrics_data: LyricsData, ttl: ChronoDuration) -> Self {
        Self {
            song_info,
            lyrics_data,
            expires_at: Utc::now() + ttl,
        }
    }

    fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}

/// 缓存配置
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// 缓存生存时间
    pub ttl: ChronoDuration,
    /// 磁盘缓存目录
    pub cache_dir: PathBuf,
    /// 磁盘缓存最大文件数
    pub max_files: usize,
    /// 清理过期文件的间隔（小时）
    pub cleanup_interval_hours: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        let cache_dir = Self::get_default_cache_dir()
            .unwrap_or_else(|| PathBuf::from("cache/lyrics"));
            
        Self {
            ttl: ChronoDuration::hours(24), // 24小时
            cache_dir,
            max_files: 5000,
            cleanup_interval_hours: 6, // 每6小时清理一次
        }
    }
}

impl CacheConfig {
    /// 获取默认缓存目录
    fn get_default_cache_dir() -> Option<PathBuf> {
        dirs::cache_dir().map(|mut path| {
            path.push("taskbar_lrc");
            path.push("lyrics");
            path
        })
    }
}

/// 歌词缓存（仅文件缓存）
pub struct LyricsCache {
    config: CacheConfig,
    last_cleanup: std::sync::Mutex<Option<DateTime<Utc>>>,
}

impl LyricsCache {
    /// 创建新的歌词缓存
    pub fn new(config: CacheConfig) -> LyricsResult<Self> {
        // 创建缓存目录
        fs::create_dir_all(&config.cache_dir)
            .map_err(|e| LyricsError::CacheError(format!("创建缓存目录失败: {}", e)))?;

        debug!("歌词缓存目录: {:?}", config.cache_dir);

        Ok(Self {
            config,
            last_cleanup: std::sync::Mutex::new(None),
        })
    }

    /// 创建默认缓存
    pub fn default() -> LyricsResult<Self> {
        Self::new(CacheConfig::default())
    }

    /// 获取歌词
    pub async fn get(&self, song_info: &SongInfo) -> Option<LyricsData> {
        let cache_key = song_info.cache_key();
        
        // 检查是否需要清理
        self.maybe_cleanup().await;
        
        match self.get_from_disk(&cache_key).await {
            Ok(Some(entry)) => {
                if !entry.is_expired() {
                    debug!("从缓存获取歌词: {}", song_info);
                    Some(entry.lyrics_data)
                } else {
                    // 过期则删除文件
                    debug!("缓存过期，删除文件: {}", song_info);
                    let _ = self.remove_from_disk(&cache_key).await;
                    None
                }
            }
            Ok(None) => None,
            Err(e) => {
                warn!("读取缓存失败: {}", e);
                None
            }
        }
    }

    /// 存储歌词
    pub async fn put(&self, song_info: SongInfo, lyrics_data: LyricsData) -> LyricsResult<()> {
        let cache_key = song_info.cache_key();
        let entry = CacheEntry::new(song_info.clone(), lyrics_data, self.config.ttl);

        self.put_to_disk(&cache_key, &entry).await?;
        debug!("缓存歌词: {}", song_info);

        // 检查是否需要清理缓存
        self.cleanup_if_needed().await?;

        Ok(())
    }

    /// 清理过期缓存
    pub async fn cleanup_expired(&self) -> LyricsResult<()> {
        debug!("开始清理过期缓存");
        
        if !self.config.cache_dir.exists() {
            return Ok(());
        }

        let entries = fs::read_dir(&self.config.cache_dir)
            .map_err(|e| LyricsError::CacheError(format!("读取缓存目录失败: {}", e)))?;

        let mut cleaned_count = 0;
        
        for entry in entries {
            let entry = entry.map_err(|e| LyricsError::CacheError(format!("读取目录条目失败: {}", e)))?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                match fs::read_to_string(&path) {
                    Ok(content) => {
                        match serde_json::from_str::<CacheEntry>(&content) {
                            Ok(cache_entry) => {
                                if cache_entry.is_expired() {
                                    if let Err(e) = fs::remove_file(&path) {
                                        warn!("删除过期缓存文件失败: {} - {}", path.display(), e);
                                    } else {
                                        cleaned_count += 1;
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("解析缓存文件失败，删除: {} - {}", path.display(), e);
                                let _ = fs::remove_file(&path);
                                cleaned_count += 1;
                            }
                        }
                    }
                    Err(e) => {
                        warn!("读取缓存文件失败，删除: {} - {}", path.display(), e);
                        let _ = fs::remove_file(&path);
                        cleaned_count += 1;
                    }
                }
            }
        }

        debug!("清理过期缓存完成，删除 {} 个文件", cleaned_count);
        
        // 更新最后清理时间
        if let Ok(mut last_cleanup) = self.last_cleanup.lock() {
            *last_cleanup = Some(Utc::now());
        }

        Ok(())
    }

    /// 清空所有缓存
    pub async fn clear(&self) -> LyricsResult<()> {
        debug!("清空所有缓存");
        
        if self.config.cache_dir.exists() {
            fs::remove_dir_all(&self.config.cache_dir)
                .map_err(|e| LyricsError::CacheError(format!("清空缓存目录失败: {}", e)))?;
            fs::create_dir_all(&self.config.cache_dir)
                .map_err(|e| LyricsError::CacheError(format!("重建缓存目录失败: {}", e)))?;
        }

        // 更新最后清理时间
        if let Ok(mut last_cleanup) = self.last_cleanup.lock() {
            *last_cleanup = Some(Utc::now());
        }

        debug!("缓存已清空");
        Ok(())
    }

    /// 从磁盘获取
    async fn get_from_disk(&self, cache_key: &str) -> LyricsResult<Option<CacheEntry>> {
        let file_path = self.config.cache_dir.join(format!("{}.json", cache_key));
        
        if !file_path.exists() {
            return Ok(None);
        }
        
        let content = fs::read_to_string(&file_path)
            .map_err(|e| LyricsError::CacheError(format!("读取缓存文件失败: {}", e)))?;
        
        let entry: CacheEntry = serde_json::from_str(&content)
            .map_err(|e| LyricsError::CacheError(format!("解析缓存文件失败: {}", e)))?;
        
        Ok(Some(entry))
    }

    /// 存储到磁盘
    async fn put_to_disk(&self, cache_key: &str, entry: &CacheEntry) -> LyricsResult<()> {
        let file_path = self.config.cache_dir.join(format!("{}.json", cache_key));
        
        let content = serde_json::to_string_pretty(entry)
            .map_err(|e| LyricsError::CacheError(format!("序列化缓存条目失败: {}", e)))?;
        
        fs::write(&file_path, content)
            .map_err(|e| LyricsError::CacheError(format!("写入缓存文件失败: {}", e)))?;
        
        Ok(())
    }

    /// 从磁盘删除
    async fn remove_from_disk(&self, cache_key: &str) -> LyricsResult<()> {
        let file_path = self.config.cache_dir.join(format!("{}.json", cache_key));
        
        if file_path.exists() {
            fs::remove_file(&file_path)
                .map_err(|e| LyricsError::CacheError(format!("删除缓存文件失败: {}", e)))?;
        }
        
        Ok(())
    }

    /// 如果需要则清理缓存
    async fn cleanup_if_needed(&self) -> LyricsResult<()> {
        // 检查文件数量
        let file_count = self.count_cache_files().await?;
        
        if file_count > self.config.max_files {
            debug!("缓存文件数量 ({}) 超过限制 ({}), 开始清理", file_count, self.config.max_files);
            self.cleanup_oldest_files().await?;
        }

        Ok(())
    }

    /// 清理最旧的文件
    async fn cleanup_oldest_files(&self) -> LyricsResult<()> {
        if !self.config.cache_dir.exists() {
            return Ok(());
        }

        let mut file_infos = Vec::new();
        
        let entries = fs::read_dir(&self.config.cache_dir)
            .map_err(|e| LyricsError::CacheError(format!("读取缓存目录失败: {}", e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| LyricsError::CacheError(format!("读取目录条目失败: {}", e)))?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        file_infos.push((path, modified));
                    }
                }
            }
        }

        // 按修改时间排序（最旧的在前）
        file_infos.sort_by_key(|(_, modified)| *modified);

        // 删除最旧的文件，保留75%
        let target_count = self.config.max_files * 3 / 4;
        let files_to_remove = file_infos.len().saturating_sub(target_count);
        
        let mut removed_count = 0;
        for (path, _) in file_infos.iter().take(files_to_remove) {
            if let Err(e) = fs::remove_file(path) {
                warn!("删除旧缓存文件失败: {} - {}", path.display(), e);
            } else {
                removed_count += 1;
            }
        }

        debug!("清理旧缓存文件完成，删除 {} 个文件", removed_count);
        Ok(())
    }

    /// 检查是否需要定期清理
    async fn maybe_cleanup(&self) {
        let should_cleanup = {
            if let Ok(last_cleanup) = self.last_cleanup.lock() {
                match *last_cleanup {
                    Some(last) => {
                        let elapsed = Utc::now().signed_duration_since(last);
                        elapsed.num_hours() >= self.config.cleanup_interval_hours as i64
                    }
                    None => true, // 从未清理过
                }
            } else {
                false
            }
        };

        if should_cleanup {
            if let Err(e) = self.cleanup_expired().await {
                error!("定期清理缓存失败: {}", e);
            }
        }
    }

    /// 统计缓存文件数量
    async fn count_cache_files(&self) -> LyricsResult<usize> {
        if !self.config.cache_dir.exists() {
            return Ok(0);
        }

        let entries = fs::read_dir(&self.config.cache_dir)
            .map_err(|e| LyricsError::CacheError(format!("读取缓存目录失败: {}", e)))?;
        
        let count = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.path().is_file() && 
                entry.path().extension().map_or(false, |ext| ext == "json")
            })
            .count();
        
        Ok(count)
    }

    /// 获取缓存统计信息
    pub async fn get_stats(&self) -> CacheStats {
        let file_count = self.count_cache_files().await.unwrap_or(0);
        
        CacheStats {
            file_count,
            max_files: self.config.max_files,
            cache_dir: self.config.cache_dir.clone(),
            ttl_hours: self.config.ttl.num_hours(),
        }
    }
}

/// 缓存统计信息
#[derive(Debug)]
pub struct CacheStats {
    pub file_count: usize,
    pub max_files: usize,
    pub cache_dir: PathBuf,
    pub ttl_hours: i64,
}

impl std::fmt::Display for CacheStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, 
            "缓存统计: {}/{} 个文件, 目录: {:?}, TTL: {}小时",
            self.file_count, self.max_files, self.cache_dir, self.ttl_hours
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_cache_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let config = CacheConfig {
            cache_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        
        let cache = LyricsCache::new(config).unwrap();
        let song_info = SongInfo::new("测试歌曲", "测试歌手");
        let lyrics_data = LyricsData::default();
        
        // 测试存储和获取
        cache.put(song_info.clone(), lyrics_data.clone()).await.unwrap();
        let retrieved = cache.get(&song_info).await;
        
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_cache_entry_expiration() {
        let song_info = SongInfo::new("测试", "歌手");
        let lyrics_data = LyricsData::default();
        let ttl = ChronoDuration::milliseconds(-1); // 立即过期
        
        let entry = CacheEntry::new(song_info, lyrics_data, ttl);
        assert!(entry.is_expired());
    }

    #[tokio::test]
    async fn test_cache_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let config = CacheConfig {
            cache_dir: temp_dir.path().to_path_buf(),
            max_files: 2,
            ..Default::default()
        };
        
        let cache = LyricsCache::new(config).unwrap();
        
        // 添加多个缓存条目
        for i in 0..5 {
            let song_info = SongInfo::new(format!("歌曲{}", i), "歌手");
            let lyrics_data = LyricsData::default();
            cache.put(song_info, lyrics_data).await.unwrap();
        }
        
        let stats = cache.get_stats().await;
        assert!(stats.file_count <= stats.max_files);
    }
}