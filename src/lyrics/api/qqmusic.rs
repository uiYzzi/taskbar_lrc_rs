use crate::lyrics::{
    LyricsResult, LyricsError, LyricsData, SongInfo, QQSearchResult,
    QQSearchResponse, QQMusicLyricsResponse,
    http_client::HttpClient,
};
use super::common::{url_encode, build_query};
use tracing::{debug, warn};

/// QQ音乐API客户端
pub struct QQMusicApi {
    http_client: HttpClient,
    base_search_url: String,
    base_lyrics_url: String,
}

impl QQMusicApi {
    /// 创建新的QQ音乐API客户端
    pub fn new(http_client: HttpClient) -> Self {
        Self {
            http_client,
            base_search_url: "http://c.y.qq.com/soso/fcgi-bin/search_cp".to_string(),
            base_lyrics_url: "https://api.vkeys.cn/v2/music/tencent/lyric".to_string(),
        }
    }

    /// 搜索歌曲
    pub async fn search_song(&self, song_info: &SongInfo) -> LyricsResult<Option<QQSearchResult>> {
        if !song_info.is_valid() {
            return Err(LyricsError::InvalidSongInfo);
        }

        let query = build_query(&song_info.title, &song_info.artist);
        let encoded_query = url_encode(&query);

        let search_url = format!(
            "{}?t=0&aggr=1&cr=1&catZhida=1&lossless=0&flag_qc=0&p=1&w={}&n=1&g_tk=938407465&loginUin=0&hostUin=0&format=json&inCharset=utf8&outCharset=utf-8&notice=0&platform=yqq&needNewCode=0",
            self.base_search_url, encoded_query
        );

        debug!("QQ音乐搜索URL: {}", search_url);

        let response_text = self.http_client.get(&search_url).await?;
        self.parse_search_response(&response_text)
    }

    /// 获取歌词
    pub async fn get_lyrics(&self, song_id: &str, song_mid: &str) -> LyricsResult<LyricsData> {
        if song_id.is_empty() && song_mid.is_empty() {
            return Err(LyricsError::SongNotFound);
        }

        // 优先使用song_mid
        let lyrics_url = if !song_mid.is_empty() {
            format!("{}?mid={}", self.base_lyrics_url, song_mid)
        } else {
            format!("{}?id={}", self.base_lyrics_url, song_id)
        };

        debug!("QQ音乐歌词URL: {}", lyrics_url);

        let response_text = self.http_client.get(&lyrics_url).await?;
        self.parse_lyrics_response(&response_text)
    }

    /// 搜索并获取歌词
    pub async fn search_and_get_lyrics(&self, song_info: &SongInfo) -> LyricsResult<LyricsData> {
        // 先搜索歌曲
        let search_result = self.search_song(song_info).await?;
        
        match search_result {
            Some(result) => {
                debug!("找到QQ音乐歌曲: {} (ID: {}, MID: {})", result.title, result.song_id, result.song_mid);
                self.get_lyrics(&result.song_id, &result.song_mid).await
            }
            None => {
                warn!("未找到QQ音乐歌曲: {}", song_info);
                Err(LyricsError::SongNotFound)
            }
        }
    }

    /// 解析搜索响应
    fn parse_search_response(&self, response: &str) -> LyricsResult<Option<QQSearchResult>> {
        // 尝试使用serde_json解析
        match serde_json::from_str::<QQSearchResponse>(response) {
            Ok(parsed) => {
                if let Some(data) = parsed.data {
                    if let Some(song_data) = data.song {
                        if let Some(songs) = song_data.list {
                            if let Some(first_song) = songs.first() {
                                let artist_names: Vec<String> = first_song.singer
                                    .iter()
                                    .map(|singer| singer.name.clone())
                                    .collect();
                                
                                return Ok(Some(QQSearchResult {
                                    song_id: first_song.songid.to_string(),
                                    song_mid: first_song.songmid.clone(),
                                    title: first_song.songname.clone(),
                                    artist: artist_names.join(", "),
                                }));
                            }
                        }
                    }
                }
                Ok(None)
            }
            Err(_) => {
                // 如果JSON解析失败，尝试手动解析
                debug!("QQ音乐JSON解析失败，尝试手动解析");
                self.parse_search_response_manual(response)
            }
        }
    }

