use crate::{
    api::Api,
    auth::Session,
    error::{Error, Result},
    models::{File, FileDetails, FilePage},
};
use reqwest::Method;
use serde_json::json;
pub fn validate_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 256
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(Error::Invalid("文件标识"));
    }
    Ok(())
}
pub fn validate_name(name: &str) -> Result<()> {
    if name.trim().is_empty()
        || name.len() > 255
        || name.chars().any(|c| c.is_control() || "/\\".contains(c))
        || matches!(name, "." | "..")
    {
        return Err(Error::Invalid("文件名称"));
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct ConfirmedDeletion {
    ids: Vec<String>,
}
impl ConfirmedDeletion {
    // UI must present the exact IDs/count and invoke this only after confirmation.
    pub fn confirm(ids: Vec<String>, accepted: bool) -> Result<Self> {
        if !accepted {
            return Err(Error::ConfirmationRequired);
        }
        if ids.is_empty() || ids.len() > 100 {
            return Err(Error::Invalid("请选择 1–100 个文件"));
        }
        for id in &ids {
            validate_id(id)?;
        }
        Ok(Self { ids })
    }
}
impl Api {
    pub async fn list_files(
        &self,
        session: &Session,
        parent: &str,
        trashed: bool,
    ) -> Result<Vec<File>> {
        if !parent.is_empty() {
            validate_id(parent)?;
        }
        let mut query = vec![
            ("parent_id", parent.into()),
            ("limit", "100".into()),
            ("filters", json!({"trashed":{"eq":trashed}}).to_string()),
        ];
        let mut files = Vec::new();
        let mut metadata_bytes = 0usize;
        let mut seen = std::collections::HashSet::new();
        for _ in 0..1000 {
            let page: FilePage = self
                .drive_request(session, Method::GET, "/drive/v1/files", &query, None)
                .await?;
            // Pagination tokens also remain in the cycle-detection set; bound their bytes.
            if page.next_page_token.len() > 4096 {
                return Err(Error::Invalid("分页数据超出安全范围"));
            }
            for file in &page.files {
                metadata_bytes = metadata_bytes.saturating_add(
                    file.id.len()
                        + file.name.len()
                        + file.parent_id.len()
                        + file.kind.len()
                        + file.modified_time.len()
                        + file.mime_type.len()
                        + file.web_content_link.len(),
                );
            }
            if files.len().saturating_add(page.files.len()) > 100_000
                || metadata_bytes > 64 * 1024 * 1024
            {
                return Err(Error::Invalid("目录数据过多，请使用更小的文件夹"));
            }
            files.extend(page.files);
            if page.next_page_token.is_empty() {
                return Ok(files);
            }
            if !seen.insert(page.next_page_token.clone()) {
                return Err(Error::Protocol);
            }
            query.retain(|(k, _)| *k != "page_token");
            query.push(("page_token", page.next_page_token));
        }
        Err(Error::Protocol)
    }
    pub async fn create_folder(&self, s: &Session, parent: &str, name: &str) -> Result<File> {
        validate_name(name)?;
        if !parent.is_empty() {
            validate_id(parent)?;
        }
        self.drive_request(
            s,
            Method::POST,
            "/drive/v1/files",
            &[],
            Some(json!({"kind":"drive#folder","name":name,"parent_id":parent})),
        )
        .await
    }
    pub async fn rename(&self, s: &Session, id: &str, name: &str) -> Result<File> {
        validate_id(id)?;
        validate_name(name)?;
        self.drive_request(
            s,
            Method::PATCH,
            &format!("/drive/v1/files/{id}"),
            &[],
            Some(json!({"name":name})),
        )
        .await
    }
    pub async fn trash(&self, s: &Session, confirmation: ConfirmedDeletion) -> Result<()> {
        let _: serde_json::Value = self
            .drive_request(
                s,
                Method::POST,
                "/drive/v1/files:batchTrash",
                &[],
                Some(json!({"ids":confirmation.ids})),
            )
            .await?;
        Ok(())
    }
    pub async fn file_details(&self, s: &Session, id: &str) -> Result<FileDetails> {
        validate_id(id)?;
        self.drive_request(
            s,
            Method::GET,
            &format!("/drive/v1/files/{id}"),
            &[("usage", "FETCH".into())],
            None,
        )
        .await
    }

    /// `CACHE` exposes playback media renditions. Downloads continue to use the
    /// separate `FETCH` details above.
    pub async fn playback_details(&self, s: &Session, id: &str) -> Result<FileDetails> {
        validate_id(id)?;
        self.drive_request(
            s,
            Method::GET,
            &format!("/drive/v1/files/{id}"),
            &[("usage", "CACHE".into())],
            None,
        )
        .await
    }
}
