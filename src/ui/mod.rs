mod input;
mod pages;
mod theme;
use gpui::{
    App, Application, Bounds, Context, Entity, KeyBinding, SharedString, Subscription,
    TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, size,
    uniform_list,
};
use input::TextInput;
use mokawa::transfers::TransferCenter;
use mokawa::{
    api::Api,
    auth::Registration,
    credentials,
    error::{Error, Result},
    files::ConfirmedDeletion,
    models::File,
    state::BrowserState,
};
use pages::Page;
use std::sync::Arc;
use theme::color as rgb;
actions!(mokawa, [Refresh, Quit, Dismiss]);
struct Mokawa {
    browser: BrowserState,
    authenticated: bool,
    logging_out: bool,
    has_saved_session: bool,
    theme_mode: mokawa::settings::ThemeMode,
    appearance_subscription: Option<Subscription>,
    page: Page,
    transfers: TransferCenter,
    download_admission: Arc<tokio::sync::Semaphore>,
    preparation_slots: Arc<tokio::sync::Semaphore>,
    lifetime: tokio_util::sync::CancellationToken,
    download_scope: tokio_util::sync::CancellationToken,
    hide_details: bool,
    ready: bool,
    saving_preferences: bool,
    search: Entity<TextInput>,
    search_query: String,
    search_task: Option<gpui::Task<()>>,
    name: Entity<TextInput>,
    _subscription: Subscription,
    runtime: Arc<tokio::runtime::Runtime>,
    status: String,
    busy: bool,
    descending: bool,
    grid: bool,
    pending_delete: Option<Vec<String>>,
    context_menu: Option<gpui::Point<gpui::Pixels>>,
    task: Option<tokio::task::AbortHandle>,
}
#[derive(Clone)]
enum Operation {
    List,
    Create(String),
    Rename(String, String),
    Trash(Vec<String>),
}
impl Drop for Mokawa {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.lifetime.cancel();
        self.download_scope.cancel();
        self.transfers.cancel_all();
    }
}
impl Mokawa {
    fn request(&mut self, operation: Operation, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if !self.authenticated && !matches!(operation, Operation::List) {
            self.page = Page::Login;
            self.status = Error::NoSession.to_string();
            cx.notify();
            return;
        }
        self.pending_delete = None;
        self.context_menu = None;
        let is_listing = matches!(operation, Operation::List);
        let authenticating = !self.authenticated;
        let generation = self.browser.begin_navigation();
        let parent = self
            .browser
            .path
            .last()
            .map(|p| p.0.clone())
            .unwrap_or_default();
        self.busy = true;
        self.status = "正在连接 PikPak…".into();
        let epoch = credentials::generation();
        let job = self.runtime.spawn(async move {
            let api = Api::new()?;
            let mut session = credentials::load()?;
            if session.expired() {
                session = api.refresh(&Registration::from_env()?, &session).await?;
                credentials::save_if_current(&session, epoch)?;
            }
            match operation {
                Operation::List => {}
                Operation::Create(name) => {
                    api.create_folder(&session, &parent, &name).await?;
                }
                Operation::Rename(id, name) => {
                    api.rename(&session, &id, &name).await?;
                }
                Operation::Trash(ids) => {
                    api.trash(&session, ConfirmedDeletion::confirm(ids, true)?)
                        .await?;
                }
            }
            api.list_files(&session, &parent, false).await
        });
        self.task = Some(job.abort_handle());
        cx.spawn(async move |this, cx| {
            let result: Result<Vec<File>> = job.await.unwrap_or(Err(Error::Cancelled));
            let _ = this.update(cx, |this, cx| {
                if generation != this.browser.generation {
                    return;
                }
                this.busy = false;
                this.task = None;
                match result {
                    Ok(files) => {
                        this.authenticated = true;
                        this.has_saved_session = true;
                        if authenticating {
                            this.page = Page::Files;
                        }
                        let count = files.len();
                        this.browser.apply(generation, files);
                        this.status = format!("共 {count} 项 · PikPak");
                    }
                    Err(error) => {
                        if matches!(
                            error,
                            Error::Unauthorized
                                | Error::Credentials
                                | Error::NoSession
                                | Error::InvalidSession
                        ) {
                            this.authenticated = false;
                            this.page = Page::Login;
                        }
                        if is_listing
                            || matches!(
                                error,
                                Error::Unauthorized
                                    | Error::Credentials
                                    | Error::NoSession
                                    | Error::InvalidSession
                            )
                        {
                            this.browser.files.clear();
                        }
                        this.status = error.to_string();
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn button(label: &str) -> gpui::Stateful<gpui::Div> {
        div()
            .id(SharedString::from(label.to_owned()))
            .px_3()
            .py_2()
            .rounded_lg()
            .bg(rgb(0x292d40))
            .text_sm()
            .cursor_pointer()
            .hover(|s| s.bg(rgb(0x3b4058)))
            .child(label.to_owned())
    }
}
impl Render for Mokawa {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        theme::apply(self.theme_mode, window.appearance());
        let titlebar = self.titlebar(window);
        let body = self.render_body(window, cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x141621))
            .text_color(rgb(0xe4e7f2))
            .font_family("Microsoft YaHei UI")
            .on_action(cx.listener(|this, _: &Dismiss, _, cx| {
                this.context_menu = None;
                this.pending_delete = None;
                cx.notify();
            }))
            .child(titlebar)
            .child(div().flex_1().min_h_0().child(body))
    }
}
impl Mokawa {
    fn render_body(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        if self.page != Page::Files {
            return self.secondary_page(cx).into_any_element();
        }
        let files = self
            .browser
            .visible_indices(&self.search_query, self.descending);
        let selected = self
            .browser
            .files
            .iter()
            .find(|f| self.browser.selected.contains(&f.id))
            .cloned();
        let mut crumbs =
            div()
                .flex()
                .gap_2()
                .items_center()
                .child(
                    Self::button("我的文件").on_click(cx.listener(|this, _, _, cx| {
                        if !this.busy {
                            this.browser.path.clear();
                            this.request(Operation::List, cx);
                        }
                    })),
                );
        for (index, (_, name)) in self.browser.path.iter().enumerate() {
            crumbs = crumbs.child("/").child(
                Self::button(name)
                    .id(SharedString::from(format!("crumb-{index}")))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.busy {
                            this.browser.path.truncate(index + 1);
                            this.request(Operation::List, cx);
                        }
                    })),
            );
        }
        let mut content = div()
            .id("file-list")
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .min_h_0()
            .overflow_hidden();
        if files.is_empty() {
            content = content.child(div().p_8().text_color(rgb(0x9da6be)).child(if self.busy {
                "正在加载文件…"
            } else if self.browser.files.is_empty() {
                "暂无文件。请先连接 PikPak 账号，再刷新。"
            } else {
                "没有匹配的文件。"
            }));
        }
        let columns = if self.grid {
            if _window.viewport_size().width >= px(1120.) {
                3
            } else {
                2
            }
        } else {
            1
        };
        let list = uniform_list(
            "virtual-files",
            files.len().div_ceil(columns),
            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                let mut rows = Vec::with_capacity(range.len());
                for row_index in range {
                    let mut group = div().flex().w_full();
                    for &file_index in
                        &files[row_index * columns..((row_index + 1) * columns).min(files.len())]
                    {
                        let file = &this.browser.files[file_index];
                        let id = file.id.clone();
                        let right_id = id.clone();
                        let name = file.name.clone();
                        let folder = file.is_folder();
                        let highlighted = this.browser.selected.contains(&id);
                        let row = div()
                            .id(SharedString::from(format!("file-{id}")))
                            .flex()
                            .items_center()
                            .gap_4()
                            .px_4()
                            .py_3()
                            .rounded_lg()
                            .cursor_pointer()
                            .bg(rgb(if highlighted { 0x30374f } else { 0x191c28 }))
                            .hover(|s| s.bg(rgb(0x292f42)))
                            .child(if folder { "▣" } else { "▤" })
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .when(!this.grid, |label| label.flex_1())
                                    .when(this.grid, |label| label.w_full().h(px(42.)))
                                    .child(file.name.clone()),
                            )
                            .child(div().text_sm().text_color(rgb(0xa3acc3)).child(if folder {
                                "文件夹".into()
                            } else {
                                format!("{} 字节", file.size)
                            }))
                            .when(!this.grid, |row| {
                                row.child(
                                    div()
                                        .w(px(185.))
                                        .text_sm()
                                        .text_color(rgb(0xa3acc3))
                                        .child(file.modified_time.clone()),
                                )
                            })
                            .when(this.grid, |row| {
                                row.flex_col()
                                    .items_start()
                                    .h(px(148.))
                                    .m_1()
                                    .border_1()
                                    .border_color(rgb(0x30374f))
                                    .bg(rgb(if highlighted { 0x30374f } else { 0x202331 }))
                            })
                            .flex_1()
                            .min_w_0()
                            .on_mouse_down(
                                gpui::MouseButton::Right,
                                cx.listener(
                                    move |this, event: &gpui::MouseDownEvent, window, cx| {
                                        if this.busy {
                                            return;
                                        }
                                        if !this.browser.selected.contains(&right_id) {
                                            this.browser.selected.clear();
                                            this.browser.selected.insert(right_id.clone());
                                        }
                                        let viewport = window.viewport_size();
                                        this.context_menu = Some(gpui::point(
                                            event
                                                .position
                                                .x
                                                .min(viewport.width - px(190.))
                                                .max(px(0.)),
                                            (event.position.y - px(44.))
                                                .min(viewport.height - px(250.))
                                                .max(px(0.)),
                                        ));
                                        cx.notify();
                                    },
                                ),
                            )
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                                    this.context_menu = None;
                                    if this.busy {
                                        return;
                                    }
                                    if event.click_count == 2 && folder {
                                        this.search.update(cx, |input, cx| {
                                            input.reset();
                                            cx.notify();
                                        });
                                        this.browser.path.push((id.clone(), name.clone()));
                                        this.request(Operation::List, cx);
                                    } else {
                                        if !event.modifiers.control {
                                            this.browser.selected.clear();
                                        }
                                        if !this.browser.selected.insert(id.clone()) {
                                            this.browser.selected.remove(&id);
                                        }
                                        cx.notify();
                                    }
                                }),
                            );
                        group = group.child(row);
                    }
                    for _ in ((row_index + 1) * columns).min(files.len())..(row_index + 1) * columns
                    {
                        group = group.child(div().flex_1().min_w_0());
                    }
                    rows.push(group);
                }
                rows
            }),
        )
        .flex_1()
        .h_full();
        if !self.browser.files.is_empty() {
            content = content.child(list);
        }

        let sidebar = self.navigation(cx);
        let toolbar = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .items_center()
            .child(div().w(px(240.)).child(self.search.clone()))
            .child(
                Self::button("刷新")
                    .on_click(cx.listener(|this, _, _, cx| this.request(Operation::List, cx))),
            )
            .child(
                Self::button("切换名称排序").on_click(cx.listener(|this, _, _, cx| {
                    this.descending = !this.descending;
                    cx.notify();
                })),
            )
            .child(
                Self::button(if self.grid {
                    "列表视图"
                } else {
                    "网格视图"
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.grid = !this.grid;
                    cx.notify();
                })),
            );
        let operations = div()
            .child(
                Self::button("上传文件").on_click(cx.listener(|this, _, _, cx| {
                    this.status = "上传尚未接入，当前无法上传文件。".into();
                    cx.notify();
                })),
            )
            .child(
                Self::button("下载")
                    .on_click(cx.listener(|this, _, _, cx| this.choose_download(cx))),
            )
            .flex()
            .flex_wrap()
            .gap_2()
            .items_center()
            .child(div().w(px(220.)).child(self.name.clone()))
            .child(
                Self::button("新建文件夹").on_click(cx.listener(|this, _, _, cx| {
                    let name = this.name.read(cx).content.to_string();
                    this.request(Operation::Create(name), cx);
                })),
            )
            .child(
                Self::button("重命名").on_click(cx.listener(|this, _, _, cx| {
                    if this.browser.selected.len() == 1 {
                        let id = this.browser.selected.first().unwrap().clone();
                        let name = this.name.read(cx).content.to_string();
                        this.request(Operation::Rename(id, name), cx);
                    } else {
                        this.status = "请只选择一个文件后重命名。".into();
                        cx.notify();
                    }
                })),
            )
            .child(
                Self::button("移入回收站").on_click(cx.listener(|this, _, _, cx| {
                    if !this.browser.selected.is_empty() {
                        this.pending_delete = Some(this.browser.selected.iter().cloned().collect());
                        cx.notify();
                    }
                })),
            );
        let mut center = div()
            .bg(rgb(0x191c28))
            .rounded_xl()
            .mr_3()
            .mb_3()
            .flex_1()
            .min_w_0()
            .h_full()
            .p_5()
            .flex()
            .flex_col()
            .gap_4()
            .when(smoke_files(), |center| {
                center.child(
                    div()
                        .p_2()
                        .rounded_lg()
                        .bg(rgb(0x4a2935))
                        .text_sm()
                        .child("界面测试数据 · 未连接 PikPak · 仅验证渲染"),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_2xl().child("我的文件"))
                    .child(
                        Self::button(if self.authenticated {
                            "已连接 PikPak"
                        } else {
                            "登录账号"
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.page = Page::Login;
                            cx.notify();
                        })),
                    ),
            )
            .child(toolbar)
            .child(crumbs)
            .child(operations)
            .when(!self.grid, |center| {
                center.child(
                    div()
                        .flex()
                        .px_4()
                        .py_2()
                        .text_sm()
                        .text_color(rgb(0xaab3ce))
                        .child(div().flex_1().child("名称"))
                        .child(div().w(px(130.)).child("大小"))
                        .child(div().w(px(185.)).child("修改时间")),
                )
            });
        if let Some(ids) = &self.pending_delete {
            center = center.child(
                div()
                    .p_4()
                    .rounded_lg()
                    .bg(rgb(0x4a2935))
                    .flex()
                    .gap_3()
                    .items_center()
                    .child(format!(
                        "确定将选中的 {} 个文件移入 PikPak 回收站？",
                        ids.len()
                    ))
                    .child(Self::button("确认移入回收站").on_click(cx.listener(
                        |this, _, _, cx| {
                            if !this.busy
                                && let Some(ids) = this.pending_delete.take()
                            {
                                this.request(Operation::Trash(ids), cx);
                            }
                        },
                    )))
                    .child(Self::button("取消").on_click(cx.listener(|this, _, _, cx| {
                        this.pending_delete = None;
                        cx.notify();
                    }))),
            );
        }
        center = center.child(content).child(
            div()
                .text_sm()
                .text_color(rgb(0xaab3ce))
                .child(self.status.clone()),
        );
        let selected_present = selected.is_some();
        let mut detail = div()
            .w(px(220.))
            .flex_shrink_0()
            .h_full()
            .p_5()
            .bg(rgb(0x141621))
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_lg().child("文件详情"));
        detail = if let Some(file) = selected {
            detail
                .child(file.name)
                .child(format!("{} 字节", file.size))
                .child(file.mime_type)
                .child(file.modified_time)
        } else {
            detail.child(
                div()
                    .text_sm()
                    .text_color(rgb(0x838ba4))
                    .child("选择文件查看详情，按住 Ctrl 可多选。"),
            )
        };
        div()
            .relative()
            .size_full()
            .flex()
            .bg(rgb(0x141621))
            .text_color(rgb(0xe4e7f2))
            .font_family("Microsoft YaHei UI")
            .on_action(cx.listener(|this, _: &Refresh, _, cx| this.request(Operation::List, cx)))
            .child(sidebar)
            .child(center)
            .when(
                selected_present
                    && !self.hide_details
                    && _window.viewport_size().width >= px(1120.),
                |root| root.child(detail),
            )
            .children(self.context_menu.map(|position| {
                div()
                    .absolute()
                    .left(position.x)
                    .top(position.y)
                    .w(px(180.))
                    .p_2()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(0x30374f))
                    .bg(rgb(0x202331))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Self::button("下载").on_click(cx.listener(|this, _, _, cx| {
                        this.context_menu = None;
                        this.choose_download(cx);
                    })))
                    .child(
                        Self::button("重命名").on_click(cx.listener(|this, _, _, cx| {
                            this.context_menu = None;
                            if this.browser.selected.len() == 1 {
                                let id = this.browser.selected.first().unwrap().clone();
                                let name = this.name.read(cx).content.to_string();
                                this.request(Operation::Rename(id, name), cx);
                            } else {
                                this.status = "请只选择一个文件后重命名。".into();
                                cx.notify();
                            }
                        })),
                    )
                    .child(
                        Self::button("移入回收站").on_click(cx.listener(|this, _, _, cx| {
                            this.context_menu = None;
                            this.pending_delete =
                                Some(this.browser.selected.iter().cloned().collect());
                            cx.notify();
                        })),
                    )
                    .child(
                        Self::button("关闭菜单").on_click(cx.listener(|this, _, _, cx| {
                            this.context_menu = None;
                            cx.notify();
                        })),
                    )
            }))
            .into_any_element()
    }
}
fn smoke_files() -> bool {
    std::env::args().any(|arg| arg == "--smoke-test")
        && std::env::args().any(|arg| arg == "--smoke-fixture")
}
fn smoke_theme() -> Option<mokawa::settings::ThemeMode> {
    std::env::args().find_map(|arg| match arg.as_str() {
        "--smoke-theme=light" => Some(mokawa::settings::ThemeMode::Light),
        "--smoke-theme=dark" => Some(mokawa::settings::ThemeMode::Dark),
        "--smoke-theme=system" => Some(mokawa::settings::ThemeMode::System),
        _ => None,
    })
}
pub fn run() {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(8)
        .enable_all()
        .build()
    {
        Ok(r) => Arc::new(r),
        Err(_) => {
            tracing::error!("Could not initialize background runtime");
            return;
        }
    };
    Application::new().run(move |cx: &mut App| {
        input::init(cx);
        cx.bind_keys([
            KeyBinding::new("f5", Refresh, None),
            KeyBinding::new("escape", Dismiss, None),
            KeyBinding::new("alt-f4", Quit, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        let small = std::env::args().any(|arg| arg == "--smoke-small");
        let mut dimensions = if small {
            size(px(760.), px(560.))
        } else {
            size(px(1200.), px(780.))
        };
        if let Some(display) = cx.primary_display() {
            let available = display.bounds().size;
            // Leave room for the native frame and taskbar on smaller desktops.
            dimensions = dimensions.min(&size(
                available.width - px(64.),
                available.height - px(128.),
            ));
        }
        let minimum = size(px(760.), px(560.)).min(&dimensions);
        let bounds = Bounds::centered(None, dimensions, cx);
        let result = cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Mokawa".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(minimum),
                app_id: Some(mokawa::settings::APP_ID.into()),
                window_background: gpui::WindowBackgroundAppearance::Opaque,
                ..Default::default()
            },
            |window, cx| {
                theme::apply(Default::default(), window.appearance());
                let search = cx.new(|cx| TextInput::new("搜索当前文件夹", cx));
                let name = cx.new(|cx| TextInput::new("输入文件或文件夹名称", cx));
                cx.new(|cx| {
                    let subscription = cx.observe(&search, |this: &mut Mokawa, _, cx| {
                        let query = this.search.read(cx).content.to_string();
                        this.search_task = Some(cx.spawn(async move |this, cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(180))
                                .await;
                            let _ = this.update(cx, |this, cx| {
                                this.search_query = query;
                                cx.notify();
                            });
                        }));
                    });
                    let mut view = Mokawa {
                        browser: BrowserState::default(),
                        authenticated: false,
                        logging_out: false,
                        has_saved_session: false,
                        theme_mode: smoke_theme().unwrap_or_default(),
                        appearance_subscription: None,
                        page: match std::env::args()
                            .find_map(|arg| arg.strip_prefix("--smoke-page=").map(str::to_owned))
                            .as_deref()
                        {
                            Some("files") => Page::Files,
                            Some("tasks") => Page::Tasks,
                            Some("settings") => Page::Settings,
                            _ => Page::Login,
                        },
                        transfers: Mokawa::new_transfers(),
                        download_admission: Arc::new(tokio::sync::Semaphore::new(32)),
                        preparation_slots: Arc::new(tokio::sync::Semaphore::new(4)),
                        lifetime: tokio_util::sync::CancellationToken::new(),
                        download_scope: tokio_util::sync::CancellationToken::new(),
                        hide_details: false,
                        ready: false,
                        saving_preferences: false,
                        search,
                        search_query: String::new(),
                        search_task: None,
                        name,
                        _subscription: subscription,
                        runtime: runtime.clone(),
                        status: "尚未登录 · 授权接入待完成".into(),
                        busy: false,
                        descending: false,
                        grid: false,
                        pending_delete: None,
                        context_menu: None,
                        task: None,
                    };
                    if smoke_files() {
                        // Explicit diagnostic data only: auto-exits, never a successful account state.
                        view.browser.files = (0..10_000)
                            .map(|index| File {
                                id: format!("ui-test-{index}"),
                                name: format!("测试文件 {index:05}.dat"),
                                kind: if index % 10 == 0 {
                                    "drive#folder"
                                } else {
                                    "drive#file"
                                }
                                .into(),
                                size: (index + 1) * 1024,
                                modified_time: "界面测试数据".into(),
                                ..Default::default()
                            })
                            .collect();
                        view.status =
                            "界面测试：10,000 个合成文件，未连接 PikPak，不代表真实网盘功能。"
                                .into();
                    }
                    view.appearance_subscription =
                        Some(cx.observe_window_appearance(window, |this, window, cx| {
                            theme::apply(this.theme_mode, window.appearance());
                            this.search.update(cx, |_, cx| cx.notify());
                            this.name.update(cx, |_, cx| cx.notify());
                            cx.notify();
                        }));
                    view.start_background(cx);
                    let center = view.transfers.clone();
                    let runtime = view.runtime.clone();
                    cx.on_app_quit(move |view, _| {
                        view.download_scope.cancel();
                        center.cancel_all();
                        let center = center.clone();
                        let job = runtime.spawn(async move {
                            let _ = tokio::time::timeout(
                                std::time::Duration::from_secs(3),
                                center.wait_idle(),
                            )
                            .await;
                        });
                        async move {
                            let _ = job.await;
                        }
                    })
                    .detach();
                    view
                })
            },
        );
        if result.is_err() {
            tracing::error!("Mokawa could not open its native window");
            std::process::exit(1);
        }
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        if std::env::args().any(|arg| arg == "--smoke-test") {
            let timer = cx
                .background_executor()
                .timer(std::time::Duration::from_secs(
                    if std::env::args().any(|arg| arg == "--smoke-interactive") {
                        60
                    } else {
                        5
                    },
                ));
            cx.spawn(async move |cx| {
                timer.await;
                let _ = cx.update(|cx| cx.quit());
            })
            .detach();
        }
        cx.activate(true);
    });
}
