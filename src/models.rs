use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct File {
    pub id: String,
    #[serde(default)]
    pub parent_id: String,
    pub name: String,
    pub kind: String,
    #[serde(default, deserialize_with = "size_value")]
    pub size: u64,
    #[serde(default)]
    pub modified_time: String,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub trashed: bool,
    #[serde(default)]
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
        _ => Err(serde::de::Error::custom("invalid size")),
    }
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
}
