use crate::bilibili::{self, BilibiliState, LoginPoll, LoginQr, Session, VideoText};
use crate::error::AppResult;
use tauri::State;

/// 检查本机保存的 B 站登录是否有效，不返回凭据。
#[tauri::command]
pub async fn get_bilibili_session(state: State<'_, BilibiliState>) -> AppResult<Session> {
    bilibili::session(state.inner()).await
}

/// 创建 B 站登录二维码（在本机渲染）。
#[tauri::command]
pub async fn start_bilibili_login(state: State<'_, BilibiliState>) -> AppResult<LoginQr> {
    bilibili::start_login(state.inner()).await
}

/// 检查扫码状态，成功后将凭据保存到系统凭据管理器。
#[tauri::command]
pub async fn poll_bilibili_login(
    state: State<'_, BilibiliState>,
    key: String,
) -> AppResult<LoginPoll> {
    bilibili::poll_login(state.inner(), key).await
}

/// 取消当前二维码，防止已关闭的登录请求保存凭据。
#[tauri::command]
pub async fn cancel_bilibili_login(state: State<'_, BilibiliState>, key: String) -> AppResult<()> {
    bilibili::cancel_login(state.inner(), key).await;
    Ok(())
}

/// 清除本应用保存的 B 站登录凭据（不影响浏览器登录）。
#[tauri::command]
pub async fn forget_bilibili_login(state: State<'_, BilibiliState>) -> AppResult<()> {
    bilibili::forget_login(state.inner()).await
}

/// 读取链接所指分 P 的字幕，失败时尝试 AI 总结；不修改网课记录。
#[tauri::command]
pub async fn fetch_bilibili_text(
    state: State<'_, BilibiliState>,
    url: String,
) -> AppResult<VideoText> {
    bilibili::fetch_text(state.inner(), &url).await
}
