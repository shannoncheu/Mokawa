#![cfg(windows)]

/// Exercises the real Windows backend using an isolated synthetic credential.
/// Never reads, writes or deletes the application's PikPak session entry.
#[test]
fn windows_credential_manager_round_trip() {
    let unique = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    struct Cleanup(keyring::Entry);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = self.0.delete_credential();
        }
    }
    let entry = Cleanup(keyring::Entry::new("app.mokawa.desktop.tests", &unique).unwrap());
    entry
        .0
        .set_password("synthetic-vault-test-not-an-account-token")
        .unwrap();
    assert_eq!(
        entry.0.get_password().unwrap(),
        "synthetic-vault-test-not-an-account-token"
    );
    entry.0.delete_credential().unwrap();
    assert!(matches!(
        entry.0.get_password(),
        Err(keyring::Error::NoEntry)
    ));
}
