#[path = "store.rs"]
mod store;
use mokawa::{
    api::Api,
    auth::{AuthMethod, Registration, Session},
    credentials,
    error::Error,
    files::ConfirmedDeletion,
    models::{File, OfflineTask, Share, TransferPhase, TransferRecord},
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
            Error::Unauthorized => "unauthorized",
            Error::NoSession => "no_session",
            Error::InvalidSession => "invalid_session",
            Error::Credentials => "credential_store",
            Error::AccessDenied | Error::UnsupportedPermission => "permission",
            Error::Protocol => "protocol",
            Error::ProtocolResponse { .. } => "protocol",
            Error::Network => "network",
            Error::RateLimited => "rate_limited",
            Error::Http(_) => "http",
            Error::Api => "api",
            Error::VerificationRequired | Error::ClientRegistrationRequired => "authorization",
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
    preview: Mutex<(u64, CancellationToken)>,
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
    fn preview_scope(&self, generation: u64) -> mokawa::error::Result<CancellationToken> {
        let mut preview = self.preview.lock().unwrap();
        if generation < preview.0 {
            return Err(Error::Cancelled);
        }
        preview.1.cancel();
        let token = CancellationToken::new();
        *preview = (generation, token.clone());
        Ok(token)
    }
    fn cancel_preview(&self, generation: u64) {
        let mut preview = self.preview.lock().unwrap();
        if generation >= preview.0 {
            preview.1.cancel();
            *preview = (generation, CancellationToken::new());
        }
    }
    fn cancel_all(&self) {
        self.cancel_page();
        self.preview.lock().unwrap().1.cancel();
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
    starred: bool,
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
            starred: file.starred,
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    appearance: store::Appearance,
    has_session: bool,
    can_clear_session: bool,
    auth_method: Option<AuthMethod>,
    authorization_ready: bool,
    warning: Option<String>,
}
#[tauri::command]
async fn bootstrap(state: State<'_, Shared>) -> Reply<Bootstrap> {
    state.cancel_page();
    let appearance = state.store.load().await;
    let result = tauri::async_runtime::spawn_blocking(credentials::load)
        .await
        .map_err(|_| CommandError::from(Error::Credentials))?;
    let (has_session, can_clear_session, auth_method, warning) = match result {
        Ok(session) => (true, true, Some(session.auth_method), None),
        Err(Error::NoSession) => (false, false, None, None),
        Err(Error::InvalidSession) => (false, true, None, Some(Error::InvalidSession.to_string())),
        Err(e) => (false, false, None, Some(e.to_string())),
    };
    Ok(Bootstrap {
        appearance,
        has_session,
        can_clear_session,
        auth_method,
        authorization_ready: false,
        warning,
    })
}
#[tauri::command]
async fn begin_authorization() -> Reply<()> {
    Err(Error::AuthorizationPending.into())
}
#[tauri::command]
async fn connect_personal_access_token(state: State<'_, Shared>, token: String) -> Reply<()> {
    let session = Session::from_personal_access_token(token.into())?;
    let _gate = state.session_gate.lock().await;
    let epoch = credentials::generation();
    state.api.verify_personal_access_token(&session).await?;
    let saved = session.clone();
    tauri::async_runtime::spawn_blocking(move || credentials::save_if_current(&saved, epoch))
        .await
        .map_err(|_| CommandError::from(Error::Credentials))??;
    Ok(())
}
#[tauri::command]
async fn open_auth_help() -> Reply<()> {
    tauri::async_runtime::spawn_blocking(|| webbrowser::open("https://mypikpak.com/zh-CN/faq#55"))
        .await
        .map_err(|_| CommandError::from(Error::Io))?
        .map_err(|_| Error::Io.into())
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
async fn storage_quota(state: State<'_, Shared>) -> Reply<mokawa::models::DriveQuota> {
    let _slot = state
        .files
        .clone()
        .try_acquire_owned()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙，请稍后重试")))?;
    let session = state.session().await?;
    Ok(state
        .api
        .drive_about(&session)
        .await?
        .quota
        .unwrap_or_default())
}
#[tauri::command]
async fn list_favorites(state: State<'_, Shared>) -> Reply<Vec<PublicFile>> {
    let _slot = state
        .files
        .clone()
        .try_acquire_owned()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙，请稍后重试")))?;
    let session = state.session().await?;
    Ok(state
        .api
        .list_favorites(&session)
        .await?
        .into_iter()
        .map(PublicFile::from)
        .collect())
}
#[tauri::command]
async fn set_favorite(state: State<'_, Shared>, id: String, starred: bool) -> Reply<()> {
    let _slot = state
        .files
        .clone()
        .try_acquire_owned()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙，请稍后重试")))?;
    let session = state.session().await?;
    state
        .api
        .set_favorite(&session, &id, starred)
        .await
        .map_err(Into::into)
}
#[tauri::command]
async fn list_trash(state: State<'_, Shared>) -> Reply<Vec<PublicFile>> {
    let _slot = state
        .files
        .clone()
        .try_acquire_owned()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙，请稍后重试")))?;
    let session = state.session().await?;
    Ok(state
        .api
        .list_files(&session, "", true)
        .await?
        .into_iter()
        .map(PublicFile::from)
        .collect())
}
#[tauri::command]
async fn restore_files(state: State<'_, Shared>, ids: Vec<String>) -> Reply<()> {
    let _slot = state
        .files
        .clone()
        .try_acquire_owned()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙，请稍后重试")))?;
    let session = state.session().await?;
    state
        .api
        .restore_files(&session, &ids)
        .await
        .map_err(Into::into)
}
#[tauri::command]
async fn permanently_delete_files(
    state: State<'_, Shared>,
    ids: Vec<String>,
    confirmed: bool,
) -> Reply<()> {
    if !confirmed {
        return Err(Error::ConfirmationRequired.into());
    }
    let _slot = state
        .files
        .clone()
        .try_acquire_owned()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙，请稍后重试")))?;
    let session = state.session().await?;
    state
        .api
        .permanently_delete(&session, &ids)
        .await
        .map_err(Into::into)
}
#[tauri::command]
async fn list_shares(state: State<'_, Shared>) -> Reply<Vec<Share>> {
    let _slot = state
        .files
        .clone()
        .try_acquire_owned()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙，请稍后重试")))?;
    let session = state.session().await?;
    state.api.list_shares(&session).await.map_err(Into::into)
}
#[tauri::command]
async fn create_share(
    state: State<'_, Shared>,
    ids: Vec<String>,
    expiration_days: u32,
    require_pass_code: bool,
) -> Reply<Share> {
    let _slot = state
        .files
        .clone()
        .try_acquire_owned()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙，请稍后重试")))?;
    let session = state.session().await?;
    state
        .api
        .create_share(&session, &ids, expiration_days, require_pass_code)
        .await
        .map_err(Into::into)
}
#[tauri::command]
async fn delete_shares(state: State<'_, Shared>, ids: Vec<String>, confirmed: bool) -> Reply<()> {
    if !confirmed {
        return Err(Error::ConfirmationRequired.into());
    }
    let _slot = state
        .files
        .clone()
        .try_acquire_owned()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙，请稍后重试")))?;
    let session = state.session().await?;
    state
        .api
        .delete_shares(&session, &ids)
        .await
        .map_err(Into::into)
}
#[tauri::command]
async fn preview_content_link(
    state: State<'_, Shared>,
    id: String,
    generation: u64,
) -> Reply<String> {
    mokawa::files::validate_id(&id)?;
    let _slot = state
        .files
        .clone()
        .try_acquire_owned()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙，请稍后重试")))?;
    let session = state.session().await?;
    let cancel = state.preview_scope(generation)?;
    tokio::select! {
        _ = cancel.cancelled() => Err(Error::Cancelled.into()),
        result = state.api.file_playback_link(&session, &id) => result.map_err(Into::into),
    }
}
#[tauri::command]
async fn text_preview(state: State<'_, Shared>, id: String, generation: u64) -> Reply<String> {
    mokawa::files::validate_id(&id)?;
    let _slot = state
        .files
        .clone()
        .try_acquire_owned()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙，请稍后重试")))?;
    let session = state.session().await?;
    let cancel = state.preview_scope(generation)?;
    tokio::select! {
        _ = cancel.cancelled() => Err(Error::Cancelled.into()),
        result = state.api.text_preview(&session, &id) => result.map_err(Into::into),
    }
}
#[tauri::command]
fn cancel_preview(state: State<'_, Shared>, generation: u64) {
    state.cancel_preview(generation);
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
    start_downloads(&state, vec![id]).await
}
async fn start_downloads(state: &Backend, ids: Vec<String>) -> Reply<Option<String>> {
    if ids.is_empty() || ids.len() > 32 {
        return Err(Error::Invalid("一次最多下载 32 个文件").into());
    }
    let mut unique = std::collections::HashSet::new();
    for id in &ids {
        mokawa::files::validate_id(id)?;
        if !unique.insert(id) {
            return Err(Error::Invalid("下载列表包含重复文件").into());
        }
    }
    let admission = state
        .admission
        .clone()
        .try_acquire_many_owned(ids.len() as u32)
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
    let mut items = Vec::with_capacity(ids.len());
    for id in ids {
        let file = tokio::select! {_ = scope.cancelled()=>return Err(Error::Cancelled.into()),file=state.api.file_details(&session,&id)=>file?};
        if epoch != credentials::generation() || scope.is_cancelled() {
            return Err(Error::Cancelled.into());
        }
        let content_link = file.preferred_content_link()?.to_owned();
        items.push((id, file, content_link));
    }
    let required = items
        .iter()
        .try_fold(0u64, |sum, (_, f, _)| sum.checked_add(f.size))
        .ok_or_else(|| CommandError::from(Error::Invalid("下载文件总大小超出范围")))?;
    let check_dir = directory.clone();
    let available = tauri::async_runtime::spawn_blocking(move || fs2::available_space(check_dir))
        .await
        .map_err(|_| CommandError::from(Error::Io))?
        .map_err(|_| CommandError::from(Error::Io))?;
    if required > available {
        return Err(Error::Invalid("本地磁盘空间不足").into());
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let task_id = format!(
        "{timestamp}-{}-{}",
        std::process::id(),
        state.next.fetch_add(1, Ordering::Relaxed)
    );
    let center = state.center.clone();
    let admission = Arc::new(admission);
    for (index, (id, file, content_link)) in items.into_iter().enumerate() {
        let record = TransferRecord {
            id: format!("{task_id}-{index}"),
            file_id: id,
            file_name: file.name,
            completed: 0,
            total: Some(file.size),
            phase: TransferPhase::Queued,
            local_path: None,
        };
        let center = center.clone();
        let directory = directory.clone();
        let scope = scope.child_token();
        let admission = admission.clone();
        tauri::async_runtime::spawn(async move {
            let _admission = admission;
            let _ = center
                .run_cancellable(record, content_link, directory, scope)
                .await;
        });
    }
    Ok(Some(task_id))
}
#[tauri::command]
async fn start_download_batch(state: State<'_, Shared>, ids: Vec<String>) -> Reply<Option<String>> {
    start_downloads(&state, ids).await
}
#[tauri::command]
async fn offline_tasks(state: State<'_, Shared>) -> Reply<Vec<OfflineTask>> {
    let _slot = state
        .files
        .try_acquire()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙")))?;
    let session = state.session().await?;
    state.api.offline_tasks(&session).await.map_err(Into::into)
}
#[tauri::command]
async fn create_offline_task(
    state: State<'_, Shared>,
    source: String,
    parent_id: String,
) -> Reply<OfflineTask> {
    let _slot = state
        .files
        .try_acquire()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙")))?;
    let session = state.session().await?;
    state
        .api
        .create_offline_task(&session, &source, &parent_id)
        .await
        .map_err(Into::into)
}
#[tauri::command]
async fn retry_offline_task(state: State<'_, Shared>, id: String) -> Reply<()> {
    let _slot = state
        .files
        .try_acquire()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙")))?;
    let session = state.session().await?;
    state
        .api
        .retry_offline_task(&session, &id)
        .await
        .map_err(Into::into)
}
#[tauri::command]
async fn delete_offline_task(state: State<'_, Shared>, id: String) -> Reply<()> {
    let _slot = state
        .files
        .try_acquire()
        .map_err(|_| CommandError::from(Error::Invalid("请求繁忙")))?;
    let session = state.session().await?;
    state
        .api
        .delete_offline_tasks(&session, &[id])
        .await
        .map_err(Into::into)
}
#[tauri::command]
async fn choose_torrent(state: State<'_, Shared>) -> Reply<Option<String>> {
    let _dialog = state
        .dialogs
        .clone()
        .try_acquire_owned()
        .map_err(|_| CommandError::from(Error::Invalid("请先完成当前选择")))?;
    let path = tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .add_filter("BitTorrent 种子", &["torrent"])
            .pick_file()
    })
    .await
    .map_err(|_| CommandError::from(Error::Io))?;
    let Some(path) = path else { return Ok(None) };
    tauri::async_runtime::spawn_blocking(move || {
        let metadata = std::fs::metadata(&path)?;
        if metadata.len() > 8 * 1024 * 1024 {
            return Err(Error::Invalid("种子文件超过 8 MiB"));
        }
        let bytes = std::fs::read(path)?;
        mokawa::torrent::magnet_from_torrent(&bytes)
    })
    .await
    .map_err(|_| CommandError::from(Error::Io))?
    .map(Some)
    .map_err(Into::into)
}
#[tauri::command]
async fn transfer_snapshot(state: State<'_, Shared>) -> Reply<mokawa::transfers::TransferSnapshot> {
    Ok(state.snapshot().await)
}
#[tauri::command]
fn cancel_transfer(state: State<'_, Shared>, id: String) {
    state.center.cancel(&id);
}
#[tauri::command]
async fn open_transfer_location(
    state: State<'_, Shared>,
    id: String,
    reveal_file: bool,
) -> Reply<()> {
    if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
        return Err(Error::Invalid("下载任务标识").into());
    }
    let record = state
        .center
        .snapshot()
        .records
        .into_iter()
        .find(|record| record.id == id && record.phase == TransferPhase::Completed)
        .ok_or_else(|| CommandError::from(Error::Invalid("该下载尚未完成或记录已不存在")))?;
    let path = record
        .local_path
        .map(std::path::PathBuf::from)
        .ok_or_else(|| CommandError::from(Error::Invalid("旧版任务没有本地文件路径记录")))?;
    let target = if reveal_file {
        if !path.is_file() {
            return Err(Error::Invalid("下载文件已被移动或删除").into());
        }
        path
    } else {
        path.parent()
            .filter(|parent| parent.is_dir())
            .ok_or_else(|| CommandError::from(Error::Invalid("下载目录已被移动或删除")))?
            .to_path_buf()
    };
    tokio::task::spawn_blocking(move || {
        let mut command = std::process::Command::new("explorer.exe");
        if reveal_file {
            command.arg(format!("/select,{}", target.display()));
        } else {
            command.arg(target);
        }
        command.spawn().map(|_| ()).map_err(|_| Error::Io)
    })
    .await
    .map_err(|_| CommandError::from(Error::Io))??;
    Ok(())
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
        preview: Mutex::new((0, CancellationToken::new())),
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
        .invoke_handler(tauri::generate_handler![bootstrap,begin_authorization,connect_personal_access_token,open_auth_help,save_appearance,choose_image,remove_image,cancel_page,cancel_preview,list_files,list_favorites,set_favorite,list_trash,restore_files,permanently_delete_files,list_shares,create_share,delete_shares,storage_quota,preview_content_link,text_preview,create_folder,rename_file,trash_files,sign_out,start_download,start_download_batch,transfer_snapshot,cancel_transfer,open_transfer_location,offline_tasks,create_offline_task,retry_offline_task,delete_offline_task,choose_torrent])
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
