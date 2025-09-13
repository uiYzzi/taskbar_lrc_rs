use crate::lyrics::{
    LyricsResult, LyricsError, LyricsData, SongInfo, SearchResult,
    NetEaseSearchResponse, NetEaseLyricsResponse,
    http_client::HttpClient,
};
use super::common::{url_encode, build_query};
use tracing::{debug, warn};

/// 网易云音乐API客户端
pub struct NetEaseApi {
    http_client: HttpClient,
    base_search_url: String,
    base_lyrics_url: String,
}

impl NetEaseApi {
    /// 创建新的网易云音乐API客户端
    pub fn new(http_client: HttpClient) -> Self {
        Self {
            http_client,
            base_search_url: "https://music.163.com/api/search/get/web".to_string(),
            base_lyrics_url: "https://api.vkeys.cn/v2/music/netease/lyric".to_string(),
        }
    }

    /// 搜索歌曲
    pub async fn search_song(&self, song_info: &SongInfo) -> LyricsResult<Option<SearchResult>> {
        if !song_info.is_valid() {
            return Err(LyricsError::InvalidSongInfo);
        }

        let query = build_query(&song_info.title, &song_info.artist);
        let encoded_query = url_encode(&query);

        let search_url = format!(
            "{}?csrf_token=hlpretag=&hlposttag=&s={}&type=1&offset=0&total=true&limit=1",
            self.base_search_url, encoded_query
        );

        debug!("网易云搜索URL: {}", search_url);

        let response_text = self.http_client.get(&search_url).await?;
        self.parse_search_response(&response_text)
    }

    /// 获取歌词
    pub async fn get_lyrics(&self, music_id: &str) -> LyricsResult<LyricsData> {
        if music_id.is_empty() {
            return Err(LyricsError::SongNotFound);
        }

        let lyrics_url = format!("{}?id={}", self.base_lyrics_url, music_id);
        debug!("网易云歌词URL: {}", lyrics_url);

        let response_text = self.http_client.get(&lyrics_url).await?;
        self.parse_lyrics_response(&response_text)
    }

    /// 搜索并获取歌词
    pub async fn search_and_get_lyrics(&self, song_info: &SongInfo) -> LyricsResult<LyricsData> {
        // 先搜索歌曲
        let search_result = self.search_song(song_info).await?;
        
        match search_result {
            Some(result) => {
                debug!("找到歌曲: {} (ID: {})", result.title, result.id);
                self.get_lyrics(&result.id).await
            }
            None => {
                warn!("未找到歌曲: {}", song_info);
                Err(LyricsError::SongNotFound)
            }
        }
    }

    /// 解析搜索响应
    fn parse_search_response(&self, response: &str) -> LyricsResult<Option<SearchResult>> {
        // 尝试使用serde_json解析
        match serde_json::from_str::<NetEaseSearchResponse>(response) {
            Ok(parsed) => {
                if let Some(result) = parsed.result {
                    if let Some(songs) = result.songs {
                        if let Some(first_song) = songs.first() {
                            let artist_names: Vec<String> = first_song.ar
                                .iter()
                                .map(|artist| artist.name.clone())
                                .collect();
                            
                            return Ok(Some(SearchResult {
                                id: first_song.id.to_string(),
                                title: first_song.name.clone(),
                                artist: artist_names.join(", "),
                                duration: first_song.dt.map(|ms| std::time::Duration::from_millis(ms)),
                            }));
                        }
                    }
                }
                Ok(None)
            }
            Err(_) => {
                // 如果JSON解析失败，尝试手动解析
                debug!("JSON解析失败，尝试手动解析");
                self.parse_search_response_manual(response)
            }
        }
    }

    /// 手动解析搜索响应（备用方法）
    fn parse_search_response_manual(&self, response: &str) -> LyricsResult<Option<SearchResult>> {
        // 使用正则表达式或更安全的方法来解析ID
        // 查找第一个歌曲的ID
        if let Some(id) = self.extract_first_song_id(response) {
            if !id.is_empty() && id != "0" {
                return Ok(Some(SearchResult {
                    id,
                    title: "Unknown".to_string(), // 简化处理
                    artist: "Unknown".to_string(),
                    duration: None,
                }));
            }
        }
        
        Ok(None)
    }
    
