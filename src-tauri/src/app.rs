#[path = "store.rs"]
mod store;
use mokawa::{
    api::Api,
    auth::{Registration, Session},
    credentials,
    error::Error,
    files::ConfirmedDeletion,
    models::{File, TransferPhase, TransferRecord},
    personalization::Personalization,
    transfers::TransferCenter,
};
use serde::Serialize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use tauri::{Emitter, Manager, State};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
type Shared = Arc<Backend>;
type Reply<T> = std::result::Result<T, CommandError>;
#[derive(Serialize)]
struct CommandError {
    code: &'static str,
    message: String,
}
impl From<Error> for CommandError {
    fn from(error: Error) -> Self {
        let code = match error {
            Error::Unauthorized
            | Error::NoSession
            | Error::InvalidSession
            | Error::Credentials
            | Error::VerificationRequired
            | Error::ClientRegistrationRequired => "authentication",
            Error::Cancelled => "cancelled",
            _ => "operation",
        };
        Self {
            code,
            message: error.to_string(),
        }
    }
}
struct Backend {
    api: Api,
    center: TransferCenter,
    store: store::Store,
    restored: tokio::sync::OnceCell<Option<String>>,
    page: Mutex<Option<(u64, CancellationToken)>>,
    downloads: Mutex<CancellationToken>,
    quit: CancellationToken,
    session_gate: tokio::sync::Mutex<()>,
    files: Arc<Semaphore>,
    admission: Arc<Semaphore>,
    preparation: Arc<Semaphore>,
    dialogs: Arc<Semaphore>,
    next: AtomicU64,
    exiting: AtomicBool,
}
impl Backend {
    async fn snapshot(&self) -> mokawa::transfers::TransferSnapshot {
        let error = self
            .restored
            .get_or_init(|| async {
                self.center
                    .restore()
                    .await
                    .err()
                    .map(|error| error.to_string())
            })
            .await;
        let mut snapshot = self.center.snapshot();
        if snapshot.persistence_error.is_none() {
            snapshot.persistence_error = error.clone();
        }
        snapshot
    }

