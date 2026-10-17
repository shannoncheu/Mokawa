use crate::{
    auth::{AuthMethod, Session},
    error::{Error, Result},
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
        Self::decode(request.send().await?).await
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
    async fn decode<T: DeserializeOwned>(mut response: reqwest::Response) -> Result<T> {
        match response.status().as_u16() {
            401 => return Err(Error::Unauthorized),
            429 => return Err(Error::RateLimited),
            403 => return Err(Error::AccessDenied),
            200..=299 => {}
            s => return Err(Error::Http(s)),
        }
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
            serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
        if value
            .get("error_code")
            .is_some_and(|c| c.as_i64() != Some(0))
            || value
                .get("error")
                .is_some_and(|e| !e.is_null() && e.as_str() != Some(""))
        {
            return Err(Error::Api);
        }
        serde_json::from_value(value).map_err(|_| Error::Protocol)
    }
}
