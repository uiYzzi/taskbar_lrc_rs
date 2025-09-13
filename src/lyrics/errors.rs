use thiserror::Error;

/// 歌词服务错误类型
#[derive(Error, Debug)]
pub enum LyricsError {
    #[error("网络请求失败: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("JSON解析失败: {0}")]
    JsonParseError(#[from] serde_json::Error),

    #[error("URL解析失败: {0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("缓存操作失败: {0}")]
    CacheError(String),

    #[error("API返回错误: {code}, 消息: {message}")]
    ApiError { code: String, message: String },

    #[error("歌曲信息无效")]
    InvalidSongInfo,

    #[error("未找到歌曲")]
    SongNotFound,

    #[error("未找到歌词")]
    LyricsNotFound,

    #[error("请求超时")]
    Timeout,

    #[error("请求次数过多，已达到限制")]
    RateLimited,

    #[error("服务不可用")]
    ServiceUnavailable,

    #[error("内部错误: {0}")]
    InternalError(String),
}

impl LyricsError {
    /// 检查错误是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(self, 
            LyricsError::NetworkError(_) |
            LyricsError::Timeout |
            LyricsError::ServiceUnavailable
        )
    }

    /// 获取重试延迟（毫秒）
    pub fn retry_delay_ms(&self, attempt: u32) -> u64 {
        match self {
            LyricsError::RateLimited => 60000, // 1分钟
            LyricsError::ServiceUnavailable => 30000, // 30秒
            _ => {
                // 指数退避：100ms * 2^attempt，最大30秒
                let base_delay = 100u64;
                let max_delay = 30000u64;
                std::cmp::min(base_delay * 2u64.pow(attempt), max_delay)
            }
        }
    }
}

/// 歌词服务结果类型
pub type LyricsResult<T> = Result<T, LyricsError>;
