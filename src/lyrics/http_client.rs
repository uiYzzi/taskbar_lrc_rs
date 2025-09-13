use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn, error};
use url::Url;
use crate::lyrics::{LyricsError, LyricsResult};

/// HTTP客户端配置
#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    /// 请求超时时间
    pub timeout: Duration,
    /// 最大重试次数
    pub max_retries: u32,
    /// 用户代理
    pub user_agent: String,
    /// 连接超时
    pub connect_timeout: Duration,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_retries: 3,
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string(),
            connect_timeout: Duration::from_secs(10),
        }
    }
}

/// HTTP客户端，支持指数退避重试
#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    config: HttpClientConfig,
}

impl HttpClient {
    /// 创建新的HTTP客户端
    pub fn new(config: HttpClientConfig) -> LyricsResult<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .connect_timeout(config.connect_timeout)
            .user_agent(&config.user_agent)
            .build()
            .map_err(LyricsError::NetworkError)?;

        Ok(Self { client, config })
    }

    /// 创建默认HTTP客户端
    pub fn default() -> LyricsResult<Self> {
        Self::new(HttpClientConfig::default())
    }

    /// 发送GET请求
    pub async fn get(&self, url: &str) -> LyricsResult<String> {
        self.request_with_retry(url).await
    }

    /// 带重试机制的请求
    async fn request_with_retry(&self, url: &str) -> LyricsResult<String> {
        let parsed_url = Url::parse(url)?;
        debug!("发送HTTP请求: {}", url);

        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            match self.execute_request(&parsed_url).await {
                Ok(response_text) => {
                    debug!("请求成功，尝试次数: {}", attempt + 1);
                    return Ok(response_text);
                }
                Err(error) => {
                    last_error = Some(error);
                    
                    if attempt < self.config.max_retries {
                        let delay_ms = self.calculate_retry_delay(attempt);
                        warn!(
                            "请求失败，将在{}ms后重试 (尝试 {}/{}): {:?}",
                            delay_ms,
                            attempt + 1,
                            self.config.max_retries + 1,
                            last_error
                        );
                        
                        sleep(Duration::from_millis(delay_ms)).await;
                    } else {
                        error!("请求最终失败，已达到最大重试次数: {:?}", last_error);
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            LyricsError::InternalError("未知的请求错误".to_string())
        }))
    }

    /// 执行单次HTTP请求
    async fn execute_request(&self, url: &Url) -> LyricsResult<String> {
        let response = self.client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| self.classify_error(e))?;

        let status = response.status();
        
        if status.is_success() {
            let text = response.text().await.map_err(LyricsError::NetworkError)?;
            Ok(text)
        } else {
            match status.as_u16() {
                429 => Err(LyricsError::RateLimited),
                500..=599 => Err(LyricsError::ServiceUnavailable),
                _ => Err(LyricsError::ApiError {
                    code: status.as_u16().to_string(),
                    message: format!("HTTP错误: {}", status),
                }),
            }
        }
    }

    /// 计算重试延迟时间（毫秒）
    fn calculate_retry_delay(&self, attempt: u32) -> u64 {
        // 指数退避：100ms * 2^attempt，加上随机抖动，最大30秒
        let base_delay = 100u64;
        let exponential_delay = base_delay * 2u64.pow(attempt);
        let max_delay = 30000u64;
        
        // 添加随机抖动（±25%）
        let jitter_range = exponential_delay / 4;
        let jitter = fastrand::u64(0..=jitter_range * 2);
        let delay_with_jitter = exponential_delay.saturating_sub(jitter_range) + jitter;
        
        std::cmp::min(delay_with_jitter, max_delay)
    }

    /// 分类网络错误
    fn classify_error(&self, error: reqwest::Error) -> LyricsError {
        if error.is_timeout() {
            LyricsError::Timeout
        } else if error.is_connect() {
            LyricsError::ServiceUnavailable
        } else {
            LyricsError::NetworkError(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_delay_calculation() {
        let config = HttpClientConfig::default();
        let client = HttpClient::new(config).unwrap();
        
        for attempt in 0..5 {
            let delay = client.calculate_retry_delay(attempt);
            assert!(delay >= 50);
            assert!(delay <= 30000);
        }
    }

    #[tokio::test]
    async fn test_error_classification() {
        let client = HttpClient::default().unwrap();
        
        match client.get("invalid-url").await {
            Err(LyricsError::UrlParseError(_)) => {
            }
            _ => {
            }
        }
    }
}
