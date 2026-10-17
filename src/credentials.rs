use crate::{
    auth::Session,
    error::{Error, Result},
};
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
static SESSION_GENERATION: AtomicU64 = AtomicU64::new(0);
static SESSION_WRITE: Mutex<()> = Mutex::new(());
pub fn invalidate_pending_writes() {
    if let Ok(_guard) = SESSION_WRITE.lock() {
        SESSION_GENERATION.fetch_add(1, Ordering::SeqCst);
    }
}
pub fn generation() -> u64 {
    SESSION_GENERATION.load(Ordering::SeqCst)
}
pub fn save_if_current(session: &Session, epoch: u64) -> Result<()> {
    let _guard = SESSION_WRITE.lock().map_err(|_| Error::Credentials)?;
    if generation() != epoch {
        return Err(Error::Cancelled);
    }
    save_unlocked(session)
}
pub fn save(session: &Session) -> Result<()> {
    save_if_current(session, generation())
}
pub fn delete() -> Result<()> {
    let _guard = SESSION_WRITE.lock().map_err(|_| Error::Credentials)?;
    SESSION_GENERATION.fetch_add(1, Ordering::SeqCst);
    delete_unlocked()
}
#[cfg(windows)]
fn entry() -> Result<keyring::Entry> {
    keyring::Entry::new(crate::settings::APP_ID, "pikpak-session").map_err(|_| Error::Credentials)
}
#[cfg(windows)]
fn save_unlocked(session: &Session) -> Result<()> {
    use secrecy::ExposeSecret;
    // Serialize secrets only into the system credential vault, never a settings file.
    let json=serde_json::json!({"access_token":session.access_token.expose_secret(),"refresh_token":session.refresh_token.expose_secret(),"captcha_token":session.captcha_token.expose_secret(),"device_id":session.device_id,"expires_at":session.expires_at}).to_string();
    entry()?.set_password(&json).map_err(|_| Error::Credentials)
}
#[cfg(windows)]
pub fn load() -> Result<Session> {
    serde_json::from_str(&entry()?.get_password().map_err(|error| match error {
        keyring::Error::NoEntry => Error::NoSession,
        _ => Error::Credentials,
    })?)
    .map_err(|_| Error::Credentials)
}
#[cfg(windows)]
fn delete_unlocked() -> Result<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(Error::Credentials),
    }
}
#[cfg(not(windows))]
fn save_unlocked(_: &Session) -> Result<()> {
    Err(Error::Credentials)
}
#[cfg(not(windows))]
pub fn load() -> Result<Session> {
    Err(Error::Credentials)
}
#[cfg(not(windows))]
fn delete_unlocked() -> Result<()> {
    Err(Error::Credentials)
}