    /// 手动解析搜索响应（备用方法）
    fn parse_search_response_manual(&self, response: &str) -> LyricsResult<Option<QQSearchResult>> {
        // 查找song对象
        let song_pos = response.find("\"song\":")
            .ok_or_else(|| LyricsError::InternalError("未找到song字段".to_string()))?;

        // 查找list数组
        let list_pos = response[song_pos..]
            .find("\"list\":")
            .map(|pos| song_pos + pos)
            .ok_or_else(|| LyricsError::InternalError("未找到list字段".to_string()))?;

        // 找到第一个歌曲对象
        let first_song_start = response[list_pos..]
            .find("[")
            .and_then(|array_start| {
                response[list_pos + array_start..]
                    .find("{")
                    .map(|obj_start| list_pos + array_start + obj_start)
            })
            .ok_or_else(|| LyricsError::SongNotFound)?;

        // 查找对象结束
        let mut brace_count = 1;
        let mut obj_end = first_song_start + 1;
        
        while obj_end < response.len() && brace_count > 0 {
            match response.chars().nth(obj_end) {
                Some('{') => brace_count += 1,
                Some('}') => brace_count -= 1,
                _ => {}
            }
            obj_end += 1;
        }

        if brace_count != 0 {
            return Err(LyricsError::InternalError("歌曲对象不完整".to_string()));
        }

        let song_object = &response[first_song_start..obj_end];

        // 提取songid
        let song_id = self.extract_numeric_field(song_object, "songid")?;
        
        // 提取songmid
        let song_mid = self.extract_string_field(song_object, "songmid")?;

        if !song_id.is_empty() && !song_mid.is_empty() {
            Ok(Some(QQSearchResult {
                song_id,
                song_mid,
                title: "Unknown".to_string(), // 简化处理
                artist: "Unknown".to_string(),
            }))
        } else {
            Ok(None)
        }
    }

    /// 提取数字字段
    fn extract_numeric_field(&self, json: &str, field: &str) -> LyricsResult<String> {
        let search_key = format!("\"{}\":", field);
        
        if let Some(field_pos) = json.find(&search_key) {
            let start_pos = field_pos + search_key.len();
            
            // 跳过空白字符
            let mut current_pos = start_pos;
            while current_pos < json.len() && json.chars().nth(current_pos).map_or(false, |c| c.is_whitespace()) {
                current_pos += 1;
            }

            // 提取数字
            let mut end_pos = current_pos;
            while end_pos < json.len() && json.chars().nth(end_pos).map_or(false, |c| c.is_ascii_digit()) {
                end_pos += 1;
            }

            if end_pos > current_pos {
                return Ok(json[current_pos..end_pos].to_string());
            }
        }
        
        Ok(String::new())
    }

    /// 提取字符串字段
    fn extract_string_field(&self, json: &str, field: &str) -> LyricsResult<String> {
        let search_key = format!("\"{}\":", field);
        
        if let Some(field_pos) = json.find(&search_key) {
            let start_pos = field_pos + search_key.len();
            
            // 跳过空白字符
            let mut current_pos = start_pos;
            while current_pos < json.len() && json.chars().nth(current_pos).map_or(false, |c| c.is_whitespace()) {
                current_pos += 1;
            }

            // 检查是否是字符串值
            if current_pos < json.len() && json.chars().nth(current_pos) == Some('"') {
                current_pos += 1; // 跳过开始引号

                // 查找结束引号
                let mut end_pos = current_pos;
                while end_pos < json.len() && json.chars().nth(end_pos) != Some('"') {
                    end_pos += 1;
                }

                if end_pos < json.len() {
                    return Ok(json[current_pos..end_pos].to_string());
                }
            }
        }
        
        Ok(String::new())
    }

    /// 解析歌词响应
    fn parse_lyrics_response(&self, response: &str) -> LyricsResult<LyricsData> {
        match serde_json::from_str::<QQMusicLyricsResponse>(response) {
            Ok(api_response) => {
                let lyrics_data = LyricsData::from_qqmusic_response(api_response);
                
                if !lyrics_data.has_any_content() {
                    Err(LyricsError::LyricsNotFound)
                } else {
                    Ok(lyrics_data)
                }
            }
            Err(e) => {
                warn!("解析QQ音乐歌词响应失败: {}", e);
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
    async fn test_qqmusic_api_creation() {
        let http_client = HttpClient::new(HttpClientConfig::default()).unwrap();
        let api = QQMusicApi::new(http_client);
        
        assert_eq!(api.base_search_url, "http://c.y.qq.com/soso/fcgi-bin/search_cp");
    }

    #[test]
    fn test_extract_numeric_field() {
        let http_client = HttpClient::new(HttpClientConfig::default()).unwrap();
        let api = QQMusicApi::new(http_client);
        
        let json = r#"{"songid": 123, "other": "value"}"#;
        let result = api.extract_numeric_field(json, "songid").unwrap();
        assert_eq!(result, "123");
    }

    #[test]
    fn test_extract_string_field() {
        let http_client = HttpClient::new(HttpClientConfig::default()).unwrap();
        let api = QQMusicApi::new(http_client);
        
        let json = r#"{"songmid": "abc123", "other": 456}"#;
        let result = api.extract_string_field(json, "songmid").unwrap();
        assert_eq!(result, "abc123");
    }
}
