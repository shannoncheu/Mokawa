use mokawa::{
    error::{Error, Result},
    personalization::{MAX_IMAGE_BYTES, Personalization},
};
use serde::Serialize;
use std::path::PathBuf;
use tokio::io::AsyncReadExt;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Appearance {
    pub settings: Personalization,
    pub avatar: Option<String>,
    pub background: Option<String>,
    pub warnings: Vec<String>,
}
pub struct Store {
    root: PathBuf,
    writer: tokio::sync::Mutex<()>,
}
impl Store {
    pub fn new() -> Self {
        Self {
            root: mokawa::settings::data_dir().join("personalization"),
            writer: tokio::sync::Mutex::new(()),
        }
    }
    pub async fn load(&self) -> Appearance {
        let _writer = self.writer.lock().await;
        let mut warnings = Vec::new();
        let settings = match async {
            let file = tokio::fs::File::open(self.root.join("settings.json")).await?;
            let mut bytes = Vec::new();
            file.take(16385).read_to_end(&mut bytes).await?;
            if bytes.len() > 16384 {
                return Err(Error::Invalid("设置文件过大"));
            }
            let p: Personalization = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
            p.validate()?;
            Ok(p)
        }
        .await
        {
            Ok(p) => p,
            Err(_) => {
                if self.root.join("settings.json").exists() {
                    warnings.push("个性化设置无法读取，已恢复安全默认值。".into());
                }
                Personalization::default()
            }
        };
        let mut image_path = |name: &str| {
            let path = self.root.join(name);
            match std::fs::metadata(&path) {
                Ok(meta) if meta.len() <= MAX_IMAGE_BYTES => {
                    Some(path.to_string_lossy().into_owned())
                }
                Ok(_) => {
                    warnings.push("本地图片超出限制，已回退默认外观。".into());
                    None
                }
                Err(_) => None,
            }
        };
        let avatar = image_path("avatar.png");
        let background = image_path("background.jpg");
        Appearance {
            settings,
            avatar,
            background,
            warnings,
        }
    }
    pub async fn save(&self, settings: Personalization) -> Result<()> {
        settings.validate()?;
        let _writer = self.writer.lock().await;
        tokio::fs::create_dir_all(&self.root).await?;
        let bytes = serde_json::to_vec(&settings).map_err(|_| Error::Protocol)?;
        let pending = self.root.join("settings.pending");
        tokio::fs::write(&pending, bytes).await?;
        tokio::fs::rename(pending, self.root.join("settings.json")).await?;
        Ok(())
    }
    pub async fn import(&self, path: PathBuf, avatar: bool) -> Result<String> {
        let _writer = self.writer.lock().await;
        let bytes = tauri::async_runtime::spawn_blocking(move || {
            mokawa::personalization::sanitize_image(&path, avatar)
        })
        .await
        .map_err(|_| Error::Io)??;
        tokio::fs::create_dir_all(&self.root).await?;
        let destination = self.root.join(if avatar {
            "avatar.png"
        } else {
            "background.jpg"
        });
        let pending = destination.with_extension("pending");
        tokio::fs::write(&pending, bytes).await?;
        tokio::fs::rename(pending, &destination).await?;
        Ok(destination.to_string_lossy().into_owned())
    }
    pub async fn remove(&self, avatar: bool) -> Result<()> {
        let _writer = self.writer.lock().await;
        let path = self.root.join(if avatar {
            "avatar.png"
        } else {
            "background.jpg"
        });
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(Error::Io),
        }
    }
}
