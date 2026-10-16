use std::path::PathBuf;
pub const APP_ID: &str = "app.mokawa.desktop";
pub fn data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Mokawa")
}
pub fn init_logging() {
    // Never enable HTTP wire logging; upstream errors and bodies may contain credentials.
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();
}
