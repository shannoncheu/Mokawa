use crate::{
    api::Api,
    error::{Error, Result},
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    #[default]
    #[serde(rename = "oauth", alias = "o_auth")]
    OAuth,
    PersonalAccessToken,
}

#[derive(Clone, Deserialize)]
pub struct Session {
    pub access_token: SecretString,
    pub refresh_token: SecretString,
    pub captcha_token: SecretString,
    pub device_id: String,
    pub expires_at: u64,
    #[serde(default)]
    pub auth_method: AuthMethod,
}
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Session([REDACTED])")
    }
}
impl Session {
    pub fn expired(&self) -> bool {
        self.auth_method == AuthMethod::OAuth && self.expires_at <= now().saturating_add(60)
    }

    pub fn from_personal_access_token(token: SecretString) -> Result<Self> {
        let token = token.expose_secret();
        if token.trim().is_empty() || token.len() > 2048 || token.chars().any(char::is_control) {
            return Err(Error::Invalid("个人访问令牌格式"));
        }
        Ok(Self {
            access_token: token.to_owned().into(),
            refresh_token: String::new().into(),
            captcha_token: String::new().into(),
            device_id: String::new(),
            expires_at: u64::MAX,
            auth_method: AuthMethod::PersonalAccessToken,
        })
    }
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub struct Registration {
    pub client_id: String,
    pub client_secret: SecretString,
}
impl Registration {
    pub fn from_env() -> Result<Self> {
        let client_id = std::env::var("MOKAWA_PIKPAK_CLIENT_ID")
            .map_err(|_| Error::ClientRegistrationRequired)?;
        let client_secret = std::env::var("MOKAWA_PIKPAK_CLIENT_SECRET")
            .map_err(|_| Error::ClientRegistrationRequired)?;
        if client_id.is_empty() || client_secret.is_empty() {
            return Err(Error::ClientRegistrationRequired);
        }
        Ok(Self {
            client_id,
            client_secret: client_secret.into(),
        })
    }
}
#[derive(Deserialize)]
struct Token {
    access_token: SecretString,
    #[serde(default)]
    refresh_token: Option<SecretString>,
    expires_in: u64,
}
impl Api {
    pub async fn login(
        &self,
        registration: &Registration,
        username: &str,
        password: &SecretString,
        device_id: &str,
    ) -> Result<Session> {
        if username.trim().is_empty() || password.expose_secret().is_empty() || device_id.is_empty()
        {
            return Err(Error::Invalid("登录信息不能为空"));
        }
        let challenge: serde_json::Value = self.user_request("/v1/shield/captcha/init", &serde_json::json!({
            "client_id":registration.client_id,"device_id":device_id,"action":"POST:/v1/auth/signin","meta":{"username":username}
        })).await?;
        if challenge
            .get("url")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty())
        {
            return Err(Error::VerificationRequired);
        }
        let captcha = challenge
            .get("captcha_token")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or(Error::Protocol)?;
        let token: Token = self.user_request("/v1/auth/signin", &serde_json::json!({
            "client_id":registration.client_id,"client_secret":registration.client_secret.expose_secret(),
            "grant_type":"password","username":username,"password":password.expose_secret(),"captcha_token":captcha
        })).await?;
        if token.access_token.expose_secret().is_empty() {
            return Err(Error::Protocol);
        }
        Ok(Session {
            access_token: token.access_token,
            refresh_token: token.refresh_token.ok_or(Error::Protocol)?,
            captcha_token: captcha.to_owned().into(),
            device_id: device_id.into(),
            expires_at: now().saturating_add(token.expires_in),
            auth_method: AuthMethod::OAuth,
        })
    }
    pub async fn refresh(&self, registration: &Registration, session: &Session) -> Result<Session> {
        if session.auth_method != AuthMethod::OAuth {
            return Err(Error::Unauthorized);
        }
        let token:Token = self.user_request("/v1/auth/token", &serde_json::json!({"client_id":registration.client_id,
            "client_secret":registration.client_secret.expose_secret(),"grant_type":"refresh_token","refresh_token":session.refresh_token.expose_secret()})).await?;
        if token.access_token.expose_secret().is_empty() {
            return Err(Error::Protocol);
        }
        Ok(Session {
            access_token: token.access_token,
            refresh_token: token
                .refresh_token
                .unwrap_or_else(|| session.refresh_token.clone()),
            expires_at: now().saturating_add(token.expires_in),
            auth_method: AuthMethod::OAuth,
            ..session.clone()
        })
    }
}