    /// 安全地提取第一个歌曲的ID
    fn extract_first_song_id(&self, response: &str) -> Option<String> {
        // 查找songs数组的开始
        let songs_start = response.find("\"songs\":")? + 8; // 跳过 "songs":
        
        // 在songs部分查找第一个id字段
        let songs_section = &response[songs_start..];
        
        // 使用更安全的字符串搜索方法
        let mut chars = songs_section.char_indices();
        let mut in_id_field = false;
        let mut id_value_start = None;
        let mut in_quotes = false;
        let mut escape_next = false;
        
        while let Some((byte_idx, ch)) = chars.next() {
            if escape_next {
                escape_next = false;
                continue;
            }
            
            match ch {
                '\\' => escape_next = true,
                '"' => {
                    if in_id_field && !in_quotes {
                        // 开始读取ID值
                        in_quotes = true;
                    } else if in_quotes {
                        // 结束读取ID值
                        if let Some(start_idx) = id_value_start {
                            let id_str = &songs_section[start_idx..byte_idx];
                            if id_str.chars().all(|c| c.is_ascii_digit()) && !id_str.is_empty() {
                                return Some(id_str.to_string());
                            }
                        }
                        in_quotes = false;
                        in_id_field = false;
                        id_value_start = None;
                    }
                }
                ':' if !in_quotes => {
                    // 检查前面是否是"id"
                    let before_colon = &songs_section[..byte_idx];
                    if before_colon.ends_with("\"id\"") {
                        in_id_field = true;
                    }
                }
                c if in_id_field && !in_quotes && c.is_ascii_digit() => {
                    // 开始数字ID（不在引号中）
                    if id_value_start.is_none() {
                        id_value_start = Some(byte_idx);
                    }
                }
                c if in_id_field && !in_quotes && !c.is_ascii_digit() && !c.is_whitespace() => {
                    // 结束数字ID
                    if let Some(start_idx) = id_value_start {
                        let id_str = &songs_section[start_idx..byte_idx];
                        if !id_str.is_empty() {
                            return Some(id_str.to_string());
                        }
                    }
                    in_id_field = false;
                    id_value_start = None;
                }
                _ => {}
            }
        }
        
        // 如果到达字符串末尾且还在读取ID
        if let Some(start_idx) = id_value_start {
            let id_str = &songs_section[start_idx..];
            if id_str.chars().all(|c| c.is_ascii_digit()) && !id_str.is_empty() {
                return Some(id_str.to_string());
            }
        }
        
        None
    }

    /// 解析歌词响应
    fn parse_lyrics_response(&self, response: &str) -> LyricsResult<LyricsData> {
        match serde_json::from_str::<NetEaseLyricsResponse>(response) {
            Ok(api_response) => {
                let lyrics_data = LyricsData::from_netease_response(api_response);
                
                if !lyrics_data.has_any_content() {
                    Err(LyricsError::LyricsNotFound)
                } else {
                    Ok(lyrics_data)
                }
            }
            Err(e) => {
                warn!("解析歌词响应失败: {}", e);
                Err(LyricsError::JsonParseError(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::http_client::HttpClientConfig;

    #[tokio::test]
    async fn test_netease_api_creation() {
        let http_client = HttpClient::new(HttpClientConfig::default()).unwrap();
        let api = NetEaseApi::new(http_client);
        
        assert_eq!(api.base_search_url, "https://music.163.com/api/search/get/web");
    }

    #[test]
    fn test_parse_search_response_manual() {
        let http_client = HttpClient::new(HttpClientConfig::default()).unwrap();
        let api = NetEaseApi::new(http_client);
        
        let mock_response = r#"{"result":{"songs":[{"name":"test","alias":[],"id":123}]}}"#;
        let result = api.parse_search_response_manual(mock_response);
        
        // 这个测试可能需要根据实际的解析逻辑调整
        assert!(result.is_ok());
    }
}
