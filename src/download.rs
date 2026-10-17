use crate::{
    error::{Error, Result},
    files::validate_name,
    models::{TransferPhase, TransferRecord},
};
use futures_util::StreamExt;
use std::{path::Path, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const MAX_QUEUE_BYTES: u64 = 2 * 1024 * 1024;
use tokio_util::sync::CancellationToken;
/// New downloads only. Range/resume is deliberately unavailable until live validation.
pub async fn download(
    url: &str,
    directory: &Path,
    name: &str,
    cancel: CancellationToken,
    progress: tokio::sync::watch::Sender<u64>,
) -> Result<()> {
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    validate_name(name)?;
    if name.ends_with('.') || name.ends_with(' ') || name.chars().any(|c| "<>:\"|?*".contains(c)) {
        return Err(Error::Invalid("Windows file name"));
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        return Err(Error::Invalid("reserved Windows file name"));
    }
    let url = reqwest::Url::parse(url).map_err(|_| Error::Invalid("download URL"))?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err(Error::Invalid("download URL must use HTTPS"));
    }
    let directory = tokio::fs::canonicalize(directory).await?;
    let destination = directory.join(name);
    let partial = directory.join(format!("{name}.mokawa.part"));
    if tokio::fs::try_exists(&destination).await? {
        return Err(Error::Invalid("destination already exists"));
    }
    let (file, _cleanup) = create_partial(partial.clone()).await?;
    let mut file = tokio::fs::File::from_std(file);
    let result=async {
        let client=reqwest::Client::builder().connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(24*3600)).redirect(reqwest::redirect::Policy::none()).build()?;
        let response=tokio::select!{_ = cancel.cancelled()=>return Err(Error::Cancelled),r=client.get(url).send()=>r?};
        if response.status()!=reqwest::StatusCode::OK {return Err(Error::Http(response.status().as_u16()));}
        let expected=response.content_length();let mut completed=0;let mut stream=response.bytes_stream();
        let mut last_progress=std::time::Instant::now();
        loop {
            let chunk=tokio::select!{_ = cancel.cancelled()=>return Err(Error::Cancelled),r=tokio::time::timeout(Duration::from_secs(30),stream.next())=>r.map_err(|_|Error::Network)?};
            let Some(chunk)=chunk else {break;};let chunk=chunk?;
            file.write_all(&chunk).await?;completed+=chunk.len() as u64;if last_progress.elapsed() >= Duration::from_millis(100) { let _=progress.send(completed); last_progress=std::time::Instant::now(); }
        }
        if expected.is_some_and(|total|total!=completed){return Err(Error::Protocol);}
        let _=progress.send(completed);
        file.sync_all().await?;Ok(())
    }.await;
    drop(file);
    if result.is_ok() {
        // Hard-link creation is atomic and refuses overwriting a concurrently created target.
        if tokio::fs::hard_link(&partial, &destination).await.is_err() {
            let _ = tokio::fs::remove_file(&partial).await;
            return Err(Error::Io);
        }
    }
    let _ = tokio::fs::remove_file(&partial).await;
    result
}
pub async fn save_queue(path: &Path, records: &[TransferRecord]) -> Result<()> {
    let bytes = serde_json::to_vec(records).map_err(|_| Error::Protocol)?;
    // Queue stores file IDs and progress only, never signed download URLs or credentials.
    let temporary = path.with_extension("pending");
    let mut file = tokio::fs::File::create(&temporary).await?;
    file.write_all(&bytes).await?;
    file.sync_all().await?;
    drop(file);
    tokio::fs::rename(temporary, path).await?;
    Ok(())
}
pub async fn load_queue(path: &Path) -> Result<Vec<TransferRecord>> {
    // Bound bytes before parsing, even if a local file grows while being read.
    let file = tokio::fs::File::open(path).await?;
    let mut bytes = Vec::new();
    file.take(MAX_QUEUE_BYTES + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() as u64 > MAX_QUEUE_BYTES {
        return Err(Error::Invalid("task history exceeds size limit"));
    }
    let mut records: Vec<TransferRecord> =
        serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
    for record in &mut records {
        if matches!(record.phase, TransferPhase::Running | TransferPhase::Queued) {
            record.phase = TransferPhase::Interrupted;
        }
    }
    Ok(records)
}

// Construct the owner inside the blocking job: aborting while open() is pending
// still drops the returned owner and cleans up the newly created file.
async fn create_partial(path: std::path::PathBuf) -> Result<(std::fs::File, PartialPath)> {
    tokio::task::spawn_blocking(move || {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        Ok((file, PartialPath(path)))
    })
    .await
    .map_err(|_| Error::Io)?
}
struct PartialPath(std::path::PathBuf);
impl Drop for PartialPath {
    fn drop(&mut self) {
        // Runs on a background worker, never the GPUI thread. Windows file handles
        // use FILE_SHARE_DELETE; unlink also works if an aborted write still owns a handle.
        if let Err(error) = std::fs::remove_file(&self.0)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!("Could not remove a download temporary file");
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn abort_removes_owned_partial_file_without_deleting_conflicts() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.mokawa.part");
        let task_path = path.clone();
        let (ready, started) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let (file, _cleanup) = create_partial(task_path).await.unwrap();
            let mut file = tokio::fs::File::from_std(file);
            file.write_all(&[1; 65536]).await.unwrap();
            ready.send(()).unwrap();
            std::future::pending::<()>().await;
        });
        started.await.unwrap();
        assert!(path.exists());
        task.abort();
        let _ = task.await;
        assert!(!path.exists());
        std::fs::write(&path, b"existing").unwrap();
        assert!(create_partial(path.clone()).await.is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"existing");
    }
}
