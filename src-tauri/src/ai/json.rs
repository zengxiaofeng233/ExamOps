//! Extracting JSON from model replies (models often wrap it in prose or fences).
//! Everything parsed here is validated by the caller before it touches the
//! database: model output is data, never commands.

use serde::de::DeserializeOwned;

/// Finds the first parseable JSON object/array in `text`.
pub fn extract_value(text: &str) -> Option<serde_json::Value> {
    let trimmed = text.trim();
    if let Ok(value) = serde_json::from_str(trimmed) {
        return Some(value);
    }
    // ```json ... ``` fences
    let mut rest = trimmed;
    while let Some(start) = rest.find("```") {
        let after = &rest[start + 3..];
        let body_start = after.find('\n').map(|i| i + 1).unwrap_or(0);
        if let Some(end) = after[body_start..].find("```") {
            let body = &after[body_start..body_start + end];
            if let Ok(value) = serde_json::from_str(body.trim()) {
                return Some(value);
            }
            rest = &after[body_start + end + 3..];
        } else {
            break;
        }
    }
    // First balanced {...} or [...] outside of strings.
    let chars: Vec<(usize, char)> = trimmed.char_indices().collect();
    for (index, &(start, open)) in chars.iter().enumerate() {
        if open != '{' && open != '[' {
            continue;
        }
        let close = if open == '{' { '}' } else { ']' };
        let (mut depth, mut in_string, mut escaped) = (0i32, false, false);
        for &(position, c) in &chars[index..] {
            if in_string {
                match (escaped, c) {
                    (true, _) => escaped = false,
                    (false, '\\') => escaped = true,
                    (false, '"') => in_string = false,
                    _ => {}
                }
                continue;
            }
            match c {
                '"' => in_string = true,
                c if c == open => depth += 1,
                c if c == close => {
                    depth -= 1;
                    if depth == 0 {
                        let candidate = &trimmed[start..position + c.len_utf8()];
                        if let Ok(value) = serde_json::from_str(candidate) {
                            return Some(value);
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    None
}

pub fn extract<T: DeserializeOwned>(text: &str) -> Result<T, String> {
    let value = extract_value(text).ok_or_else(|| "AI 回复中没有找到 JSON 数据".to_string())?;
    serde_json::from_value(value).map_err(|e| format!("AI 回复的数据格式不符合要求：{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct Item {
        title: String,
    }

    #[test]
    fn finds_json_in_fences_prose_and_raw() {
        assert_eq!(extract::<Item>(r#"{"title":"极限"}"#).unwrap().title, "极限");
        let fenced = "好的，以下是结果：\n```json\n{\"title\": \"导数\"}\n```\n希望有帮助";
        assert_eq!(extract::<Item>(fenced).unwrap().title, "导数");
        let prose = "结果如下 {\"title\": \"含有 } 括号\"} 结束";
        assert_eq!(extract::<Item>(prose).unwrap().title, "含有 } 括号");
        let list: Vec<Item> = extract("[{\"title\":\"a\"},{\"title\":\"b\"}]").unwrap();
        assert_eq!(list.len(), 2);
        assert!(extract::<Item>("没有数据").is_err());
        assert!(extract::<Item>("{\"name\": 1}").unwrap_err().contains("格式"));
    }
}
