//! Bilibili text import. Cookies stay in the OS credential store and are never
//! returned to the UI, passed to the model, or sent to subtitle CDNs.
use crate::error::{AppError, AppResult};
use crate::security::{KeyringStore, SecretStore};
use base64::Engine;
use reqwest::{header, Client, Response};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use url::Url;

const API: &str = "https://api.bilibili.com";
const PASSPORT: &str = "https://passport.bilibili.com/x/passport-login/web/qrcode";
const SECRET: &str = "bilibili-session";
const MAX_BYTES: usize = 8 * 1024 * 1024;
pub const SUBTITLE_HEADER: &str = "【B站字幕】";
pub const SUMMARY_HEADER: &str = "【B站 AI 总结（非完整字幕）】";

pub struct BilibiliState {
    secrets: Arc<dyn SecretStore>,
    pending: Mutex<Option<(String, Instant)>>,
    generation: AtomicU64,
}

impl Default for BilibiliState {
    fn default() -> Self {
        Self {
            secrets: Arc::new(KeyringStore),
            pending: Mutex::new(None),
            generation: AtomicU64::new(0),
        }
    }
}

#[derive(Serialize)]
pub struct Session {
    pub logged_in: bool,
    pub name: String,
}
#[derive(Serialize)]
pub struct LoginQr {
    pub key: String,
    pub image: String,
}
#[derive(Serialize)]
pub struct LoginPoll {
    pub status: String,
}
#[derive(Serialize)]
pub struct VideoText {
    pub title: String,
    pub url: String,
    pub kind: String,
    pub transcript: String,
    pub notice: String,
}

fn client() -> AppResult<Client> {
    Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/130.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build().map_err(|_| AppError::msg("无法初始化 B 站连接"))
}

fn network_error(_: reqwest::Error) -> AppError {
    // reqwest errors can contain a signed subtitle URL or a login key.
    AppError::msg("连接 B 站失败或超时，请稍后重试")
}

