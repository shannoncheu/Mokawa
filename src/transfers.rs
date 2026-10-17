//! PikPak download queue. URLs remain in memory; only non-secret task metadata is persisted.
use crate::{
    download,
    error::{Error, Result},
    models::{TransferPhase, TransferRecord},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::sync::{Notify, Semaphore, watch};

pub const MAX_ACTIVE_TASKS: usize = 32;
pub const MAX_HISTORY: usize = 200;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Default)]
pub struct TransferSnapshot {
    pub records: Vec<TransferRecord>,
    pub errors: HashMap<String, String>,
    pub speeds: HashMap<String, u64>,
    pub persistence_error: Option<String>,
}
#[derive(Clone)]
pub struct TransferCenter(Arc<Inner>);
struct Inner {
    state: Mutex<TransferSnapshot>,
    cancellations: Mutex<HashMap<String, CancellationToken>>,
    writer: tokio::sync::Mutex<()>,
    slots: Semaphore,
    path: PathBuf,
    idle: Notify,
    changes: watch::Sender<()>,
}
impl TransferCenter {
    pub fn new(path: PathBuf) -> Self {
        Self(Arc::new(Inner {
            state: Mutex::new(TransferSnapshot::default()),
            cancellations: Mutex::new(HashMap::new()),
            writer: tokio::sync::Mutex::new(()),
            slots: Semaphore::new(2),
            path,
            idle: Notify::new(),
            changes: watch::channel(()).0,
        }))
    }
    pub fn subscribe(&self) -> watch::Receiver<()> {
        self.0.changes.subscribe()
    }
    pub fn snapshot(&self) -> TransferSnapshot {
        self.0.state.lock().unwrap().clone()
    }
    pub async fn restore(&self) -> Result<()> {
        if !tokio::fs::try_exists(&self.0.path).await? {
            return Ok(());
        }
        let records = download::load_queue(&self.0.path).await?;
        let mut state = self.0.state.lock().unwrap();
        state.records = records.into_iter().rev().take(MAX_HISTORY).collect();
        state.records.reverse();
        self.0.changes.send_replace(());
        Ok(())
    }
    async fn persist(&self) {
        // Serialize writers and take a fresh snapshot only after obtaining the writer lock.
        let _writer = self.0.writer.lock().await;
        let records = self.snapshot().records;
        let result = async {
            if let Some(parent) = self.0.path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            download::save_queue(&self.0.path, &records).await
        }
        .await;
        self.0.state.lock().unwrap().persistence_error = result.err().map(|e: Error| e.to_string());
    }
    pub fn cancel(&self, id: &str) {
        if let Some(token) = self.0.cancellations.lock().unwrap().get(id) {
            token.cancel();
        }
    }
    pub fn cancel_all(&self) {
        for token in self.0.cancellations.lock().unwrap().values() {
            token.cancel();
        }
    }
    pub async fn wait_idle(&self) {
        loop {
            let notified = self.0.idle.notified();
            if self.0.cancellations.lock().unwrap().is_empty() {
                return;
            }
            notified.await;
        }
    }
    /// Call on the background runtime; `url` must come from PikPak file details, never user text.
    pub async fn run(&self, record: TransferRecord, url: String, directory: PathBuf) -> Result<()> {
        self.run_cancellable(record, url, directory, CancellationToken::new())
            .await
    }
    pub async fn run_cancellable(
        &self,
        record: TransferRecord,
        url: String,
        directory: PathBuf,
        cancel: CancellationToken,
    ) -> Result<()> {
        let id = record.id.clone();
        let name = record.file_name.clone();
        {
            let mut active = self.0.cancellations.lock().unwrap();
            // Admission includes queued tasks. A duplicate must never replace a live token.
            if active.len() >= MAX_ACTIVE_TASKS || active.contains_key(&id) {
                return Err(Error::Invalid("下载队列已满或任务已经存在"));
            }
            let mut state = self.0.state.lock().unwrap();
            if state.records.iter().any(|r| r.id == id) {
                return Err(Error::Invalid("重复的任务标识"));
            }
            while state.records.len() >= MAX_HISTORY {
                let Some(index) = state
                    .records
                    .iter()
                    .position(|r| !active.contains_key(&r.id))
                else {
                    return Err(Error::Invalid("下载历史已满"));
                };
                let removed = state.records.remove(index);
                state.errors.remove(&removed.id);
                state.speeds.remove(&removed.id);
            }
            active.insert(id.clone(), cancel.clone());
            state.records.push(record);
        }
        let _registration = Registration {
            center: self.clone(),
            id: id.clone(),
        };
        self.0.changes.send_replace(());
        self.persist().await;
        let result=async {
            if cancel.is_cancelled(){return Err(Error::Cancelled);}
            let _slot=tokio::select!{_ = cancel.cancelled()=>return Err(Error::Cancelled),slot=self.0.slots.acquire()=>slot.map_err(|_|Error::Cancelled)?};
            self.set_phase(&id,TransferPhase::Running);self.persist().await;
            let (tx,mut rx)=watch::channel(0);
            let transfer=download::download(&url,&directory,&name,cancel.clone(),tx);
            tokio::pin!(transfer);
            let started=Instant::now();
            let mut progress_open=true;
            loop {
                tokio::select!{
                    result=&mut transfer=>{if let Some(record)=self.0.state.lock().unwrap().records.iter_mut().find(|r|r.id==id){record.completed = *rx.borrow();}break result;},
                    changed=rx.changed(), if progress_open=>{
                        if changed.is_err(){progress_open=false;continue;}
                        let completed=*rx.borrow_and_update();
                        {let mut state=self.0.state.lock().unwrap();
                        if let Some(record)=state.records.iter_mut().find(|r|r.id==id){record.completed=completed;}
                        state.speeds.insert(id.clone(),(completed as f64/started.elapsed().as_secs_f64().max(0.001)) as u64);}
                        self.0.changes.send_replace(());
                    }
                }
            }
        }.await;
        {
            let mut state = self.0.state.lock().unwrap();
            if let Some(record) = state.records.iter_mut().find(|r| r.id == id) {
                record.phase = match &result {
                    Ok(()) => TransferPhase::Completed,
                    Err(Error::Cancelled) => TransferPhase::Cancelled,
                    Err(_) => TransferPhase::Failed,
                };
            }
            if let Err(ref error) = result {
                state.errors.insert(id.clone(), error.to_string());
            }
            state.speeds.remove(&id);
        }
        self.persist().await;
        self.0.changes.send_replace(());
        result
    }
    fn set_phase(&self, id: &str, phase: TransferPhase) {
        if let Some(record) = self
            .0
            .state
            .lock()
            .unwrap()
            .records
            .iter_mut()
            .find(|r| r.id == id)
        {
            record.phase = phase;
        }
    }
}

