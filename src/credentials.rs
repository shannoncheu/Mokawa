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
    let json=serde_json::json!({"access_token":session.access_token.expose_secret(),"refresh_token":session.refresh_token.expose_secret(),"captcha_token":session.captcha_token.expose_secret(),"device_id":session.device_id,"expires_at":session.expires_at,"auth_method":session.auth_method}).to_string();
    entry()?.set_password(&json).map_err(|_| Error::Credentials)
}
#[cfg(windows)]
pub fn load() -> Result<Session> {
    read_session(&entry()?)
}
#[cfg(windows)]
fn read_session(entry: &keyring::Entry) -> Result<Session> {
    serde_json::from_str(&entry.get_password().map_err(|error| match error {
        keyring::Error::NoEntry => Error::NoSession,
        _ => Error::Credentials,
    })?)
    .map_err(|_| Error::InvalidSession)
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

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn absent_and_corrupt_credentials_are_distinct() {
        let name = format!("ui-errors-{}-{}", std::process::id(), crate::auth::now());
        let entry = keyring::Entry::new("app.mokawa.desktop.tests", &name).unwrap();
        struct Cleanup(keyring::Entry);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = self.0.delete_credential();
            }
        }
        let entry = Cleanup(entry);
        assert!(matches!(read_session(&entry.0), Err(Error::NoSession)));
        entry.0.set_password("not-a-real-session").unwrap();
        assert!(matches!(read_session(&entry.0), Err(Error::InvalidSession)));
    }

    #[test]
    fn older_saved_sessions_default_to_oauth() {
        let saved = r#"{"access_token":"a","refresh_token":"r","captcha_token":"c","device_id":"d","expires_at":99}"#;
        let session: Session = serde_json::from_str(saved).unwrap();
        assert_eq!(session.auth_method, crate::auth::AuthMethod::OAuth);
    }
}
