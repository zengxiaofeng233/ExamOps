use crate::error::{AppError, AppResult};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_IMAGES: usize = 4;
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
// Image tokenization varies by model; reserve space without counting base64 as text.
pub const IMAGE_TOKEN_RESERVE: usize = 2048;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ImageAttachment {
    pub name: String,
    pub data_url: String,
}

pub fn validate(images: &[ImageAttachment]) -> AppResult<()> {
    if images.len() > MAX_IMAGES {
        return Err(AppError::invalid("每条消息最多上传 4 张图片"));
    }
    for image in images {
        if image.name.chars().count() > 255 || image.data_url.len() > MAX_IMAGE_BYTES.div_ceil(3) * 4 + 32 {
            return Err(AppError::invalid("图片名称过长或图片超过 5 MB"));
        }
        let (header, encoded) = image.data_url.split_once(',').ok_or_else(|| AppError::invalid("图片数据无效"))?;
        if !matches!(header, "data:image/png;base64" | "data:image/jpeg;base64" | "data:image/webp;base64") {
            return Err(AppError::invalid("仅支持 JPG、PNG、WebP 图片"));
        }
        let bytes = base64::engine::general_purpose::STANDARD.decode(encoded).map_err(|_| AppError::invalid("图片数据无效"))?;
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(AppError::invalid("每张图片不能超过 5 MB"));
        }
        let valid = match header {
            "data:image/png;base64" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            "data:image/jpeg;base64" => bytes.starts_with(b"\xff\xd8\xff"),
            "data:image/webp;base64" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
            _ => false,
        };
        if !valid {
            return Err(AppError::invalid("图片内容与格式不匹配，请重新选择图片"));
        }
    }
    Ok(())
}

pub fn from_meta(meta: &Value) -> AppResult<Vec<ImageAttachment>> {
    match meta.get("images") {
        Some(images) => Ok(serde_json::from_value(images.clone())?),
        None => Ok(Vec::new()),
    }
}
