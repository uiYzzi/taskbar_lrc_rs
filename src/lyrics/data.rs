use serde::{Deserialize, Serialize};
use std::time::Duration;
use chrono::{DateTime, Utc};

/// 歌词数据结构
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LyricsData {
    /// 原文歌词
    pub original: Option<String>,
    /// 翻译歌词
    pub translated: Option<String>,
    /// 罗马音歌词
    pub romanized: Option<String>,
    /// 是否有歌词
    pub has_lyrics: bool,
    /// 歌词来源
    pub source: LyricsSource,
    /// 获取时间
    pub fetched_at: DateTime<Utc>,
}

/// 歌词来源
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LyricsSource {
    NetEase,
    QQMusic,
    Unknown,
}

impl Default for LyricsSource {
    fn default() -> Self {
        LyricsSource::Unknown
    }
}

/// 歌曲信息
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SongInfo {
    pub title: String,
    pub artist: String,
}

impl SongInfo {
    pub fn new(title: impl Into<String>, artist: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            artist: artist.into(),
        }
    }

    /// 生成缓存键
    pub fn cache_key(&self) -> String {
        use sha2::{Digest, Sha256};
        
        let combined = format!("{}|{}", self.title.trim().to_lowercase(), self.artist.trim().to_lowercase());
        let mut hasher = Sha256::new();
        hasher.update(combined.as_bytes());
        let result = hasher.finalize();
        format!("{:x}", result)
    }

    /// 检查歌曲信息是否有效
    pub fn is_valid(&self) -> bool {
        !self.title.trim().is_empty() && !self.artist.trim().is_empty()
    }
}

/// 搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub duration: Option<Duration>,
}

/// QQ音乐搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QQSearchResult {
    pub song_id: String,
    pub song_mid: String,
    pub title: String,
    pub artist: String,
}

/// 网易云音乐API响应
#[derive(Debug, Deserialize)]
pub struct NetEaseSearchResponse {
    pub result: Option<NetEaseSearchResult>,
}

#[derive(Debug, Deserialize)]
pub struct NetEaseSearchResult {
    pub songs: Option<Vec<NetEaseSong>>,
}

#[derive(Debug, Deserialize)]
pub struct NetEaseSong {
    pub id: u64,
    pub name: String,
    pub ar: Vec<NetEaseArtist>,
    pub dt: Option<u64>, // 时长（毫秒）
}

#[derive(Debug, Deserialize)]
pub struct NetEaseArtist {
    pub name: String,
}

/// QQ音乐API响应
#[derive(Debug, Deserialize)]
pub struct QQSearchResponse {
    pub data: Option<QQSearchData>,
}

#[derive(Debug, Deserialize)]
pub struct QQSearchData {
    pub song: Option<QQSongData>,
}

#[derive(Debug, Deserialize)]
pub struct QQSongData {
    pub list: Option<Vec<QQSong>>,
}

#[derive(Debug, Deserialize)]
pub struct QQSong {
    pub songid: u64,
    pub songmid: String,
    pub songname: String,
    pub singer: Vec<QQSinger>,
    pub interval: Option<u64>, // 时长（秒）
}

#[derive(Debug, Deserialize)]
pub struct QQSinger {
    pub name: String,
}

/// 歌词API响应
#[derive(Debug, Deserialize)]
pub struct LyricsApiResponse {
    pub code: Option<String>,
    pub lrc: Option<String>,
    pub trans: Option<String>,
    pub roma: Option<String>,
}

/// 网易云歌词API响应
#[derive(Debug, Deserialize)]
pub struct NetEaseLyricsResponse {
    pub code: i32,
    pub message: String,
    pub data: Option<NetEaseLyricsData>,
    pub time: String,
    pub tips: String,
}

#[derive(Debug, Deserialize)]
pub struct NetEaseLyricsData {
    pub lrc: Option<String>,
    pub yrc: Option<String>,
}

/// QQ音乐歌词API响应
#[derive(Debug, Deserialize)]
pub struct QQMusicLyricsResponse {
    pub code: i32,
    pub message: String,
    pub data: Option<QQMusicLyricsData>,
    pub time: String,
    pub pid: Option<i32>,
    pub tips: String,
}

#[derive(Debug, Deserialize)]
pub struct QQMusicLyricsData {
    pub lrc: Option<String>,
    pub trans: Option<String>,
    pub yrc: Option<String>,
    pub roma: Option<String>,
}

impl LyricsData {
    /// 清空歌词数据
    pub fn clear(&mut self) {
        self.original = None;
        self.translated = None;
        self.romanized = None;
        self.has_lyrics = false;
        self.source = LyricsSource::Unknown;
    }

    /// 检查是否有任何歌词内容
    pub fn has_any_content(&self) -> bool {
        self.original.as_ref().map_or(false, |s| !s.trim().is_empty()) ||
        self.translated.as_ref().map_or(false, |s| !s.trim().is_empty()) ||
        self.romanized.as_ref().map_or(false, |s| !s.trim().is_empty())
    }

    /// 处理歌词字符串中的转义字符
    pub fn process_lyrics_string(lyrics: &str) -> String {
        lyrics
            .replace("\\n", "\n")
            .replace("\\r", "\r")
            .replace("\\t", "\t")
    }