async fn json(mut response: Response) -> AppResult<Value> {
    if !response.status().is_success() {
        return Err(AppError::msg(format!(
            "B 站暂时拒绝访问（HTTP {}），请稍后重试",
            response.status().as_u16()
        )));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(network_error)? {
        if bytes.len() + chunk.len() > MAX_BYTES {
            return Err(AppError::msg("B 站返回的文字过大，请手动导入"));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| AppError::msg("B 站返回了无法识别的内容，请稍后重试"))
}

async fn request(client: &Client, url: &str, cookie: Option<&str>) -> AppResult<Value> {
    let mut req = client
        .get(url)
        .header(header::REFERER, "https://www.bilibili.com/");
    if let Some(cookie) = cookie {
        req = req.header(header::COOKIE, cookie);
    }
    json(req.send().await.map_err(network_error)?).await
}

fn data(value: &Value) -> AppResult<&Value> {
    match value["code"].as_i64() {
        Some(0) => Ok(&value["data"]),
        Some(-101) => Err(AppError::msg(
            "此内容需要登录 B 站，或登录已过期，请扫码登录后重试",
        )),
        Some(-352 | -412 | -403) => Err(AppError::msg(
            "B 站暂时限制了访问，请稍后重试，或手动粘贴字幕",
        )),
        Some(-404 | 62002) => Err(AppError::msg("视频不存在或当前账号无权访问")),
        Some(code) => Err(AppError::msg(format!(
            "B 站未能提供内容（代码 {code}），请稍后重试"
        ))),
        None => Err(AppError::msg("B 站响应格式已变化，暂时无法读取")),
    }
}

async fn cookie(state: &BilibiliState) -> AppResult<Option<String>> {
    let secrets = state.secrets.clone();
    tauri::async_runtime::spawn_blocking(move || secrets.get(SECRET))
        .await?
        .map_err(|_| AppError::msg("无法读取系统凭据管理器中的 B 站登录信息"))
}

pub async fn session(state: &BilibiliState) -> AppResult<Session> {
    let Some(cookie) = cookie(state).await? else {
        return Ok(Session {
            logged_in: false,
            name: String::new(),
        });
    };
    let value = request(
        &client()?,
        &format!("{API}/x/web-interface/nav"),
        Some(&cookie),
    )
    .await?;
    if value["code"] == -101 {
        return Ok(Session {
            logged_in: false,
            name: String::new(),
        });
    }
    let nav = data(&value)?;
    Ok(Session {
        logged_in: nav["isLogin"].as_bool().unwrap_or(false),
        name: nav["uname"].as_str().unwrap_or("").into(),
    })
}

pub async fn start_login(state: &BilibiliState) -> AppResult<LoginQr> {
    let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    *state.pending.lock().await = None;
    let value = request(&client()?, &format!("{PASSPORT}/generate"), None).await?;
    let value = data(&value)?;
    let key = value["qrcode_key"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::msg("未获取到登录二维码"))?;
    let url = value["url"]
        .as_str()
        .ok_or_else(|| AppError::msg("未获取到登录二维码"))?;
    let parsed = Url::parse(url).map_err(|_| AppError::msg("登录二维码无效"))?;
    if parsed.scheme() != "https"
        || !matches!(
            parsed.host_str(),
            Some("passport.bilibili.com" | "account.bilibili.com")
        )
        || parsed.port().is_some()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(AppError::msg("登录二维码来源异常"));
    }
    let svg = qrcode::QrCode::new(url.as_bytes())
        .map_err(|_| AppError::msg("无法生成登录二维码"))?
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(240, 240)
        .build();
    let mut pending = state.pending.lock().await;
    if state.generation.load(Ordering::SeqCst) != generation {
        return Err(AppError::msg("二维码请求已取消"));
    }
    *pending = Some((key.into(), Instant::now()));
    Ok(LoginQr {
        key: key.into(),
        image: format!(
            "data:image/svg+xml;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(svg)
        ),
    })
}

fn login_cookies(headers: &header::HeaderMap) -> AppResult<String> {
    let mut cookies = BTreeMap::new();
    for value in headers.get_all(header::SET_COOKIE) {
        let Some((name, value)) = value
            .to_str()
            .ok()
            .and_then(|s| s.split(';').next())
            .and_then(|s| s.split_once('='))
        else {
            continue;
        };
        if matches!(
            name,
            "SESSDATA" | "bili_jct" | "DedeUserID" | "DedeUserID__ckMd5"
        ) && !value.is_empty()
        {
            cookies.insert(name, value);
        }
    }
    if !cookies.contains_key("SESSDATA") {
        return Err(AppError::msg("B 站未返回有效登录凭据，请重新扫码"));
    }
    Ok(cookies
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; "))
}

pub async fn poll_login(state: &BilibiliState, key: String) -> AppResult<LoginPoll> {
    {
        let pending = state.pending.lock().await;
        if !pending
            .as_ref()
            .is_some_and(|(k, at)| *k == key && at.elapsed() < Duration::from_secs(180))
        {
            return Ok(LoginPoll {
                status: "expired".into(),
            });
        }
    }
    let client = client()?;
    let response = client
        .get(format!("{PASSPORT}/poll"))
        .query(&[("qrcode_key", &key)])
        .header(header::REFERER, "https://www.bilibili.com/")
        .send()
        .await
        .map_err(network_error)?;
    let headers = response.headers().clone();
    let response = json(response).await?;
    let status = match data(&response)?["code"].as_i64() {
        Some(86101) => "waiting",
        Some(86090) => "scanned",
        Some(86038) => "expired",
        Some(0) => {
            let cookie = login_cookies(&headers)?;
            let nav = request(
                &client,
                &format!("{API}/x/web-interface/nav"),
                Some(&cookie),
            )
            .await?;
            if data(&nav)?["isLogin"] != true {
                return Err(AppError::msg("登录验证失败，请重新扫码"));
            }
            let mut pending = state.pending.lock().await;
            if !pending.as_ref().is_some_and(|(k, _)| *k == key) {
                return Ok(LoginPoll {
                    status: "expired".into(),
                });
            }
            let secrets = state.secrets.clone();
            tauri::async_runtime::spawn_blocking(move || secrets.set(SECRET, &cookie))
                .await?
                .map_err(|_| AppError::msg("系统凭据管理器不可用，未保存登录信息，请重试"))?;
            *pending = None;
            "success"
        }
        _ => return Err(AppError::msg("B 站登录状态异常，请重新扫码")),
    };
    Ok(LoginPoll {
        status: status.into(),
    })
}

pub async fn cancel_login(state: &BilibiliState, key: String) {
    let mut pending = state.pending.lock().await;
    if pending.as_ref().is_some_and(|(k, _)| *k == key) {
        *pending = None;
    }
}

pub async fn forget_login(state: &BilibiliState) -> AppResult<()> {
    let mut pending = state.pending.lock().await;
    state.generation.fetch_add(1, Ordering::SeqCst);
    *pending = None;
    let secrets = state.secrets.clone();
    tauri::async_runtime::spawn_blocking(move || secrets.delete(SECRET))
        .await?
        .map_err(|_| AppError::msg("未能清除 B 站登录凭据，请重试"))
}

fn video_url(input: &str) -> AppResult<Url> {
    let mut url =
        Url::parse(input.trim()).map_err(|_| AppError::invalid("请填写完整的 B 站视频链接"))?;
    if !matches!(url.scheme(), "https" | "http")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || !matches!(
            url.host_str(),
            Some("www.bilibili.com" | "bilibili.com" | "m.bilibili.com" | "b23.tv")
        )
    {
        return Err(AppError::invalid(
            "只支持 bilibili.com 视频链接和 b23.tv 短链接",
        ));
    }
    let _ = url.set_scheme("https");
    Ok(url)
}

async fn resolve_video(client: &Client, input: &str) -> AppResult<Url> {
    let mut url = video_url(input)?;
    for _ in 0..4 {
        if url.host_str() != Some("b23.tv") {
            return Ok(url);
        }
        // Short-link redirects are followed manually, without any login cookies.
        let response = client
            .get(url.clone())
            .send()
            .await
            .map_err(network_error)?;
        let next = response
            .headers()
            .get(header::LOCATION)
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| {
                AppError::msg("无法解析 B 站短链接，请使用浏览器地址栏中的完整视频链接")
            })?;
        url = video_url(
            url.join(next)
                .map_err(|_| AppError::msg("短链接跳转无效"))?
                .as_str(),
        )?;
    }
    Err(AppError::msg("短链接跳转次数过多，请使用完整视频链接"))
}

fn video_id(url: &Url) -> AppResult<(&'static str, String, u64)> {
    let segments: Vec<_> = url.path_segments().map(|s| s.collect()).unwrap_or_default();
    let id = segments
        .get(1)
        .filter(|_| segments.first() == Some(&"video"))
        .copied()
        .unwrap_or("");
    let kind = if id.starts_with("BV")
        && id.len() == 12
        && id.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        "bvid"
    } else if id.starts_with("av") && id.len() > 2 && id[2..].bytes().all(|b| b.is_ascii_digit()) {
        "aid"
    } else {
        return Err(AppError::invalid(
            "请使用 /video/BV… 或 /video/av… 视频链接",
        ));
    };
    let page = url
        .query_pairs()
        .find(|(k, _)| k == "p")
        .map(|(_, v)| v.parse::<u64>())
        .transpose()
        .map_err(|_| AppError::invalid("视频分 P 编号无效"))?
        .unwrap_or(1);
    if page == 0 {
        return Err(AppError::invalid("视频分 P 编号从 1 开始"));
    }
    Ok((
        kind,
        if kind == "aid" {
            id[2..].into()
        } else {
            id.into()
        },
        page,
    ))
}

