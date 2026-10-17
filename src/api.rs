use crate::{
    auth::{AuthMethod, Session},
    error::{Error, Result},
    models::{DriveAbout, OfflineTask, OfflineTaskPage, OfflineTaskResponse, Share, SharePage},
};
use reqwest::{Client, Method};
use secrecy::ExposeSecret;
use serde::{Serialize, de::DeserializeOwned};
use std::time::Duration;
#[derive(Clone)]
pub struct Api {
    http: Client,
    drive: String,
    user: String,
}
impl Api {
    pub fn new() -> Result<Self> {
        Self::with_endpoints(
            "https://api-drive.mypikpak.com",
            "https://user.mypikpak.com",
        )
    }
    // Non-production endpoints are restricted to loopback for Mock tests.
    pub fn for_mock(base: &str) -> Result<Self> {
        let url = reqwest::Url::parse(base).map_err(|_| Error::Invalid("mock URL"))?;
        if !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")) {
            return Err(Error::Invalid("mock endpoint must be loopback"));
        }
        Self::with_endpoints(base, base)
    }
    fn with_endpoints(drive: &str, user: &str) -> Result<Self> {
        Ok(Self {
            http: Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("Mokawa/0.1.0")
                .build()?,
            drive: drive.into(),
            user: user.into(),
        })
    }
    pub(crate) async fn user_request<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T> {
        Self::decode(
            self.http
                .post(format!("{}{path}", self.user))
                .json(body)
                .send()
                .await?,
            false,
        )
        .await
    }
    pub(crate) async fn drive_request<T: DeserializeOwned>(
        &self,
        session: &Session,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        if session.expired() {
            return Err(Error::Unauthorized);
        }
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.drive))
            .bearer_auth(session.access_token.expose_secret())
            .query(query);
        if session.auth_method == AuthMethod::OAuth {
            request = request
                .header("x-device-id", &session.device_id)
                .header("x-captcha-token", session.captcha_token.expose_secret());
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        Self::decode(
            request.send().await?,
            session.auth_method == AuthMethod::PersonalAccessToken,
        )
        .await
    }

    /// Verify a user-supplied PAT against the real PikPak Drive API with the
    /// least-effect request currently needed by Mokawa: read one root entry.
    pub async fn verify_personal_access_token(&self, session: &Session) -> Result<()> {
        if session.auth_method != AuthMethod::PersonalAccessToken {
            return Err(Error::Invalid("认证类型"));
        }
        let _: serde_json::Value = self
            .drive_request(
                session,
                Method::GET,
                "/drive/v1/files",
                &[
                    ("parent_id", String::new()),
                    ("limit", "1".into()),
                    (
                        "filters",
                        serde_json::json!({"trashed":{"eq":false}}).to_string(),
                    ),
                ],
                None,
            )
            .await?;
        Ok(())
    }

    /// Read the server-reported storage quota. Never infer this from listed files.
    pub async fn drive_about(&self, session: &Session) -> Result<DriveAbout> {
        self.drive_request(session, Method::GET, "/drive/v1/about", &[], None)
            .await
    }

    pub async fn create_offline_task(
        &self,
        session: &Session,
        source: &str,
        parent_id: &str,
    ) -> Result<OfflineTask> {
        let source = validate_offline_source(source)?;
        if !parent_id.is_empty() {
            crate::files::validate_id(parent_id)?;
        }
        let mut body = serde_json::json!({
            "kind": "drive#file",
            "upload_type": "UPLOAD_TYPE_URL",
            "url": { "url": source },
        });
        if parent_id.is_empty() {
            body["folder_type"] = serde_json::json!("DOWNLOAD");
        } else {
            body["parent_id"] = serde_json::json!(parent_id);
            body["folder_type"] = serde_json::json!("");
        }
        let response: OfflineTaskResponse = self
            .drive_request(session, Method::POST, "/drive/v1/files", &[], Some(body))
            .await?;
        response
            .task
            .filter(|task| !task.id.is_empty())
            .ok_or(Error::Protocol)
    }

    pub async fn offline_tasks(&self, session: &Session) -> Result<Vec<OfflineTask>> {
        const PAGE_SIZE: &str = "100";
        const MAX_TASKS: usize = 5_000;
        let mut query = vec![
            ("type", "offline".into()),
            ("thumbnail_size", "SIZE_SMALL".into()),
            ("limit", PAGE_SIZE.into()),
            (
                "filters",
                serde_json::json!({"phase":{"in":"PHASE_TYPE_RUNNING,PHASE_TYPE_PENDING,PHASE_TYPE_COMPLETE,PHASE_TYPE_ERROR,PHASE_TYPE_PAUSED,PHASE_TYPE_UNKNOW"}}).to_string(),
            ),
            ("with", "reference_resource".into()),
        ];
        let mut tasks = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..50 {
            let page: OfflineTaskPage = self
                .drive_request(session, Method::GET, "/drive/v1/tasks", &query, None)
                .await?;
            if tasks.len().saturating_add(page.tasks.len()) > MAX_TASKS {
                return Err(Error::Invalid("离线任务过多，请稍后再试"));
            }
            tasks.extend(page.tasks);
            if page.next_page_token.is_empty() {
                return Ok(tasks);
            }
            if page.next_page_token.len() > 4096 || !seen.insert(page.next_page_token.clone()) {
                return Err(Error::Protocol);
            }
            query.retain(|(key, _)| *key != "page_token");
            query.push(("page_token", page.next_page_token));
        }
        Err(Error::Protocol)
    }

    /// Return the current account's starred files. PikPak exposes favorites through
    /// the `system_tag=STAR` file filter, not a local-only Mokawa collection.
    pub async fn list_favorites(&self, session: &Session) -> Result<Vec<crate::models::File>> {
        let mut query = vec![
            ("parent_id", "*".into()),
            ("limit", "100".into()),
            (
                "filters",
                serde_json::json!({
                    "trashed":{"eq":false},
                    "phase":{"eq":"PHASE_TYPE_COMPLETE"},
                    "system_tag":{"in":"STAR"}
                })
                .to_string(),
            ),
            ("thumbnail_size", "SIZE_SMALL".into()),
            ("with_audit", "true".into()),
        ];
        let mut files = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..1000 {
            let page: crate::models::FilePage = self
                .drive_request(session, Method::GET, "/drive/v1/files", &query, None)
                .await?;
            if files.len().saturating_add(page.files.len()) > 100_000 {
                return Err(Error::Invalid("收藏条目过多，请稍后重试"));
            }
            files.extend(page.files);
            if page.next_page_token.is_empty() {
                for file in &mut files {
                    file.starred = true;
                }
                return Ok(files);
            }
            if page.next_page_token.len() > 4096 || !seen.insert(page.next_page_token.clone()) {
                return Err(Error::Protocol);
            }
            query.retain(|(key, _)| *key != "page_token");
            query.push(("page_token", page.next_page_token));
        }
        Err(Error::Protocol)
    }

    pub async fn set_favorite(&self, session: &Session, id: &str, starred: bool) -> Result<()> {
        crate::files::validate_id(id)?;
        let action = if starred { "star" } else { "unstar" };
        let _: serde_json::Value = self
            .drive_request(
                session,
                Method::POST,
                &format!("/drive/v1/files:{action}"),
                &[],
                Some(serde_json::json!({"ids":[id]})),
            )
            .await?;
        Ok(())
    }

    pub async fn restore_files(&self, session: &Session, ids: &[String]) -> Result<()> {
        validate_ids(ids, 100, "请选择 1–100 个文件")?;
        let _: serde_json::Value = self
            .drive_request(
                session,
                Method::POST,
                "/drive/v1/files:batchUntrash",
                &[],
                Some(serde_json::json!({"ids":ids})),
            )
            .await?;
        Ok(())
    }

    pub async fn permanently_delete(&self, session: &Session, ids: &[String]) -> Result<()> {
        validate_ids(ids, 100, "请选择 1–100 个文件")?;
        let _: serde_json::Value = self
            .drive_request(
                session,
                Method::POST,
                "/drive/v1/files:batchDelete",
                &[],
                Some(serde_json::json!({"ids":ids})),
            )
            .await?;
        Ok(())
    }

    pub async fn list_shares(&self, session: &Session) -> Result<Vec<Share>> {
        let mut query = vec![
            ("limit", "100".into()),
            ("thumbnail_size", "SIZE_SMALL".into()),
        ];
        let mut shares = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..100 {
            let page: SharePage = self
                .drive_request(session, Method::GET, "/drive/v1/share/list", &query, None)
                .await?;
            if shares.len().saturating_add(page.data.len()) > 10_000 {
                return Err(Error::Invalid("分享条目过多，请稍后重试"));
            }
            shares.extend(page.data);
            if page
                .next_page_token
                .as_deref()
                .unwrap_or_default()
                .is_empty()
            {
                return Ok(shares);
            }
            let Some(next_page_token) = page.next_page_token else {
                return Ok(shares);
            };
            if next_page_token.len() > 4096 || !seen.insert(next_page_token.clone()) {
                return Err(Error::Protocol);
            }
            query.retain(|(key, _)| *key != "page_token");
            query.push(("page_token", next_page_token));
        }
        Err(Error::Protocol)
    }

    pub async fn create_share(
        &self,
        session: &Session,
        ids: &[String],
        expiration_days: u32,
        require_pass_code: bool,
    ) -> Result<Share> {
        validate_ids(ids, 100, "请选择 1–100 个文件")?;
        if !matches!(expiration_days, 1 | 7 | 30) {
            return Err(Error::Invalid("分享有效期"));
        }
        let value: serde_json::Value = self
            .drive_request(
                session,
                Method::POST,
                "/drive/v1/share",
                &[],
                Some(serde_json::json!({
                    "file_ids":ids,
                    "share_to": if require_pass_code { "encryptedlink" } else { "publiclink" },
                    "expiration_days": expiration_days,
                    "pass_code_option": if require_pass_code { "REQUIRED" } else { "NOT_REQUIRED" }
                })),
            )
            .await?;
        let share = value.get("share").cloned().unwrap_or(value);
        serde_json::from_value(share).map_err(|_| Error::Protocol)
    }

    pub async fn delete_shares(&self, session: &Session, ids: &[String]) -> Result<()> {
        validate_ids(ids, 100, "请选择 1–100 个分享")?;
        let _: serde_json::Value = self
            .drive_request(
                session,
                Method::POST,
                "/drive/v1/share:batchDelete",
                &[],
                Some(serde_json::json!({"ids":ids})),
            )
            .await?;
        Ok(())
    }

    pub async fn retry_offline_task(&self, session: &Session, id: &str) -> Result<()> {
        crate::files::validate_id(id)?;
        let task = self.offline_task(session, id).await?;
        if !task.phase.eq_ignore_ascii_case("PHASE_TYPE_ERROR")
            && !task.phase.eq_ignore_ascii_case("ERROR")
            && !task.phase.eq_ignore_ascii_case("FAILED")
        {
            return Err(Error::Invalid("仅失败的离线任务可以重试"));
        }
        self.drive_empty_mutation(
            session,
            Method::POST,
            "/drive/v1/task",
            &[],
            Some(serde_json::json!({"type":"offline","create_type":"RETRY","id":id})),
        )
        .await
    }

    pub async fn offline_task(&self, session: &Session, id: &str) -> Result<OfflineTask> {
        crate::files::validate_id(id)?;
        let path = format!("/drive/v1/tasks/{id}");
        let value: serde_json::Value = self
            .drive_request(
                session,
                Method::GET,
                &path,
                &[("type", "offline".into()), ("checkPhase", "true".into())],
                None,
            )
            .await?;
        let task = value.get("task").cloned().unwrap_or(value);
        serde_json::from_value(task).map_err(|_| Error::Protocol)
    }

    pub async fn delete_offline_tasks(&self, session: &Session, ids: &[String]) -> Result<()> {
        if ids.is_empty() || ids.len() > 100 {
            return Err(Error::Invalid("请选择 1–100 个离线任务"));
        }
        for id in ids {
            crate::files::validate_id(id)?;
        }
        let query = vec![
            ("task_ids", ids.join(",")),
            ("delete_files", "false".into()),
        ];
        self.drive_empty_mutation(session, Method::DELETE, "/drive/v1/tasks", &query, None)
            .await
    }

    async fn drive_empty_mutation(
        &self,
        session: &Session,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<serde_json::Value>,
    ) -> Result<()> {
        if session.expired() {
            return Err(Error::Unauthorized);
        }
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.drive))
            .bearer_auth(session.access_token.expose_secret())
            .query(query);
        if session.auth_method == AuthMethod::OAuth {
            request = request
                .header("x-device-id", &session.device_id)
                .header("x-captcha-token", session.captcha_token.expose_secret());
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await?;
        match response.status().as_u16() {
            401 => return Err(Error::Unauthorized),
            429 => return Err(Error::RateLimited),
            403 if session.auth_method == AuthMethod::PersonalAccessToken => {
                return Err(Error::UnsupportedPermission);
            }
            403 => return Err(Error::AccessDenied),
            200..=299 => {}
            status => return Err(Error::Http(status)),
        }
        if response.content_length() == Some(0) {
            return Ok(());
        }
        let _: serde_json::Value = Self::decode(
            response,
            session.auth_method == AuthMethod::PersonalAccessToken,
        )
        .await?;
        Ok(())
    }

    /// Resolve the current signed media/download URL from PikPak metadata.
    pub async fn file_content_link(&self, session: &Session, id: &str) -> Result<String> {
        let file = self.file_details(session, id).await?;
        let raw_url = file.preferred_content_link()?;
        let url = reqwest::Url::parse(raw_url).map_err(|_| Error::Protocol)?;
        if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
            return Err(Error::Protocol);
        }
        Ok(url.to_string())
    }

    /// Resolve a playback rendition separately from the original FETCH URL
    /// used for local downloads.
    pub async fn file_playback_link(&self, session: &Session, id: &str) -> Result<String> {
        let file = self.playback_details(session, id).await?;
        let raw_url = file.preferred_playback_link()?;
        let url = reqwest::Url::parse(raw_url).map_err(|_| Error::Protocol)?;
        if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
            return Err(Error::Protocol);
        }
        Ok(url.to_string())
    }

    /// Read only a small text preview from PikPak's signed content URL.
    pub async fn text_preview(&self, session: &Session, id: &str) -> Result<String> {
        const MAX_PREVIEW: usize = 512 * 1024;
        let url = self.file_content_link(session, id).await?;
        let url = text_preview_url(&url)?;
        let mut response = self.http.get(url).send().await?;
        if response.status() != reqwest::StatusCode::OK {
            return Err(Error::Http(response.status().as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|len| len > MAX_PREVIEW as u64)
        {
            return Err(Error::Invalid("文本预览超过 512 KB，请下载查看"));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if chunk.len() > MAX_PREVIEW - bytes.len() {
                return Err(Error::Invalid("文本预览超过 512 KB，请下载查看"));
            }
            bytes.extend_from_slice(&chunk);
        }
        decode_text_preview(bytes)
    }
    async fn decode<T: DeserializeOwned>(mut response: reqwest::Response, pat: bool) -> Result<T> {
        match response.status().as_u16() {
            401 => return Err(Error::Unauthorized),
            429 => return Err(Error::RateLimited),
            403 if pat => return Err(Error::UnsupportedPermission),
            403 => return Err(Error::AccessDenied),
            200..=299 => {}
            s => return Err(Error::Http(s)),
        }
        let response_status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("未提供")
            .chars()
            .take(80)
            .collect::<String>();
        // Metadata is bounded independently of streaming file downloads. Do not trust Content-Length.
        const MAX_RESPONSE: usize = 4 * 1024 * 1024;
        if response
            .content_length()
            .is_some_and(|size| size > MAX_RESPONSE as u64)
        {
            return Err(Error::Invalid("接口响应过大，请缩小请求范围"));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if chunk.len() > MAX_RESPONSE - bytes.len() {
                return Err(Error::Invalid("接口响应过大，请缩小请求范围"));
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| Error::ProtocolResponse {
                status: response_status,
                content_type: content_type.clone(),
            })?;
        if value
            .get("error_code")
            .is_some_and(|c| c.as_i64() != Some(0))
            || value
                .get("error")
                .is_some_and(|e| !e.is_null() && e.as_str() != Some(""))
        {
            return Err(Error::Api);
        }
        serde_json::from_value(value).map_err(|_| Error::ProtocolResponse {
            status: response_status,
            content_type,
        })
    }
}

fn validate_ids(ids: &[String], max: usize, message: &'static str) -> Result<()> {
    if ids.is_empty() || ids.len() > max {
        return Err(Error::Invalid(message));
    }
    for id in ids {
        crate::files::validate_id(id)?;
    }
    Ok(())
}

pub fn validate_offline_source(source: &str) -> Result<String> {
    let source = source.trim();
    if source.is_empty() || source.len() > 8192 || source.chars().any(char::is_control) {
        return Err(Error::Invalid("离线下载链接长度或内容无效"));
    }
    let url =
        reqwest::Url::parse(source).map_err(|_| Error::Invalid("请输入有效的链接或 Magnet"))?;
    match url.scheme() {
        "magnet" => {
            let valid_hash = url.query_pairs().any(|(key, value)| {
                if key != "xt" {
                    return false;
                }
                let Some(hash) = value.strip_prefix("urn:btih:") else {
                    return false;
                };
                (hash.len() == 40 && hash.bytes().all(|c| c.is_ascii_hexdigit()))
                    || (hash.len() == 32
                        && hash.bytes().all(|c| {
                            let c = c.to_ascii_uppercase();
                            c.is_ascii_digit() && matches!(c, b'2'..=b'7')
                                || c.is_ascii_uppercase() && !matches!(c, b'I' | b'L' | b'O' | b'U')
                        }))
            });
            if !valid_hash {
                return Err(Error::Invalid("Magnet 必须包含有效的 btih 标识"));
            }
        }
        "http" | "https" => {
            if url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.host_str().is_some_and(|host| {
                    host.eq_ignore_ascii_case("localhost")
                        || host.ends_with(".localhost")
                        || host.parse::<std::net::IpAddr>().is_ok()
                })
            {
                return Err(Error::Invalid(
                    "HTTP 链接必须指向公开主机且不能包含账号密码",
                ));
            }
        }
        _ => return Err(Error::Invalid("仅支持 Magnet、HTTP 或 HTTPS 链接")),
    }
    Ok(source.to_owned())
}

fn text_preview_url(raw: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(raw).map_err(|_| Error::Protocol)?;
    // Text previews are fetched by Rust (rather than the WebView); limit this
    // proxy-like operation to PikPak-owned HTTPS hosts to avoid SSRF.
    let host = url.host_str().unwrap_or_default();
    let trusted_host = host == "mypikpak.com"
        || host.ends_with(".mypikpak.com")
        || host == "pikpakdrive.com"
        || host.ends_with(".pikpakdrive.com");
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || !trusted_host
    {
        return Err(Error::Invalid(
            "此文件所在的下载域名不支持安全文本预览，请下载查看",
        ));
    }
    Ok(url)
}

fn decode_text_preview(bytes: Vec<u8>) -> Result<String> {
    const MAX_PREVIEW: usize = 512 * 1024;
    if bytes.len() > MAX_PREVIEW {
        return Err(Error::Invalid("文本预览超过 512 KB，请下载查看"));
    }
    String::from_utf8(bytes).map_err(|_| Error::Invalid("文件不是有效 UTF-8 文本，请下载查看"))
}

#[cfg(test)]
mod source_tests {
    use super::*;

    #[test]
    fn offline_sources_accept_only_http_magnet_with_btih() {
        assert!(
            validate_offline_source("magnet:?xt=urn:btih:0123456789012345678901234567890123456789")
                .is_ok()
        );
        assert!(validate_offline_source("https://example.org/file.torrent").is_ok());
        assert!(validate_offline_source("http://example.org/file.zip").is_ok());
        assert!(validate_offline_source("magnet:?xt=urn:btih:not-a-hash").is_err());
        assert!(validate_offline_source("file:///etc/passwd").is_err());
        assert!(validate_offline_source("https://localhost/private").is_err());
        assert!(validate_offline_source("https://127.0.0.1/private").is_err());
        assert!(validate_offline_source("https://user:pass@example.org/file").is_err());
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;

    #[test]
    fn text_preview_restricts_targets_and_decoded_size() {
        assert!(text_preview_url("https://cdn.mypikpak.com/file").is_ok());
        assert!(text_preview_url("http://cdn.mypikpak.com/file").is_err());
        assert!(text_preview_url("https://127.0.0.1/file").is_err());
        assert!(text_preview_url("https://attacker.example/file").is_err());
        assert_eq!(
            decode_text_preview("只读文本".as_bytes().to_vec()).unwrap(),
            "只读文本"
        );
        assert!(decode_text_preview(vec![b'a'; 512 * 1024 + 1]).is_err());
    }
}

#[cfg(test)]
mod pikpak_collection_api_tests {
    use super::*;
    use crate::auth::Session;
    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_json, method, path, query_param},
    };

    fn pat() -> Session {
        Session::from_personal_access_token("test-only-token".into()).unwrap()
    }

    #[tokio::test]
    async fn favorites_use_pikpak_star_filter_and_mutation_endpoint() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/drive/v1/files"))
            .and(query_param("parent_id", "*"))
            .and(query_param("with_audit", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "files":[{"id":"f1","name":"收藏文件.txt","kind":"drive#file","size":"12","system_tag":"STAR"}],
                "next_page_token":""
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/drive/v1/files:unstar"))
            .and(body_json(json!({"ids":["f1"]})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;

        let api = Api::for_mock(&server.uri()).unwrap();
        let files = api.list_favorites(&pat()).await.unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].starred);
        assert_eq!(files[0].size, 12);
        api.set_favorite(&pat(), "f1", false).await.unwrap();
    }

    #[tokio::test]
    async fn share_list_decodes_pikpaktui_string_metadata_and_create_request() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/drive/v1/share/list"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data":[{"share_id":"s1","share_url":"https://mypikpak.com/s/s1","title":"分享","expiration_days":"7","view_count":"3","restore_count":"0","file_num":"2","share_status":"OK"},{"share_id":"s0","share_url":"https://mypikpak.com/s/s0","expiration_days":"-1"}],
                "next_page_token":null
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/drive/v1/share"))
            .and(body_json(json!({"file_ids":["f1"],"share_to":"encryptedlink","expiration_days":7,"pass_code_option":"REQUIRED"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"share_id":"s2","share_url":"https://mypikpak.com/s/s2","pass_code":"1234"})))
            .expect(1)
            .mount(&server)
            .await;

        let api = Api::for_mock(&server.uri()).unwrap();
        let shares = api.list_shares(&pat()).await.unwrap();
        assert_eq!(shares[0].expiration_days, 7);
        assert_eq!(shares[0].view_count, 3);
        assert_eq!(shares[1].expiration_days, -1);
        let created = api
            .create_share(&pat(), &["f1".to_string()], 7, true)
            .await
            .unwrap();
        assert_eq!(created.pass_code, "1234");
    }

    #[tokio::test]
    async fn trash_restore_and_permanent_delete_use_distinct_confirmed_endpoints() {
        let server = MockServer::start().await;
        for (endpoint, id) in [
            ("/drive/v1/files:batchUntrash", "f1"),
            ("/drive/v1/files:batchDelete", "f2"),
        ] {
            Mock::given(method("POST"))
                .and(path(endpoint))
                .and(body_json(json!({"ids":[id]})))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
                .expect(1)
                .mount(&server)
                .await;
        }
        let api = Api::for_mock(&server.uri()).unwrap();
        api.restore_files(&pat(), &["f1".into()]).await.unwrap();
        api.permanently_delete(&pat(), &["f2".into()])
            .await
            .unwrap();
        assert!(
            api.permanently_delete(&pat(), &["../unsafe".into()])
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn pat_permission_denial_does_not_become_a_login_failure() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/denied"))
            .respond_with(ResponseTemplate::new(403).set_body_string("permission denied"))
            .expect(2)
            .mount(&server)
            .await;
        let api = Api::for_mock(&server.uri()).unwrap();
        let pat_error = api
            .drive_request::<serde_json::Value>(&pat(), Method::GET, "/denied", &[], None)
            .await
            .unwrap_err();
        assert!(matches!(pat_error, Error::UnsupportedPermission));

        let oauth: Session = serde_json::from_value(json!({
            "access_token":"test-oauth-token","refresh_token":"test-refresh",
            "captcha_token":"test-captcha","device_id":"test-device",
            "expires_at":u64::MAX,"auth_method":"oauth"
        }))
        .unwrap();
        let oauth_error = api
            .drive_request::<serde_json::Value>(&oauth, Method::GET, "/denied", &[], None)
            .await
            .unwrap_err();
        assert!(matches!(oauth_error, Error::AccessDenied));
    }

    #[tokio::test]
    async fn malformed_api_response_reports_safe_status_and_content_type_only() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/bad-response"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                "private response body must not appear in errors",
                "text/html",
            ))
            .expect(1)
            .mount(&server)
            .await;
        let api = Api::for_mock(&server.uri()).unwrap();
        let error = api
            .drive_request::<serde_json::Value>(&pat(), Method::GET, "/bad-response", &[], None)
            .await
            .unwrap_err();
        match &error {
            Error::ProtocolResponse {
                status,
                content_type,
            } => {
                assert_eq!(*status, 200);
                assert_eq!(content_type, "text/html");
            }
            other => panic!("unexpected error: {other}"),
        }
        assert!(!error.to_string().contains("private response body"));
    }

    #[tokio::test]
    async fn video_fetch_response_accepts_nullable_legacy_link_and_uses_visible_original_media() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/drive/v1/files/video1"))
            .and(query_param("usage", "FETCH"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "name":"大视频.mp4","size":"6442450944","web_content_link":null,
                "medias":[
                    {"media_name":"720P","is_origin":false,"link":{"url":"https://cdn.invalid/transcoded"}},
                    {"media_name":"Original","is_origin":true,"is_visible":true,"need_more_quota":false,"link":{"url":"https://cdn.invalid/original","expire":"later","fallbacks":[]}}
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;
        let api = Api::for_mock(&server.uri()).unwrap();
        assert_eq!(
            api.file_content_link(&pat(), "video1").await.unwrap(),
            "https://cdn.invalid/original"
        );
    }

    #[tokio::test]
    async fn video_playback_uses_cache_rendition_while_download_uses_fetch_original() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/drive/v1/files/video-playback"))
            .and(query_param("usage", "CACHE"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "name":"转码.mp4","size":"1000","web_content_link":"https://cdn.invalid/original-download",
                "medias":[
                    {"media_name":"720P","is_origin":false,"is_visible":true,"link":{"url":"https://cdn.invalid/h264-720p"}},
                    {"media_name":"Original","is_origin":true,"is_visible":true,"link":{"url":"https://cdn.invalid/hevc-original"}}
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;
        let api = Api::for_mock(&server.uri()).unwrap();
        assert_eq!(
            api.file_playback_link(&pat(), "video-playback")
                .await
                .unwrap(),
            "https://cdn.invalid/h264-720p"
        );
    }

    #[tokio::test]
    async fn hidden_or_quota_restricted_original_is_not_bypassed_by_download_link() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/drive/v1/files/video2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "name":"受限视频.mp4","size":"1000","web_content_link":"https://cdn.invalid/download",
                "medias":[{"is_origin":true,"is_visible":false,"link":{"url":"https://cdn.invalid/original"}}]
            })))
            .expect(1)
            .mount(&server)
            .await;
        let api = Api::for_mock(&server.uri()).unwrap();
        let error = api.file_content_link(&pat(), "video2").await.unwrap_err();
        assert!(error.to_string().contains("不可播放"));
    }

    #[tokio::test]
    async fn visible_transcode_can_play_when_original_is_restricted() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/drive/v1/files/video3"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "name":"转码视频.mp4","size":"1000","web_content_link":"https://cdn.invalid/download",
                "medias":[
                    {"is_origin":true,"is_visible":false,"link":{"url":"https://cdn.invalid/hidden-original"}},
                    {"is_origin":false,"is_visible":true,"need_more_quota":false,"link":{"url":"https://cdn.invalid/visible-transcode"}}
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;
        let api = Api::for_mock(&server.uri()).unwrap();
        assert_eq!(
            api.file_content_link(&pat(), "video3").await.unwrap(),
            "https://cdn.invalid/visible-transcode"
        );
    }
}