    /// 从API响应创建歌词数据
    pub fn from_api_response(response: LyricsApiResponse, source: LyricsSource) -> Self {
        let mut data = Self {
            source,
            fetched_at: Utc::now(),
            ..Default::default()
        };

        if let Some(lrc) = response.lrc {
            if !lrc.trim().is_empty() {
                data.original = Some(Self::process_lyrics_string(&lrc));
                data.has_lyrics = true;
            }
        }

        if let Some(trans) = response.trans {
            if !trans.trim().is_empty() {
                data.translated = Some(Self::process_lyrics_string(&trans));
            }
        }

        if let Some(roma) = response.roma {
            if !roma.trim().is_empty() {
                data.romanized = Some(Self::process_lyrics_string(&roma));
            }
        }

        data
    }

    /// 从网易云API响应创建歌词数据
    pub fn from_netease_response(response: NetEaseLyricsResponse) -> Self {
        let mut data = Self {
            source: LyricsSource::NetEase,
            fetched_at: Utc::now(),
            ..Default::default()
        };

        if response.code == 200 {
            if let Some(lyrics_data) = response.data {
                if let Some(lrc) = lyrics_data.lrc {
                    if !lrc.trim().is_empty() {
                        data.original = Some(Self::process_lyrics_string(&lrc));
                        data.has_lyrics = true;
                    }
                }
                // 网易云的yrc是逐字歌词，我们暂时不处理
            }
        }

        data
    }

    /// 从QQ音乐API响应创建歌词数据
    pub fn from_qqmusic_response(response: QQMusicLyricsResponse) -> Self {
        let mut data = Self {
            source: LyricsSource::QQMusic,
            fetched_at: Utc::now(),
            ..Default::default()
        };

        if response.code == 200 {
            if let Some(lyrics_data) = response.data {
                if let Some(lrc) = lyrics_data.lrc {
                    if !lrc.trim().is_empty() {
                        data.original = Some(Self::process_lyrics_string(&lrc));
                        data.has_lyrics = true;
                    }
                }

                if let Some(trans) = lyrics_data.trans {
                    if !trans.trim().is_empty() {
                        data.translated = Some(Self::process_lyrics_string(&trans));
                    }
                }

                if let Some(roma) = lyrics_data.roma {
                    if !roma.trim().is_empty() {
                        data.romanized = Some(Self::process_lyrics_string(&roma));
                    }
                }
            }
        }

        data
    }

    /// 根据当前播放时间获取对应的歌词行（静态方法）
    pub fn get_current_lyrics_line(lyrics_data: &LyricsData, current_position: Duration) -> Option<String> {
        // 优先使用原文歌词
        let lyrics_text = if let Some(original) = &lyrics_data.original {
            original
        } else if let Some(translated) = &lyrics_data.translated {
            translated
        } else {
            return None;
        };
        
        Self::parse_lrc_for_time(lyrics_text, current_position)
    }

    /// 解析LRC歌词，根据时间获取当前应显示的歌词行（静态方法）
    pub fn parse_lrc_for_time(lyrics: &str, current_position: Duration) -> Option<String> {
        let current_ms = current_position.as_millis() as u64;
        let mut lyrics_lines = Vec::new();
        
        // 解析所有歌词行
        for line in lyrics.lines() {
            let line = line.trim();
            if !line.is_empty() && line.starts_with('[') {
                if let Some(close_bracket) = line.find(']') {
                    let time_part = &line[1..close_bracket];
                    let lyrics_content = &line[close_bracket + 1..].trim();
                    
                    // 解析时间戳 [mm:ss.xx]
                    if let Some(timestamp_ms) = Self::parse_lrc_timestamp(time_part) {
                        lyrics_lines.push((timestamp_ms, lyrics_content.to_string()));
                    }
                }
            }
        }
        
        // 按时间排序
        lyrics_lines.sort_by_key(|&(time, _)| time);
        
        // 找到当前时间对应的歌词
        let mut current_lyrics = None;
        for (timestamp, lyrics_text) in lyrics_lines {
            if timestamp <= current_ms {
                current_lyrics = Some(lyrics_text);
            } else {
                break;
            }
        }
        
        current_lyrics.filter(|s| !s.is_empty())
    }

    /// 解析LRC时间戳格式 [mm:ss.xx] 返回毫秒（静态方法）
    pub fn parse_lrc_timestamp(time_str: &str) -> Option<u64> {
        // 格式: mm:ss.xx
        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() != 2 {
            return None;
        }
        
        let minutes: u64 = parts[0].parse().ok()?;
        let seconds_parts: Vec<&str> = parts[1].split('.').collect();
        if seconds_parts.len() != 2 {
            return None;
        }
        
        let seconds: u64 = seconds_parts[0].parse().ok()?;
        let centiseconds: u64 = seconds_parts[1].parse().ok()?;
        
        Some(minutes * 60 * 1000 + seconds * 1000 + centiseconds * 10)
    }
}

impl std::fmt::Display for SongInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} - {}", self.artist, self.title)
    }
}
