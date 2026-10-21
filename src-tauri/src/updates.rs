//! GitHub Release discovery and explicitly requested, signature-verified updates.
use crate::error::{AppError, AppResult};
use reqwest::{Client, StatusCode};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{ipc::Channel, AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

#[derive(Default)]
pub struct UpdateState(tokio::sync::Mutex<Option<(tauri_plugin_updater::Update, Vec<u8>)>>);

#[derive(Clone, Serialize)]
pub struct DownloadProgress {
    downloaded: u64,
    total: Option<u64>,
}

/// Re-discover the release on the backend; never accept a download URL from the UI.
#[tauri::command]
pub async fn download_update(app: AppHandle, version: String, on_progress: Channel<DownloadProgress>) -> AppResult<()> {
    if !cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        return Err(AppError::msg("应用内更新目前仅支持 Windows x64"));
    }
    let state = app.state::<UpdateState>();
    let mut ready = state.0.try_lock().map_err(|_| AppError::msg("更新正在处理中"))?;
    if ready.as_ref().is_some_and(|(update, _)| update.version == version) { return Ok(()); }
    *ready = None;
    let release = check(&app.package_info().version).await?
        .ok_or_else(|| AppError::msg("没有可下载的新版本，请重新检查更新"))?;
    if release.version != version {
        return Err(AppError::msg("可用版本已变化，请重新检查更新"));
    }
    // Pin the manifest to the selected release rather than a mutable /latest URL.
    let manifest = format!("{}/latest.json", release.release_url.replace("/releases/tag/", "/releases/download/"));
    let updater = app.updater_builder()
        .endpoints(vec![url::Url::parse(&manifest).map_err(|_| AppError::msg("更新地址无效"))?])
        .map_err(|e| AppError::msg(e.to_string()))?
        .timeout(Duration::from_secs(15))
        .build().map_err(|e| AppError::msg(e.to_string()))?;
    let mut update = updater.check().await
        .map_err(|e| AppError::msg(format!("无法获取签名更新包，可重试或手动下载：{e}")))?
        .ok_or_else(|| AppError::msg("该版本暂无应用内更新包，请手动下载"))?;
    if update.version != version {
        return Err(AppError::msg("更新清单版本不匹配，请稍后重试"));
    }
    // Large OCR bundles need a longer download timeout than the manifest request.
    update.timeout = Some(Duration::from_secs(3600));
    let mut downloaded = 0_u64;
    let mut last_event = std::time::Instant::now() - Duration::from_secs(1);
    let bytes = update.download(|chunk, total| {
        downloaded += chunk as u64;
        if last_event.elapsed() >= Duration::from_millis(200) {
            let _ = on_progress.send(DownloadProgress { downloaded, total });
            last_event = std::time::Instant::now();
        }
    }, || {}).await.map_err(|e| AppError::msg(format!("下载或签名校验失败，可重试或手动下载：{e}")))?;
    // download() returns only after signature verification succeeds.
    let _ = on_progress.send(DownloadProgress { downloaded: bytes.len() as u64, total: Some(bytes.len() as u64) });
    *ready = Some((update, bytes));
    Ok(())
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<UpdateState>();
        let ready = state.0.try_lock().map_err(|_| AppError::msg("更新正在处理中"))?;
        let (update, bytes) = ready.as_ref().ok_or_else(|| AppError::msg("请先下载并校验更新包"))?;
        let tasks = app.state::<crate::documents::DocumentTasks>();
        // Keep these locks through installer launch so the queue cannot start a new parser.
        let busy = || AppError::msg("资料正在处理，请等待处理完成后再安装更新");
        let _worker = tasks.1.try_lock().map_err(|_| busy())?;
        let _mutation = tasks.0.try_lock().map_err(|_| busy())?;
        let _registration = tasks.2.try_lock().map_err(|_| busy())?;
        update.install(bytes).map_err(|e| AppError::msg(format!("启动安装失败，可重试或手动下载：{e}")))
    }).await?
}

