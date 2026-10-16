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
use tokio::sync::{Semaphore, watch};
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
}
impl TransferCenter {
    pub fn new(path: PathBuf) -> Self {
        Self(Arc::new(Inner {
            state: Mutex::new(TransferSnapshot::default()),
            cancellations: Mutex::new(HashMap::new()),
            writer: tokio::sync::Mutex::new(()),
            slots: Semaphore::new(2),
            path,
        }))
    }
    pub fn snapshot(&self) -> TransferSnapshot {
        self.0.state.lock().unwrap().clone()
    }
    pub async fn restore(&self) -> Result<()> {
        if !tokio::fs::try_exists(&self.0.path).await? {
            return Ok(());
        }
        let records = download::load_queue(&self.0.path).await?;
        self.0.state.lock().unwrap().records = records;
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
        while !self.0.cancellations.lock().unwrap().is_empty() {
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    }
    /// Call on the background runtime; `url` must come from PikPak file details, never user text.
    pub async fn run(&self, record: TransferRecord, url: String, directory: PathBuf) {
        let id = record.id.clone();
        let name = record.file_name.clone();
        let cancel = CancellationToken::new();
        self.0
            .cancellations
            .lock()
            .unwrap()
            .insert(id.clone(), cancel.clone());
        self.0.state.lock().unwrap().records.push(record);
        self.persist().await;
        let result=async {
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
            if let Err(error) = result {
                state.errors.insert(id.clone(), error.to_string());
            }
            state.speeds.remove(&id);
        }
        self.persist().await;
        self.0.cancellations.lock().unwrap().remove(&id);
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
