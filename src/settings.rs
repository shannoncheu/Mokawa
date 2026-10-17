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

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub compact_rows: bool,
    pub hide_details: bool,
    pub theme: ThemeMode,
}
impl Preferences {
    pub async fn load(path: &std::path::Path) -> crate::error::Result<Self> {
        match tokio::fs::read(path).await {
            Ok(data) => serde_json::from_slice(&data).map_err(|_| crate::error::Error::Protocol),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(_) => Err(crate::error::Error::Io),
        }
    }
    pub async fn save(&self, path: &std::path::Path) -> crate::error::Result<()> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let data = serde_json::to_vec_pretty(self).map_err(|_| crate::error::Error::Protocol)?;
        let temporary = path.with_extension("pending");
        tokio::fs::write(&temporary, data).await?;
        tokio::fs::rename(&temporary, path).await?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}
impl ThemeMode {
    pub fn is_dark(self, system_dark: bool) -> bool {
        match self {
            Self::System => system_dark,
            Self::Light => false,
            Self::Dark => true,
        }
    }
}
