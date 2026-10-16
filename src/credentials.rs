use crate::{
    auth::Session,
    error::{Error, Result},
};
#[cfg(windows)]
fn entry() -> Result<keyring::Entry> {
    keyring::Entry::new(crate::settings::APP_ID, "pikpak-session").map_err(|_| Error::Credentials)
}
#[cfg(windows)]
pub fn save(session: &Session) -> Result<()> {
    use secrecy::ExposeSecret;
    // Serialize secrets only into the system credential vault, never a settings file.
    let json=serde_json::json!({"access_token":session.access_token.expose_secret(),"refresh_token":session.refresh_token.expose_secret(),"captcha_token":session.captcha_token.expose_secret(),"device_id":session.device_id,"expires_at":session.expires_at}).to_string();
    entry()?.set_password(&json).map_err(|_| Error::Credentials)
}
#[cfg(windows)]
pub fn load() -> Result<Session> {
    serde_json::from_str(&entry()?.get_password().map_err(|_| Error::Credentials)?)
        .map_err(|_| Error::Credentials)
}
#[cfg(windows)]
pub fn delete() -> Result<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(Error::Credentials),
    }
}
#[cfg(not(windows))]
pub fn save(_: &Session) -> Result<()> {
    Err(Error::Credentials)
}
#[cfg(not(windows))]
pub fn load() -> Result<Session> {
    Err(Error::Credentials)
}
#[cfg(not(windows))]
pub fn delete() -> Result<()> {
    Err(Error::Credentials)
}