fn signed_query(
    mut params: BTreeMap<String, String>,
    nav: &Value,
    timestamp: i64,
) -> AppResult<String> {
    let mut raw = String::new();
    for field in ["img_url", "sub_url"] {
        let url = nav["wbi_img"][field].as_str().unwrap_or("");
        let key = url
            .rsplit('/')
            .next()
            .unwrap_or("")
            .split('.')
            .next()
            .unwrap_or("");
        if key.len() != 32 || !key.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(AppError::msg("无法取得 B 站接口签名，请稍后重试"));
        }
        raw.push_str(key);
    }
    // Public WBI permutation; only the first 32 positions form the signing key.
    const ORDER: [usize; 32] = [
        46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19,
        29, 28, 14, 39, 12, 38, 41, 13,
    ];
    let mixin: String = ORDER.iter().map(|&i| raw.as_bytes()[i] as char).collect();
    params.insert("wts".into(), timestamp.to_string());
    let mut encoder = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in params {
        encoder.append_pair(&key, &value.replace(['!', '\'', '(', ')', '*'], ""));
    }
    let query = encoder.finish().replace('+', "%20");
    Ok(format!(
        "{query}&w_rid={:x}",
        md5::compute(format!("{query}{mixin}"))
    ))
}

fn subtitle_url(input: &str) -> AppResult<Url> {
    let input = if input.starts_with("//") {
        format!("https:{input}")
    } else {
        input.into()
    };
    let url = Url::parse(&input).map_err(|_| AppError::msg("字幕地址无效"))?;
    let host = url.host_str().unwrap_or("");
    if url.scheme() != "https"
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || !(host == "hdslb.com"
            || host.ends_with(".hdslb.com")
            || host == "bilibili.com"
            || host.ends_with(".bilibili.com"))
    {
        return Err(AppError::msg("字幕来源异常，未读取该地址"));
    }
    Ok(url)
}

