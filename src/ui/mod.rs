mod input;
mod pages;
use gpui::{
    App, Application, Bounds, Context, Entity, KeyBinding, SharedString, Subscription,
    TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
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
actions!(mokawa, [Refresh, Quit]);
struct Mokawa {
    browser: BrowserState,
    page: Page,
    transfers: TransferCenter,
    hide_details: bool,
    ready: bool,
    saving_preferences: bool,
    search: Entity<TextInput>,
    name: Entity<TextInput>,
    _subscription: Subscription,
    runtime: Arc<tokio::runtime::Runtime>,
    status: String,
    busy: bool,
    descending: bool,
    grid: bool,
    pending_delete: Option<Vec<String>>,
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
        self.transfers.cancel_all();
    }
}
impl Mokawa {
    fn request(&mut self, operation: Operation, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let generation = self.browser.begin_navigation();
        let parent = self
            .browser
            .path
            .last()
            .map(|p| p.0.clone())
            .unwrap_or_default();
        self.busy = true;
        self.status = "Connecting to PikPak…".into();
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
                        let count = files.len();
                        this.browser.apply(generation, files);
                        this.status = format!("{count} items · PikPak");
                    }
                    Err(error) => {
                        this.browser.files.clear();
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.page != Page::Files {
            return self.secondary_page(cx).into_any_element();
        }
        let files = self
            .browser
            .visible(self.search.read(cx).content.as_ref(), self.descending);
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
                    Self::button("All files").on_click(cx.listener(|this, _, _, cx| {
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
            .overflow_y_scroll();
        if files.is_empty() {
            content = content.child(div().p_8().text_color(rgb(0x9da6be)).child(if self.busy {
                "Loading your files…"
            } else if self.browser.files.is_empty() {
                "No files loaded. Connect your PikPak account, then refresh."
            } else {
                "No matching files."
            }));
        }
        for file in files {
            let id = file.id.clone();
            let name = file.name.clone();
            let folder = file.is_folder();
            let highlighted = self.browser.selected.contains(&id);
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
                .child(div().flex_1().child(file.name.clone()))
                .child(div().text_sm().text_color(rgb(0xa3acc3)).child(if folder {
                    "Folder".into()
                } else {
                    format!("{} bytes", file.size)
                }))
                .when(!self.grid, |row| {
                    row.child(
                        div()
                            .w(px(185.))
                            .text_sm()
                            .text_color(rgb(0xa3acc3))
                            .child(file.modified_time.clone()),
                    )
                })
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
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
            content = content.child(row);
        }
        let sidebar = self.navigation(cx);
        let toolbar = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .items_center()
            .child(div().w(px(240.)).child(self.search.clone()))
            .child(
                Self::button("Refresh")
                    .on_click(cx.listener(|this, _, _, cx| this.request(Operation::List, cx))),
            )
            .child(
                Self::button("Sort A ↔ Z").on_click(cx.listener(|this, _, _, cx| {
                    this.descending = !this.descending;
                    cx.notify();
                })),
            )
            .child(
                Self::button(if self.grid {
                    "Detailed rows"
                } else {
                    "Compact rows"
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.grid = !this.grid;
                    cx.notify();
                })),
            );
        let operations = div()
            .child(
                Self::button("Download")
                    .on_click(cx.listener(|this, _, _, cx| this.choose_download(cx))),
            )
            .flex()
            .flex_wrap()
            .gap_2()
            .items_center()
            .child(div().w(px(220.)).child(self.name.clone()))
            .child(
                Self::button("New folder").on_click(cx.listener(|this, _, _, cx| {
                    let name = this.name.read(cx).content.to_string();
                    this.request(Operation::Create(name), cx);
                })),
            )
            .child(
                Self::button("Rename selected").on_click(cx.listener(|this, _, _, cx| {
                    if this.browser.selected.len() == 1 {
                        let id = this.browser.selected.first().unwrap().clone();
                        let name = this.name.read(cx).content.to_string();
                        this.request(Operation::Rename(id, name), cx);
                    } else {
                        this.status = "Select exactly one file to rename.".into();
                        cx.notify();
                    }
                })),
            )
            .child(
                Self::button("Move to trash").on_click(cx.listener(|this, _, _, cx| {
                    if !this.browser.selected.is_empty() {
                        this.pending_delete = Some(this.browser.selected.iter().cloned().collect());
                        cx.notify();
                    }
                })),
            );
        let mut center = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .p_5()
            .flex()
            .flex_col()
            .gap_4()
            .child(crumbs)
            .child(toolbar)
            .child(operations);
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
                        "Move {} selected files to PikPak trash?",
                        ids.len()
                    ))
                    .child(
                        Self::button("Confirm trash").on_click(cx.listener(|this, _, _, cx| {
                            if !this.busy {
                                if let Some(ids) = this.pending_delete.take() {
                                    this.request(Operation::Trash(ids), cx);
                                }
                            }
                        })),
                    )
                    .child(
                        Self::button("Cancel").on_click(cx.listener(|this, _, _, cx| {
                            this.pending_delete = None;
                            cx.notify();
                        })),
                    ),
            );
        }
        center = center.child(content).child(
            div()
                .text_sm()
                .text_color(rgb(0xaab3ce))
                .child(self.status.clone()),
        );
        let mut detail = div()
            .w(px(220.))
            .flex_shrink_0()
            .h_full()
            .p_5()
            .bg(rgb(0x141621))
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_lg().child("File details"));
        detail = if let Some(file) = selected {
            detail
                .child(file.name)
                .child(format!("{} bytes", file.size))
                .child(file.mime_type)
                .child(file.modified_time)
        } else {
            detail.child(div().text_sm().text_color(rgb(0x838ba4)).child(
                "Select a file to see its information. Ctrl-click to select multiple items.",
            ))
        };
        div()
            .size_full()
            .flex()
            .bg(rgb(0x191c28))
            .text_color(rgb(0xe4e7f2))
            .font_family("Segoe UI")
            .on_action(cx.listener(|this, _: &Refresh, _, cx| this.request(Operation::List, cx)))
            .child(sidebar)
            .child(center)
            .when(
                !self.hide_details && _window.viewport_size().width >= px(1120.),
                |root| root.child(detail),
            )
            .into_any_element()
    }
}
pub fn run() {
    let runtime = match tokio::runtime::Runtime::new() {
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
            KeyBinding::new("alt-f4", Quit, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        let bounds = Bounds::centered(None, size(px(1200.), px(780.)), cx);
        let result = cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Mokawa".into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(760.), px(560.))),
                window_background: gpui::WindowBackgroundAppearance::Opaque,
                ..Default::default()
            },
            |_, cx| {
                let search = cx.new(|cx| TextInput::new("Search current folder…", cx));
                let name = cx.new(|cx| TextInput::new("New name…", cx));
                cx.new(|cx| {
                    let subscription = cx.observe(&search, |_, _, cx| cx.notify());
                    let mut view = Mokawa {
                        browser: BrowserState::default(),
                        page: Page::Login,
                        transfers: Mokawa::new_transfers(),
                        hide_details: false,
                        ready: false,
                        saving_preferences: false,
                        search,
                        name,
                        _subscription: subscription,
                        runtime: runtime.clone(),
                        status: "Not connected · Official authorization integration is pending."
                            .into(),
                        busy: false,
                        descending: false,
                        grid: false,
                        pending_delete: None,
                        task: None,
                    };
                    view.start_background(cx);
                    let center = view.transfers.clone();
                    let runtime = view.runtime.clone();
                    cx.on_app_quit(move |_| {
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
                .timer(std::time::Duration::from_secs(2));
            cx.spawn(async move |cx| {
                timer.await;
                let _ = cx.update(|cx| cx.quit());
            })
            .detach();
        }
        cx.activate(true);
    });
}
