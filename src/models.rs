use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct File {
    #[serde(default, deserialize_with = "nullable_string")]
    pub id: String,
    #[serde(default, deserialize_with = "nullable_string")]
    pub parent_id: String,
    #[serde(default, deserialize_with = "nullable_string")]
    pub name: String,
    #[serde(default, deserialize_with = "nullable_string")]
    pub kind: String,
    #[serde(default, deserialize_with = "size_value")]
    pub size: u64,
    #[serde(default, deserialize_with = "nullable_string")]
    pub modified_time: String,
    #[serde(default, deserialize_with = "nullable_string")]
    pub mime_type: String,
    #[serde(default)]
    pub trashed: bool,
    #[serde(default)]
    pub starred: bool,
    #[serde(default, deserialize_with = "nullable_string")]
    pub web_content_link: String,
}
impl File {
    pub fn is_folder(&self) -> bool {
        self.kind == "drive#folder"
    }
}
fn size_value<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    match value {
        serde_json::Value::String(s) => s.parse().map_err(serde::de::Error::custom),
        serde_json::Value::Number(n) => n
            .as_u64()
            .ok_or_else(|| serde::de::Error::custom("invalid size")),
        serde_json::Value::Null => Ok(0),
        _ => Err(serde::de::Error::custom("invalid size")),
    }
}

/// The FETCH usage response has a richer, optional media shape than a list row.
/// Keep it separate so list models stay small and unknown API fields remain ignored.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct FileDetails {
    #[serde(default, deserialize_with = "nullable_string")]
    pub name: String,
    #[serde(default, deserialize_with = "size_value")]
    pub size: u64,
    #[serde(default, deserialize_with = "nullable_string")]
    pub web_content_link: String,
    #[serde(default)]
    pub links: Option<std::collections::HashMap<String, MediaLink>>,
    #[serde(default)]
    pub medias: Option<Vec<MediaInfo>>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MediaLink {
    #[serde(default, deserialize_with = "optional_string")]
    pub url: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MediaInfo {
    #[serde(default)]
    pub is_origin: Option<bool>,
    #[serde(default)]
    pub is_visible: Option<bool>,
    #[serde(default)]
    pub need_more_quota: Option<bool>,
    #[serde(default)]
    pub link: Option<MediaLink>,
}

impl FileDetails {
    pub fn preferred_content_link(&self) -> crate::error::Result<&str> {
        if let Some(medias) = self.medias.as_ref().filter(|medias| !medias.is_empty()) {
            let available = medias
                .iter()
                .filter(|media| {
                    media.is_visible != Some(false) && media.need_more_quota != Some(true)
                })
                .filter_map(|media| {
                    media
                        .link
                        .as_ref()
                        .and_then(|link| link.url.as_deref())
                        .filter(|url| !url.is_empty())
                        .map(|url| (media.is_origin != Some(true), url))
                })
                .min_by_key(|(not_original, _)| *not_original);
            if let Some((_, url)) = available {
                return Ok(url);
            }
            if medias
                .iter()
                .any(|media| media.need_more_quota == Some(true))
            {
                return Err(crate::error::Error::Invalid(
                    "PikPak 标记媒体需要额外配额；Mokawa 不会绕过此限制",
                ));
            }
            if medias.iter().all(|media| media.is_visible == Some(false)) {
                return Err(crate::error::Error::Invalid(
                    "PikPak 标记媒体不可播放；Mokawa 不会绕过此限制",
                ));
            }
            return Err(crate::error::Error::Invalid(
                "PikPak 尚未准备好可播放的媒体链接",
            ));
        }
        self.web_content_link
            .is_empty()
            .then(|| {
                self.links
                    .as_ref()
                    .and_then(|links| links.get("application/octet-stream"))
                    .and_then(|link| link.url.as_deref())
            })
            .flatten()
            .filter(|url| !url.is_empty())
            .or_else(|| {
                (!self.web_content_link.is_empty()).then_some(self.web_content_link.as_str())
            })
            .ok_or(crate::error::Error::Invalid(
                "PikPak 未提供此文件的访问链接，请刷新或下载",
            ))
    }

    /// Playback prefers a server-provided transcoded rendition over the
    /// original file URL used for downloads.
    pub fn preferred_playback_link(&self) -> crate::error::Result<&str> {
        if let Some(medias) = self.medias.as_ref().filter(|medias| !medias.is_empty())
            && let Some(url) = medias
                .iter()
                .filter(|media| media.is_origin == Some(false))
                .filter(|media| {
                    media.is_visible != Some(false) && media.need_more_quota != Some(true)
                })
                .find_map(|media| {
                    media
                        .link
                        .as_ref()
                        .and_then(|link| link.url.as_deref())
                        .filter(|url| !url.is_empty())
                })
        {
            return Ok(url);
        }
        self.preferred_content_link()
    }
}

fn nullable_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

fn optional_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}
#[derive(Debug, Default, Deserialize)]
pub struct FilePage {
    #[serde(default)]
    pub files: Vec<File>,
    #[serde(default)]
    pub next_page_token: String,
}