    async fn session(&self) -> mokawa::error::Result<Session> {
        let _gate = self.session_gate.lock().await;
        let epoch = credentials::generation();
        let mut session = tauri::async_runtime::spawn_blocking(credentials::load)
            .await
            .map_err(|_| Error::Credentials)??;
        if session.expired() {
            session = self
                .api
                .refresh(&Registration::from_env()?, &session)
                .await?;
            let saved = session.clone();
            tauri::async_runtime::spawn_blocking(move || {
                credentials::save_if_current(&saved, epoch)
            })
            .await
            .map_err(|_| Error::Credentials)??;
        }
        Ok(session)
    }
    fn cancel_page(&self) {
        if let Some((_, token)) = self.page.lock().unwrap().take() {
            token.cancel();
        }
    }
    fn cancel_all(&self) {
        self.cancel_page();
        self.downloads.lock().unwrap().cancel();
        self.center.cancel_all();
    }
}
#[derive(Serialize)]
struct PublicFile {
    id: String,
    name: String,
    kind: String,
    size: u64,
    modified_time: String,
    mime_type: String,
}
impl From<File> for PublicFile {
    fn from(file: File) -> Self {
        Self {
            id: file.id,
            name: file.name,
            kind: file.kind,
            size: file.size,
            modified_time: file.modified_time,
            mime_type: file.mime_type,
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    appearance: store::Appearance,
    has_session: bool,
    authorization_ready: bool,
    warning: Option<String>,
}
#[tauri::command]
async fn bootstrap(state: State<'_, Shared>) -> Reply<Bootstrap> {
    let appearance = state.store.load().await;
    let result = tauri::async_runtime::spawn_blocking(credentials::load)
        .await
        .map_err(|_| CommandError::from(Error::Credentials))?;
    let (has_session, warning) = match result {
        Ok(_) => (true, None),
        Err(Error::NoSession) => (false, None),
        Err(e) => (false, Some(e.to_string())),
    };
    Ok(Bootstrap {
        appearance,
        has_session,
        authorization_ready: false,
        warning,
    })
}
#[tauri::command]
async fn begin_authorization() -> Reply<()> {
    Err(Error::AuthorizationPending.into())
}
#[tauri::command]
async fn open_auth_help() -> Reply<()> {
    tauri::async_runtime::spawn_blocking(||webbrowser::open("https://mypikpak.com/en-US/help-center/connected_apps/managing_connected_apps/what_are_connected_apps")).await.map_err(|_|CommandError::from(Error::Io))?.map_err(|_|Error::Io.into())
}
#[tauri::command]
async fn save_appearance(state: State<'_, Shared>, settings: Personalization) -> Reply<()> {
    state.store.save(settings).await.map_err(Into::into)
}
#[tauri::command]
async fn choose_image(state: State<'_, Shared>, kind: String) -> Reply<Option<String>> {
    let avatar = match kind.as_str() {
        "avatar" => true,
        "background" => false,
        _ => return Err(Error::Invalid("图片类型").into()),
    };
    let _dialog = state
        .dialogs
        .clone()
        .try_acquire_owned()
        .map_err(|_| CommandError::from(Error::Invalid("请先完成当前图片或目录选择")))?;
    let path = tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("选择本地图片（不上传）")
            .add_filter("图片", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
    })
    .await
    .map_err(|_| CommandError::from(Error::Io))?;
    match path {
        Some(path) => Ok(Some(state.store.import(path, avatar).await?)),
        None => Ok(None),
    }
}
#[tauri::command]
async fn remove_image(state: State<'_, Shared>, kind: String) -> Reply<()> {
    let avatar = match kind.as_str() {
        "avatar" => true,
        "background" => false,
        _ => return Err(Error::Invalid("图片类型").into()),
    };
    state.store.remove(avatar).await.map_err(Into::into)
}
#[tauri::command]
fn cancel_page(state: State<'_, Shared>) {
    state.cancel_page();
}
#[tauri::command]
async fn list_files(
    state: State<'_, Shared>,
    parent: String,
    generation: u64,
) -> Reply<Vec<PublicFile>> {
    let _slot = state
        .files
        .clone()
        .try_acquire_owned()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙，请稍后重试")))?;
    let cancel = CancellationToken::new();
    {
        let mut page = state.page.lock().unwrap();
        if page
            .as_ref()
            .is_some_and(|(current, _)| *current > generation)
        {
            return Err(Error::Cancelled.into());
        }
        if let Some((_, old)) = page.replace((generation, cancel.clone())) {
            old.cancel();
        }
    }
    let session = state.session().await?;
    let files = tokio::select! {_ = cancel.cancelled()=>return Err(Error::Cancelled.into()),files=state.api.list_files(&session,&parent,false)=>files?};
    Ok(files.into_iter().map(PublicFile::from).collect())
}
#[tauri::command]
async fn create_folder(
    state: State<'_, Shared>,
    parent: String,
    name: String,
) -> Reply<PublicFile> {
    let _slot = state
        .files
        .try_acquire()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙")))?;
    let session = state.session().await?;
    Ok(state
        .api
        .create_folder(&session, &parent, &name)
        .await?
        .into())
}
#[tauri::command]
async fn rename_file(state: State<'_, Shared>, id: String, name: String) -> Reply<PublicFile> {
    let _slot = state
        .files
        .try_acquire()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙")))?;
    let session = state.session().await?;
    Ok(state.api.rename(&session, &id, &name).await?.into())
}
#[tauri::command]
async fn trash_files(state: State<'_, Shared>, ids: Vec<String>, confirmed: bool) -> Reply<()> {
    let confirmation = ConfirmedDeletion::confirm(ids, confirmed)?;
    let _slot = state
        .files
        .try_acquire()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙")))?;
    let session = state.session().await?;
    state
        .api
        .trash(&session, confirmation)
        .await
        .map_err(Into::into)
}
#[tauri::command]
async fn sign_out(state: State<'_, Shared>) -> Reply<()> {
    state.cancel_all();
    *state.downloads.lock().unwrap() = CancellationToken::new();
    tauri::async_runtime::spawn_blocking(credentials::delete)
        .await
        .map_err(|_| CommandError::from(Error::Credentials))?
        .map_err(Into::into)
}
#[tauri::command]
async fn start_download(state: State<'_, Shared>, id: String) -> Reply<Option<String>> {
    mokawa::files::validate_id(&id)?;
    let admission = state
        .admission
        .clone()
        .try_acquire_owned()
        .map_err(|_| CommandError::from(Error::Invalid("下载队列已满")))?;
    let scope = state.downloads.lock().unwrap().child_token();
    let epoch = credentials::generation();
    let dialog = state
        .dialogs
        .clone()
        .try_acquire_owned()
        .map_err(|_| CommandError::from(Error::Invalid("请先完成当前选择")))?;
    let directory = tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("选择下载目录")
            .pick_folder()
    })
    .await
    .map_err(|_| CommandError::from(Error::Io))?;
    drop(dialog);
    let Some(directory) = directory else {
        return Ok(None);
    };
    let _preparation = tokio::select! {_ = scope.cancelled()=>return Err(Error::Cancelled.into()),slot=state.preparation.clone().acquire_owned()=>slot.map_err(|_|CommandError::from(Error::Cancelled))?};
    let session = state.session().await?;
    let file = tokio::select! {_ = scope.cancelled()=>return Err(Error::Cancelled.into()),file=state.api.file_details(&session,&id)=>file?};
    if epoch != credentials::generation() || scope.is_cancelled() {
        return Err(Error::Cancelled.into());
    }
    if file.web_content_link.is_empty() {
        return Err(Error::Protocol.into());
    }
    let task_id = format!(
        "{}-{}",
        mokawa::auth::now(),
        state.next.fetch_add(1, Ordering::Relaxed)
    );
    let record = TransferRecord {
        id: task_id.clone(),
        file_id: id,
        file_name: file.name,
        completed: 0,
        total: Some(file.size),
        phase: TransferPhase::Queued,
    };
    let center = state.center.clone();
    tauri::async_runtime::spawn(async move {
        let _admission = admission;
        let _ = center
            .run_cancellable(record, file.web_content_link, directory, scope)
            .await;
    });
    Ok(Some(task_id))
}
#[tauri::command]
async fn transfer_snapshot(state: State<'_, Shared>) -> mokawa::transfers::TransferSnapshot {
    state.snapshot().await
}
#[tauri::command]
fn cancel_transfer(state: State<'_, Shared>, id: String) {
    state.center.cancel(&id);
}
pub fn run() {
    mokawa::settings::init_logging();
    let background_runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(8)
        .enable_all()
        .build()
        .expect("后台运行时初始化失败");
    tauri::async_runtime::set(background_runtime.handle().clone());
    let backend = Arc::new(Backend {
        api: Api::new().expect("HTTP 客户端初始化失败"),
        center: TransferCenter::new(mokawa::settings::data_dir().join("downloads.json")),
        store: store::Store::new(),
        restored: tokio::sync::OnceCell::new(),
        page: Mutex::new(None),
        downloads: Mutex::new(CancellationToken::new()),
        quit: CancellationToken::new(),
        session_gate: tokio::sync::Mutex::new(()),
        files: Arc::new(Semaphore::new(4)),
        admission: Arc::new(Semaphore::new(32)),
        preparation: Arc::new(Semaphore::new(4)),
        dialogs: Arc::new(Semaphore::new(1)),
        next: AtomicU64::new(0),
        exiting: AtomicBool::new(false),
    });
    let app=tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window)=app.get_webview_window("main") { let _=window.unminimize();let _=window.show();let _=window.set_focus(); }
        }))
        .manage(backend.clone())
        .invoke_handler(tauri::generate_handler![bootstrap,begin_authorization,open_auth_help,save_appearance,choose_image,remove_image,cancel_page,list_files,create_folder,rename_file,trash_files,sign_out,start_download,transfer_snapshot,cancel_transfer])
        .setup(|app|{
            if let Some(window)=app.get_webview_window("main") && let Some(monitor)=window.current_monitor()? {
                let area=monitor.work_area();let scale=monitor.scale_factor();
                let width=(f64::from(area.size.width)/scale-32.).clamp(320.,1180.);
                let height=(f64::from(area.size.height)/scale-32.).clamp(300.,780.);
                window.set_min_size(Some(tauri::LogicalSize::new(width.min(840.),height.min(600.))))?;
                window.set_size(tauri::LogicalSize::new(width,height))?;
                let outer=window.outer_size()?;
                window.set_position(tauri::PhysicalPosition::new(area.position.x+(area.size.width as i32-outer.width as i32)/2,area.position.y+(area.size.height as i32-outer.height as i32)/2))?;
            }
            let state=app.state::<Shared>().inner().clone();let handle=app.handle().clone();tauri::async_runtime::spawn(async move{
            let _=handle.emit("transfers-changed",state.snapshot().await);
            let mut changes=state.center.subscribe();loop{tokio::select!{_ = state.quit.cancelled()=>break,result=changes.changed()=>if result.is_err(){break;}}tokio::time::sleep(std::time::Duration::from_millis(100)).await;let _=handle.emit("transfers-changed",state.snapshot().await);}
        });Ok(())})
        .build(tauri::generate_context!()).expect("Mokawa 桌面初始化失败");
    app.run(move |handle, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event
            && !backend.exiting.swap(true, Ordering::SeqCst)
        {
            api.prevent_exit();
            backend.cancel_all();
            backend.quit.cancel();
            let state = backend.clone();
            let handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(3),
                    state.center.wait_idle(),
                )
                .await;
                handle.exit(0);
            });
        }
    });
    background_runtime.shutdown_timeout(std::time::Duration::from_secs(3));
}
