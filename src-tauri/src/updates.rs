//! Read-only GitHub Release discovery. Downloads and installation remain manual.
use crate::error::{AppError, AppResult};
use reqwest::{Client, StatusCode};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::time::Duration;

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
    use serde_json::{json, Value};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
