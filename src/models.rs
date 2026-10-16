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