/// PikPak `/drive/v1/about` quota values are byte counts. Both number and
/// decimal-string encodings have been observed in public client models.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct DriveQuota {
    #[serde(default, deserialize_with = "optional_u64_value")]
    pub limit: Option<u64>,
    #[serde(default, deserialize_with = "optional_u64_value")]
    pub usage: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct DriveAbout {
    #[serde(default)]
    pub quota: Option<DriveQuota>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct OfflineTask {
    pub id: String,
    #[serde(default)]
    pub file_id: String,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub phase: String,
    #[serde(default, deserialize_with = "optional_u64_value")]
    pub file_size: Option<u64>,
    #[serde(default, deserialize_with = "u64_value")]
    pub progress: u64,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub created_time: String,
    #[serde(default)]
    pub updated_time: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct OfflineTaskPage {
    #[serde(default)]
    pub tasks: Vec<OfflineTask>,
    #[serde(default)]
    pub next_page_token: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OfflineTaskResponse {
    pub task: Option<OfflineTask>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Share {
    #[serde(default, alias = "shareId")]
    pub share_id: String,
    #[serde(default, alias = "shareUrl")]
    pub share_url: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, alias = "passCode")]
    pub pass_code: String,
    #[serde(default, alias = "shareTo")]
    pub share_to: String,
    #[serde(default, alias = "createTime")]
    pub create_time: String,
    #[serde(default, alias = "expirationDays", deserialize_with = "i64_value")]
    pub expiration_days: i64,
    #[serde(default, alias = "viewCount", deserialize_with = "u64_value")]
    pub view_count: u64,
    #[serde(default, alias = "restoreCount", deserialize_with = "u64_value")]
    pub restore_count: u64,
    #[serde(default, alias = "fileNum", deserialize_with = "u64_value")]
    pub file_num: u64,
    #[serde(default, alias = "shareStatus")]
    pub share_status: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct SharePage {
    #[serde(default)]
    pub data: Vec<Share>,
    #[serde(default, alias = "nextPageToken")]
    pub next_page_token: Option<String>,
}

fn optional_u64_value<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(d)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) if s.is_empty() => Ok(None),
        Some(serde_json::Value::String(s)) => s.parse().map(Some).map_err(serde::de::Error::custom),
        Some(serde_json::Value::Number(n)) => n
            .as_u64()
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom("invalid quota")),
        _ => Err(serde::de::Error::custom("invalid quota")),
    }
}

fn u64_value<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    optional_u64_value(d).map(|value| value.unwrap_or_default())
}
fn i64_value<'de, D: serde::Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(d)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(0),
        Some(serde_json::Value::String(value)) => value.parse().map_err(serde::de::Error::custom),
        Some(serde_json::Value::Number(value)) => value
            .as_i64()
            .ok_or_else(|| serde::de::Error::custom("invalid signed integer")),
        _ => Err(serde::de::Error::custom("invalid signed integer")),
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum TransferPhase {
    Queued,
    Running,
    Interrupted,
    Completed,
    Cancelled,
    Failed,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TransferRecord {
    pub id: String,
    pub file_id: String,
    pub file_name: String,
    pub completed: u64,
    pub total: Option<u64>,
    pub phase: TransferPhase,
    /// Actual on-disk path, set only after a completed download.
    #[serde(default)]
    pub local_path: Option<String>,
}