const RELEASES_API: &str = "https://api.github.com/repos/zengxiaofeng233/ExamOps/releases";
const RELEASE_PAGE: &str = "https://github.com/zengxiaofeng233/ExamOps/releases/tag/";

#[derive(Debug, Clone, Serialize)]
pub struct AvailableUpdate {
    pub current_version: String,
    pub version: String,
    pub release_url: String,
    pub asset_name: String,
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    state: String,
    size: u64,
}

fn candidate(release: Release, current: &Version) -> Option<(Version, AvailableUpdate)> {
    if release.draft || release.prerelease {
        return None;
    }
    let tag = release.tag_name.trim();
    let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()?;
    if !version.pre.is_empty() || !version.cmp_precedence(current).is_gt() {
        return None;
    }
    // Match the NSIS filename produced by this repository, including its version.
    let asset_name = format!("ExamOps_{version}_x64-setup.exe");
    if !release.assets.iter().any(|asset| {
        asset.name == asset_name && asset.state == "uploaded" && asset.size > 0
    }) {
        return None;
    }
    let mut page = url::Url::parse(RELEASE_PAGE).ok()?;
    page.path_segments_mut().ok()?.pop_if_empty().push(tag);
    Some((version.clone(), AvailableUpdate {
        current_version: current.to_string(),
        version: version.to_string(),
        release_url: page.to_string(),
        asset_name,
    }))
}

fn network_error(error: reqwest::Error) -> AppError {
    if error.is_timeout() {
        AppError::msg("检查更新超时，请稍后重试")
    } else {
        AppError::msg("无法连接 GitHub，请检查网络后重试")
    }
}

async fn fetch_updates(client: &Client, endpoint: &str, current: &Version) -> AppResult<Option<AvailableUpdate>> {
    let mut page = 1_u32;
    let mut newest: Option<(Version, AvailableUpdate)> = None;
    loop {
        let response = client.get(endpoint)
            .query(&[("per_page", 100_u32), ("page", page)])
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send().await.map_err(network_error)?;
        match response.status() {
            StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS => {
                return Err(AppError::msg("GitHub 暂时拒绝请求或访问频率受限，请稍后重试"));
            }
            StatusCode::NOT_FOUND => return Err(AppError::msg("无法访问 ExamOps 的 GitHub Release，请稍后重试")),
            status if !status.is_success() => {
                return Err(AppError::msg(format!("GitHub 更新检查失败（HTTP {}），请稍后重试", status.as_u16())));
            }
            _ => {}
        }
        let releases: Vec<Release> = response.json().await
            .map_err(|error| if error.is_timeout() {
                network_error(error)
            } else {
                AppError::msg("GitHub 返回了无法识别的版本信息，请稍后重试")
            })?;
        let count = releases.len();
        for release in releases {
            if let Some(update) = candidate(release, current) {
                if newest.as_ref().is_none_or(|(version, _)| update.0.cmp_precedence(version).is_gt()) {
                    newest = Some(update);
                }
            }
        }
        if count < 100 {
            return Ok(newest.map(|(_, update)| update));
        }
        page += 1;
    }
}

async fn check_with_client(client: &Client, endpoint: &str, current: &Version, timeout: Duration) -> AppResult<Option<AvailableUpdate>> {
    tokio::time::timeout(timeout, fetch_updates(client, endpoint, current)).await
        .map_err(|_| AppError::msg("检查更新超时，请稍后重试"))?
}