// Releases admission even when the enclosing future is dropped or aborted.
struct Registration {
    center: TransferCenter,
    id: String,
}
impl Drop for Registration {
    fn drop(&mut self) {
        let mut active = self.center.0.cancellations.lock().unwrap();
        active.remove(&self.id);
        let mut state = self.center.0.state.lock().unwrap();
        if let Some(record) = state.records.iter_mut().find(|r| r.id == self.id)
            && matches!(record.phase, TransferPhase::Queued | TransferPhase::Running)
        {
            record.phase = TransferPhase::Interrupted;
        }
        state.speeds.remove(&self.id);
        self.center.0.changes.send_replace(());
        self.center.0.idle.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(id: usize) -> TransferRecord {
        TransferRecord {
            id: id.to_string(),
            file_id: "file".into(),
            file_name: "test.bin".into(),
            completed: 0,
            total: None,
            phase: TransferPhase::Queued,
        }
    }
    #[tokio::test]
    async fn abort_releases_registration_and_idle_waiters() {
        let dir = tempfile::tempdir().unwrap();
        let center = TransferCenter::new(dir.path().join("tasks.json"));
        let _slots = center.0.slots.acquire_many(2).await.unwrap();
        let mut changes = center.subscribe();
        let worker = center.clone();
        let path = dir.path().to_path_buf();
        let job = tokio::spawn(async move {
            worker
                .run(record(0), "https://example.invalid".into(), path)
                .await
        });
        changes.changed().await.unwrap();
        assert_eq!(center.0.cancellations.lock().unwrap().len(), 1);
        job.abort();
        let _ = job.await;
        tokio::time::timeout(std::time::Duration::from_secs(1), center.wait_idle())
            .await
            .unwrap();
        assert_eq!(
            center.snapshot().records[0].phase,
            TransferPhase::Interrupted
        );
        assert!(center.snapshot().speeds.is_empty());
        assert_eq!(Arc::strong_count(&center.0), 1);
    }
    #[tokio::test]
    async fn admission_is_bounded_and_duplicate_does_not_replace_token() {
        let dir = tempfile::tempdir().unwrap();
        let center = TransferCenter::new(dir.path().join("tasks.json"));
        let _slots = center.0.slots.acquire_many(2).await.unwrap();
        let mut jobs = Vec::new();
        for id in 0..MAX_ACTIVE_TASKS {
            let mut changes = center.subscribe();
            let worker = center.clone();
            let path = dir.path().to_path_buf();
            jobs.push(tokio::spawn(async move {
                worker
                    .run(record(id), "https://example.invalid".into(), path)
                    .await
            }));
            changes.changed().await.unwrap();
        }
        assert!(
            center
                .run(
                    record(MAX_ACTIVE_TASKS),
                    "https://example.invalid".into(),
                    dir.path().into()
                )
                .await
                .is_err()
        );
        assert!(
            center
                .run(
                    record(0),
                    "https://example.invalid".into(),
                    dir.path().into()
                )
                .await
                .is_err()
        );
        assert_eq!(center.snapshot().records.len(), MAX_ACTIVE_TASKS);
        center.cancel_all();
        for job in jobs {
            assert!(matches!(job.await.unwrap(), Err(Error::Cancelled)));
        }
        center.wait_idle().await;
        assert!(center.0.cancellations.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn history_and_error_maps_remain_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let center = TransferCenter::new(dir.path().join("tasks.json"));
        for id in 0..MAX_HISTORY + 5 {
            assert!(
                center
                    .run(record(id), "http://invalid".into(), dir.path().into())
                    .await
                    .is_err()
            );
        }
        let state = center.snapshot();
        assert_eq!(state.records.len(), MAX_HISTORY);
        assert_eq!(state.errors.len(), MAX_HISTORY);
        assert_eq!(state.records[0].id, "5");
        assert!(state.speeds.is_empty());
        let mut changes = center.subscribe();
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), changes.changed())
                .await
                .is_err()
        );
    }
}