fn timestamp(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

fn subtitle_text(value: &Value) -> AppResult<String> {
    let rows = value["body"]
        .as_array()
        .ok_or_else(|| AppError::msg("字幕格式无法识别"))?;
    let lines: Vec<_> = rows
        .iter()
        .filter_map(|r| {
            let text = r["content"].as_str()?.trim();
            if text.is_empty() {
                return None;
            }
            Some(format!(
                "[{}] {}",
                timestamp(r["from"].as_f64().unwrap_or(0.0)),
                text
            ))
        })
        .collect();
    if lines.is_empty() {
        return Err(AppError::msg("字幕内容为空"));
    }
    Ok(lines.join("\n"))
}

fn summary_text(value: &Value) -> AppResult<String> {
    let value = data(value)?;
    if value["code"].as_i64() != Some(0) {
        return Err(AppError::msg("该分 P 暂无可读取的 AI 总结"));
    }
    let result = &value["model_result"];
    let mut parts = Vec::new();
    if let Some(summary) = result["summary"].as_str().filter(|s| !s.trim().is_empty()) {
        parts.push(summary.trim().to_string());
    }
    if let Some(outline) = result["outline"].as_array() {
        for item in outline {
            if let Some(title) = item["title"].as_str() {
                parts.push(format!(
                    "[{}] {}",
                    timestamp(item["timestamp"].as_f64().unwrap_or(0.0)),
                    title
                ));
            }
            if let Some(items) = item["part_outline"].as_array() {
                for part in items {
                    if let Some(content) = part["content"].as_str() {
                        parts.push(format!(
                            "[{}] {}",
                            timestamp(part["timestamp"].as_f64().unwrap_or(0.0)),
                            content
                        ));
                    }
                }
            }
        }
    }
    if parts.is_empty() {
        return Err(AppError::msg("该分 P 的 AI 总结为空"));
    }
    Ok(parts.join("\n\n"))
}

async fn read_subtitles(client: &Client, value: &Value, cid: u64) -> AppResult<(String, String)> {
    let value = data(value)?;
    if value["cid"].as_u64().is_some_and(|actual| actual != cid) {
        return Err(AppError::msg("返回的字幕不属于所选分 P"));
    }
    let mut tracks: Vec<_> = value["subtitle"]["subtitles"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    tracks.sort_by_key(|t| match t["lan"].as_str().unwrap_or("") {
        "zh-CN" | "zh-Hans" => 0,
        "zh-TW" | "zh-Hant" => 1,
        "ai-zh" => 2,
        _ => 3,
    });
    let mut failure = "没有可读取的字幕；部分字幕需要登录 B 站".to_string();
    for track in tracks.into_iter().take(5) {
        let result = async {
            let url = subtitle_url(track["subtitle_url"].as_str().unwrap_or(""))?;
            let text = subtitle_text(&request(client, url.as_str(), None).await?)?;
            Ok::<_, AppError>((
                text,
                track["lan_doc"].as_str().unwrap_or("字幕").to_string(),
            ))
        }
        .await;
        match result {
            Ok(result) => return Ok(result),
            Err(error) => failure = error.to_string(),
        }
    }
    Err(AppError::msg(failure))
}

pub async fn fetch_text(state: &BilibiliState, input: &str) -> AppResult<VideoText> {
    tokio::time::timeout(Duration::from_secs(70), fetch_inner(state, input))
        .await
        .map_err(|_| AppError::msg("获取网课文字超时，可稍后重试或手动粘贴"))?
}

async fn fetch_inner(state: &BilibiliState, input: &str) -> AppResult<VideoText> {
    let client = client()?;
    let url = resolve_video(&client, input).await?;
    let (kind, id, page) = video_id(&url)?;
    let cookie = cookie(state).await?;
    let cookie = cookie.as_deref();
    let view = request(
        &client,
        &format!("{API}/x/web-interface/view?{kind}={id}"),
        cookie,
    )
    .await?;
    let view = data(&view)?;
    let selected = view["pages"]
        .as_array()
        .and_then(|p| p.iter().find(|p| p["page"].as_u64() == Some(page)))
        .ok_or_else(|| AppError::invalid("视频没有这个分 P，请检查链接中的 p 参数"))?;
    let cid = selected["cid"]
        .as_u64()
        .filter(|v| *v > 0)
        .ok_or_else(|| AppError::msg("无法识别视频分 P"))?;
    let bvid = view["bvid"]
        .as_str()
        .ok_or_else(|| AppError::msg("无法识别视频编号"))?;
    let url = format!("https://www.bilibili.com/video/{bvid}/?p={page}");
    let title = if view["pages"].as_array().is_some_and(|p| p.len() > 1) {
        format!(
            "{} · P{page} {}",
            view["title"].as_str().unwrap_or("B站网课"),
            selected["part"].as_str().unwrap_or("")
        )
    } else {
        view["title"].as_str().unwrap_or("B站网课").into()
    };
    let nav = request(&client, &format!("{API}/x/web-interface/nav"), cookie).await?;
    // Anonymous nav has code -101 but still supplies public WBI signing keys.
    if nav["code"] != 0 && nav["code"] != -101 {
        data(&nav)?;
    }
    let nav = &nav["data"];
    let params = BTreeMap::from([
        ("bvid".into(), bvid.into()),
        ("cid".into(), cid.to_string()),
    ]);
    let query = signed_query(params.clone(), nav, chrono::Utc::now().timestamp())?;
    let subtitles = match request(&client, &format!("{API}/x/player/wbi/v2?{query}"), cookie).await
    {
        Ok(player) => read_subtitles(&client, &player, cid).await,
        Err(error) => Err(error),
    };
    let (kind, text, notice) = match subtitles {
        Ok((text, language)) => (
            "subtitle",
            format!("{SUBTITLE_HEADER}\n语言：{language}\n来源：{url}\n\n{text}"),
            format!("已获取{language}，保存后可用于 AI 学习。自动字幕可能存在识别错误。"),
        ),
        Err(subtitle_error) => {
            let mut params = params;
            if let Some(mid) = view["owner"]["mid"].as_u64() {
                params.insert("up_mid".into(), mid.to_string());
            }
            let query = signed_query(params, nav, chrono::Utc::now().timestamp())?;
            let summary = request(
                &client,
                &format!("{API}/x/web-interface/view/conclusion/get?{query}"),
                cookie,
            )
            .await
            .and_then(|value| summary_text(&value))
            .map_err(|error| {
                AppError::msg(format!(
                    "字幕：{subtitle_error}；AI 总结：{error}。仍可保存链接或手动粘贴文字。"
                ))
            })?;
            ("summary", format!("{SUMMARY_HEADER}\n来源：{url}\n这是平台生成的摘要，可能有遗漏或错误，不代表完整视频内容。\n\n{summary}"), "未获取到字幕，已读取 B 站 AI 总结。AI 将依据摘要讲解，不能覆盖完整课程。".into())
        }
    };
    if text.chars().count() > 400_000 {
        return Err(AppError::msg("字幕超过 40 万字，请分段手动导入"));
    }
    Ok(VideoText {
        title: title.chars().take(100).collect(),
        url,
        kind: kind.into(),
        transcript: text,
        notice,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn links_keep_the_requested_part_and_reject_untrusted_hosts() {
        let url = video_url("https://www.bilibili.com/video/BV1L94y1H7CV/?p=3&share_source=copy")
            .unwrap();
        assert_eq!(video_id(&url).unwrap(), ("bvid", "BV1L94y1H7CV".into(), 3));
        assert_eq!(
            video_id(&video_url("http://m.bilibili.com/video/av123").unwrap()).unwrap(),
            ("aid", "123".into(), 1)
        );
        for input in [
            "https://bilibili.com.evil.test/video/BV1L94y1H7CV",
            "https://user@b23.tv/test",
            "https://localhost/video/av123",
            "https://b23.tv:8443/test",
        ] {
            assert!(video_url(input).is_err());
        }
        assert!(video_id(&video_url("https://www.bilibili.com/video/av123?p=0").unwrap()).is_err());
        assert!(subtitle_url("//aisubtitle.hdslb.com/bfs/subtitle/test.json").is_ok());
        assert!(subtitle_url("https://aisubtitle.hdslb.com.evil.test/test").is_err());
        assert!(subtitle_url("http://127.0.0.1/test").is_err());
    }

    #[test]
    fn wbi_matches_the_public_reference_vector() {
        let nav = json!({"wbi_img": {
            "img_url": "https://i0.hdslb.com/bfs/wbi/7cd084941338484aae1ad9425b84077c.png",
            "sub_url": "https://i0.hdslb.com/bfs/wbi/4932caff0ff746eab6f01bf08b70ac45.png"
        }});
        let params = BTreeMap::from([
            ("foo".into(), "114".into()),
            ("bar".into(), "514".into()),
            ("zab".into(), "1919810".into()),
        ]);
        assert_eq!(
            signed_query(params, &nav, 1702204169).unwrap(),
            "bar=514&foo=114&wts=1702204169&zab=1919810&w_rid=8f6f2b5b3d485fe1886cec6a0be8c5d4"
        );
        assert!(signed_query(BTreeMap::new(), &json!({}), 0).is_err());
    }

    #[test]
    fn text_decoding_preserves_timestamps_and_rejects_unavailable_summaries() {
        let text = subtitle_text(
            &json!({"body": [{"from": 62.2, "content": "导数"}, {"from": 70, "content": " "}]}),
        )
        .unwrap();
        assert_eq!(text, "[00:01:02] 导数");
        assert!(subtitle_text(&json!({"body": []})).is_err());
        let summary = summary_text(&json!({"code": 0, "data": {"code": 0, "model_result": {
            "summary": "导数的几何意义", "outline": [{"title": "切线", "timestamp": 60, "part_outline": [{"timestamp": 90, "content": "斜率"}]}]
        }}})).unwrap();
        assert!(summary.contains("导数的几何意义") && summary.contains("[00:01:30] 斜率"));
        assert!(summary_text(&json!({"code": 0, "data": {"code": 1}})).is_err());
        assert!(summary_text(&json!({"code": -101}))
            .unwrap_err()
            .to_string()
            .contains("登录"));
    }

    #[test]
    fn cookies_preserve_encoding_and_ignore_unrelated_headers() {
        let mut headers = header::HeaderMap::new();
        headers.append(
            header::SET_COOKIE,
            "SESSDATA=test%2Cvalue; Path=/; HttpOnly".parse().unwrap(),
        );
        headers.append(
            header::SET_COOKIE,
            "DedeUserID=123; Path=/".parse().unwrap(),
        );
        headers.append(
            header::SET_COOKIE,
            "irrelevant=not-saved; Path=/".parse().unwrap(),
        );
        assert_eq!(
            login_cookies(&headers).unwrap(),
            "DedeUserID=123; SESSDATA=test%2Cvalue"
        );
        assert!(login_cookies(&header::HeaderMap::new()).is_err());
    }

    #[tokio::test]
    async fn cancelled_login_is_not_polled_and_forget_clears_credentials() {
        let secrets = Arc::new(crate::security::MemoryStore::default());
        secrets.set(SECRET, "SESSDATA=test").unwrap();
        let state = BilibiliState {
            secrets: secrets.clone(),
            ..Default::default()
        };
        *state.pending.lock().await = Some(("test-key".into(), Instant::now()));
        cancel_login(&state, "test-key".into()).await;
        assert_eq!(
            poll_login(&state, "test-key".into()).await.unwrap().status,
            "expired"
        );
        forget_login(&state).await.unwrap();
        assert!(secrets.get(SECRET).unwrap().is_none());
    }
}