pub async fn check(current: &Version) -> AppResult<Option<AvailableUpdate>> {
    if !cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        return Err(AppError::msg("目前仅支持检查 Windows x64 安装包更新"));
    }
    let client = Client::builder()
        .user_agent(format!("ExamOps/{current}"))
        .timeout(Duration::from_secs(10))
        .build().map_err(|_| AppError::msg("无法初始化更新检查"))?;
    check_with_client(&client, RELEASES_API, current, Duration::from_secs(15)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use serde_json::{json, Value};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // Public test vector from tauri-plugin-updater: payload "test", signed for 2.0.0.
    // These are a disposable public key and signature, never a production private key.
    const TEST_PUBLIC_KEY: &str = "untrusted comment: minisign public key: A49EF4746FA6E6D7\nRWTX5qZvdPSepDGWg2bz8n6ArQnBMwgQK1geVosorcbLzcunD+KJ+cYe";
    const TEST_SIGNATURE: &str = "untrusted comment: signature from tauri secret key\nRUTX5qZvdPSepPF2zsjvP/L/TQtBLRnWRvaq+4omK1UEugtQp/USrGiI1v762sDy3id3aor7bRC9ETyeot6lMHHrWoZ5hjUM1Qc=\ntrusted comment: timestamp:1791649051\tfile:payload\tversion:2.0.0\nDDq2CLOEl5cQmC2eJRHlnZPVViOZsu1CjpPKQoei4A/XBfkDqGO0B+QMrcrHgEQ5qV3mne75tdFWhJpqUXh9AA==";

    fn signed_update(version: &str, url: &str) -> tauri_plugin_updater::Update {
        let encode = |value| base64::engine::general_purpose::STANDARD.encode(value);
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        let config: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let mut plugin_config = config["plugins"]["updater"].clone();
        plugin_config["pubkey"] = json!(encode(TEST_PUBLIC_KEY));
        context.config_mut().plugins.0.insert("updater".into(), plugin_config);
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_updater::Builder::new().build())
            .build(context).unwrap();
        app.updater_builder().target("windows-x86_64").no_proxy()
            .endpoints(vec!["https://example.invalid/latest.json".parse().unwrap()]).unwrap()
            .executable_path("test-runtime/app.exe").build().unwrap()
            .restore_update(json!({"version": version, "platforms": {"windows-x86_64": {
                "url": url, "signature": encode(TEST_SIGNATURE)
            }}})).unwrap().unwrap()
    }

    #[tokio::test]
    async fn signed_download_accepts_valid_bytes_and_rejects_tampering() {
        for (payload, valid) in [("test", true), ("tampered", false)] {
            let (endpoint, task) = server(vec![(200, payload.into())]).await;
            let update = signed_update("2.0.0", &endpoint);
            let mut received = 0;
            let result = update.download(|chunk, _| received += chunk, || {}).await;
            assert_eq!(result.is_ok(), valid);
            assert_eq!(received, payload.len());
            if valid { assert_eq!(result.unwrap(), b"test"); }
            task.await.unwrap();
        }
    }

    #[test]
    fn signature_binds_version_and_install_rejects_invalid_bytes_before_launch() {
        let update = signed_update("2.0.0", "https://example.invalid/update.exe");
        assert!(update.verify(b"test").is_ok());
        assert!(update.install(b"tampered").is_err());
        let changed_version = signed_update("9.0.0", "https://example.invalid/update.exe");
        assert!(changed_version.verify(b"test").is_err());
    }

    fn release(tag: &str) -> Value {
        json!({"tag_name":tag,"draft":false,"prerelease":false,"assets":[{
            "name":format!("ExamOps_{}_x64-setup.exe",tag.trim_start_matches('v')),
            "state":"uploaded","size":1024
        }]})
    }

    fn select(value: Value, current: &str) -> Option<(Version, AvailableUpdate)> {
        candidate(serde_json::from_value(value).unwrap(), &Version::parse(current).unwrap())
    }

    #[test]
    fn compares_versions_numerically_and_ignores_build_metadata() {
        assert!(select(release("v0.10.0"), "0.9.0").is_some());
        assert!(select(release("0.2.0"), "0.1.0").is_some());
        for tag in ["v0.1.0", "v0.0.9", "v0.1.0+rebuild", "nightly", "v1.0.0-beta.1"] {
            assert!(select(release(tag), "0.1.0").is_none(), "{tag}");
        }
    }

    #[test]
    fn requires_a_published_stable_release_and_a_complete_matching_installer() {
        for field in ["draft", "prerelease"] {
            let mut r = release("v0.2.0");
            r[field] = json!(true);
            assert!(select(r, "0.1.0").is_none());
        }
        for name in ["source.zip", "ExamOps_0.2.0_arm64-setup.exe", "ExamOps_0.1.0_x64-setup.exe"] {
            let mut r = release("v0.2.0");
            r["assets"][0]["name"] = json!(name);
            assert!(select(r, "0.1.0").is_none());
        }
        let mut r = release("v0.2.0");
        r["assets"][0]["state"] = json!("starter");
        assert!(select(r.clone(), "0.1.0").is_none());
        r["assets"][0]["state"] = json!("uploaded");
        r["assets"][0]["size"] = json!(0);
        assert!(select(r.clone(), "0.1.0").is_none());
        r["assets"] = json!([]);
        assert!(select(r, "0.1.0").is_none());
        let (_, update) = select(release("v0.2.0"), "0.1.0").unwrap();
        assert_eq!(update.release_url, format!("{RELEASE_PAGE}v0.2.0"));
    }

    async fn server(responses: Vec<(u16, String)>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/releases", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 1024];
                while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                    let count = stream.read(&mut buffer).await.unwrap();
                    if count == 0 { break; }
                    request.extend_from_slice(&buffer[..count]);
                }
                requests.push(String::from_utf8(request).unwrap());
                let reply = format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                stream.write_all(reply.as_bytes()).await.unwrap();
            }
            requests
        });
        (endpoint, task)
    }

    fn client() -> Client {
        Client::builder().no_proxy().user_agent("ExamOps/0.1.0").build().unwrap()
    }

    #[tokio::test]
    async fn paginates_and_selects_the_highest_version_with_an_installer() {
        let mut first = vec![release("v0.1.0"); 100];
        first[0] = release("v0.3.0");
        first[1] = release("v9.0.0");
        first[1]["assets"] = json!([]);
        let (endpoint, task) = server(vec![(200, json!(first).to_string()), (200, json!([release("v0.10.0"),release("v0.2.0")]).to_string())]).await;
        let update = fetch_updates(&client(), &endpoint, &Version::new(0, 1, 0)).await.unwrap().unwrap();
        assert_eq!(update.version, "0.10.0");
        let requests = task.await.unwrap();
        assert!(requests[0].starts_with("GET /releases?per_page=100&page=1 "));
        assert!(requests[1].starts_with("GET /releases?per_page=100&page=2 "));
        assert!(requests[0].to_lowercase().contains("user-agent: examops/0.1.0"));
        assert!(!requests[0].to_lowercase().contains("authorization:"));
    }

    #[tokio::test]
    async fn empty_releases_are_not_an_error_but_http_and_invalid_data_are() {
        let (endpoint, task) = server(vec![(200, "[]".into())]).await;
        assert!(fetch_updates(&client(), &endpoint, &Version::new(0, 1, 0)).await.unwrap().is_none());
        task.await.unwrap();
        for (status, body, expected) in [(403,"{}","访问频率"),(429,"{}","访问频率"),(404,"{}","无法访问"),(500,"{}","HTTP 500"),(200,"not json","无法识别")] {
            let (endpoint, task) = server(vec![(status, body.into())]).await;
            let error = fetch_updates(&client(), &endpoint, &Version::new(0, 1, 0)).await.unwrap_err();
            assert!(error.to_string().contains(expected), "{error}");
            task.await.unwrap();
        }
    }

    #[tokio::test]
    async fn offline_and_silent_servers_fail_without_hanging() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/releases", listener.local_addr().unwrap());
        let current = Version::new(0, 1, 0);
        let error = check_with_client(&client(), &endpoint, &current, Duration::from_millis(100)).await.unwrap_err();
        assert!(error.to_string().contains("超时"));
        drop(listener);
        let error = check_with_client(&client(), &endpoint, &current, Duration::from_secs(5)).await.unwrap_err();
        assert!(error.to_string().contains("无法连接"));
    }
}
