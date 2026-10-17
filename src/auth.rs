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
impl std::fmt::Debug for Registration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Registration([REDACTED])")
    }
}
impl Registration {
    pub fn from_values(client_id: String, client_secret: SecretString) -> Result<Self> {
        if client_id.trim().is_empty()
            || client_id.len() > 256
            || client_id.chars().any(char::is_control)
            || client_secret.expose_secret().trim().is_empty()
            || client_secret.expose_secret().len() > 4096
            || client_secret.expose_secret().chars().any(char::is_control)
        {
            return Err(Error::Invalid("PikPak 客户端注册信息格式"));
        }
        Ok(Self {
            client_id,
            client_secret,
        })
    }

    pub fn from_env() -> Result<Self> {
        let client_id = std::env::var("MOKAWA_PIKPAK_CLIENT_ID")
            .map_err(|_| Error::ClientRegistrationRequired)?;
        let client_secret = std::env::var("MOKAWA_PIKPAK_CLIENT_SECRET")
            .map_err(|_| Error::ClientRegistrationRequired)?;
        Self::from_values(client_id, client_secret.into())
    }
}

/// Only allow PikPak-owned HTTPS challenge pages to be opened externally.
/// The CAPTCHA endpoint is allowed to provide a redirect URL, so the desktop
/// client must validate it before handing it to the system browser.
pub fn trusted_verification_url(raw: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(raw) else {
        return false;
    };
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && (host == "mypikpak.com"
            || host.ends_with(".mypikpak.com")
            || host == "pikpakdrive.com"
            || host.ends_with(".pikpakdrive.com"))
}

fn login_meta_key(username: &str) -> &'static str {
    if username.contains('@') {
        "email"
    } else if username.chars().all(|ch| ch.is_ascii_digit() || ch == '+')
        && username.chars().filter(char::is_ascii_digit).count() >= 7
    {
        "phone_number"
    } else {
        "username"
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
        if username.trim().is_empty()
            || username.len() > 320
            || username.chars().any(char::is_control)
            || password.expose_secret().is_empty()
            || password.expose_secret().len() > 1024
            || password.expose_secret().chars().any(char::is_control)
            || device_id.is_empty()
            || device_id.len() > 128
            || device_id.chars().any(char::is_control)
        {
            return Err(Error::Invalid("登录信息不能为空"));
        }
        let mut meta = serde_json::Map::new();
        meta.insert(
            login_meta_key(username).into(),
            serde_json::Value::String(username.into()),
        );
        let challenge: serde_json::Value = self.user_request("/v1/shield/captcha/init", registration, device_id, &serde_json::json!({
            "client_id":registration.client_id,"device_id":device_id,"action":"POST:/v1/auth/signin","meta":meta
        })).await?;
        if let Some(url) = challenge
            .get("url")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            return Err(Error::VerificationRequired(Some(url.to_owned())));
        }
        let captcha = challenge
            .get("captcha_token")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or(Error::Protocol)?;
        let token: Token = self.user_request("/v1/auth/signin", registration, device_id, &serde_json::json!({
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
        let token:Token = self.user_request("/v1/auth/token", registration, &session.device_id, &serde_json::json!({"client_id":registration.client_id,
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

#[cfg(test)]
mod tests {
    use super::{login_meta_key, trusted_verification_url};

    #[test]
    fn account_kind_is_used_for_captcha_metadata() {
        assert_eq!(login_meta_key("user@example.com"), "email");
        assert_eq!(login_meta_key("+8613800000000"), "phone_number");
        assert_eq!(login_meta_key("mokawa-user"), "username");
    }

    #[test]
    fn verification_redirect_is_restricted_to_pikpak_https_hosts() {
        assert!(trusted_verification_url("https://mypikpak.com/verify"));
        assert!(trusted_verification_url("https://user.mypikpak.com/verify"));
        assert!(!trusted_verification_url("http://mypikpak.com/verify"));
        assert!(!trusted_verification_url(
            "https://mypikpak.com.attacker.invalid/"
        ));
        assert!(!trusted_verification_url("https://user:pass@mypikpak.com/"));
        assert!(!trusted_verification_url("https://example.invalid/"));
    }
}
