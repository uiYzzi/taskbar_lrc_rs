use url::form_urlencoded;
use crate::lyrics::{LyricsResult, LyricsError};

/// URL编码工具
pub fn url_encode(input: &str) -> String {
    form_urlencoded::byte_serialize(input.as_bytes()).collect()
}

/// 提取JSON字符串值
pub fn extract_json_string(json: &str, key: &str) -> Option<String> {
    // 简单的JSON字符串提取，用于处理API响应
    let search_key = format!("\"{}\":", key);
    
    if let Some(key_pos) = json.find(&search_key) {
        let start_pos = key_pos + search_key.len();
        
        // 跳过空白字符
        let mut current_pos = start_pos;
        while current_pos < json.len() && json.chars().nth(current_pos)?.is_whitespace() {
            current_pos += 1;
        }
        
        // 检查是否是字符串值
        if current_pos < json.len() && json.chars().nth(current_pos)? == '"' {
            current_pos += 1; // 跳过开始引号
            
            // 查找结束引号
            let mut end_pos = current_pos;
            while end_pos < json.len() {
                let ch = json.chars().nth(end_pos)?;
                if ch == '"' {
                    // 检查是否是转义的引号
                    if end_pos == current_pos || json.chars().nth(end_pos - 1)? != '\\' {
                        break;
                    }
                }
                end_pos += 1;
            }
            
            if end_pos < json.len() {
                return Some(json[current_pos..end_pos].to_string());
            }
        }
    }
    
    None
}

/// 查找JSON中第一个指定对象的ID
pub fn find_first_id(json: &str, parent_key: &str, id_key: &str) -> LyricsResult<Option<String>> {
    // 查找父对象
    let parent_search = format!("\"{}\":", parent_key);
    let parent_pos = json.find(&parent_search)
        .ok_or_else(|| LyricsError::InternalError(format!("未找到父对象: {}", parent_key)))?;
    
    // 从父对象位置开始查找
    let search_slice = &json[parent_pos..];
    
    // 查找数组开始
    let array_start = search_slice.find('[')
        .ok_or_else(|| LyricsError::InternalError("未找到数组开始".to_string()))?;
    
    // 查找第一个对象
    let obj_start = search_slice[array_start..]
        .find('{')
        .ok_or_else(|| LyricsError::InternalError("未找到对象开始".to_string()))?;
    
    let obj_start_absolute = parent_pos + array_start + obj_start;
    
    // 查找对象结束
    let mut brace_count = 1;
    let mut obj_end = obj_start_absolute + 1;
    
    while obj_end < json.len() && brace_count > 0 {
        match json.chars().nth(obj_end) {
            Some('{') => brace_count += 1,
            Some('}') => brace_count -= 1,
            _ => {}
        }
        obj_end += 1;
    }
    
    if brace_count != 0 {
        return Err(LyricsError::InternalError("JSON对象不完整".to_string()));
    }
    
    // 在对象范围内查找ID
    let obj_content = &json[obj_start_absolute..obj_end];
    
    if let Some(id_str) = extract_json_string(obj_content, id_key) {
        if !id_str.is_empty() && id_str != "0" {
            return Ok(Some(id_str));
        }
    }
    
    Ok(None)
}

/// 构建查询字符串
pub fn build_query(title: &str, artist: &str) -> String {
    format!("{} {}", title.trim(), artist.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("测试"), "%E6%B5%8B%E8%AF%95");
    }

    #[test]
    fn test_extract_json_string() {
        let json = r#"{"name": "test song", "id": "123"}"#;
        assert_eq!(extract_json_string(json, "name"), Some("test song".to_string()));
        assert_eq!(extract_json_string(json, "id"), Some("123".to_string()));
        assert_eq!(extract_json_string(json, "missing"), None);
    }

    #[test]
    fn test_build_query() {
        assert_eq!(build_query("  Song Title  ", "  Artist Name  "), "Song Title Artist Name");
    }
}
