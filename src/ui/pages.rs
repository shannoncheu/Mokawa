use super::*;
use gpui::PathPromptOptions;
use mokawa::{
    models::{TransferPhase, TransferRecord},
    settings::Preferences,
    transfers::TransferCenter,
};
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Page {
    Login,
    Home,
    Files,
    Recent,
    Favorites,
    Offline,
    Shares,
    Trash,
    Tasks,
    Settings,
}
impl Mokawa {
    pub(super) fn titlebar(&self, window: &Window) -> gpui::Div {
        use gpui::WindowControlArea;
        let control = |id: &'static str, glyph: &'static str, area| {
            div()
                .id(id)
                .w(px(46.))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .window_control_area(area)
                .hover(|s| s.bg(rgb(0x3b4058)))
                .child(glyph)
        };
        div()
            .h(px(44.))
            .flex_shrink_0()
            .w_full()
            .flex()
            .items_center()
            .bg(rgb(0x141621))
            .child(
                div()
                    .id("window-drag")
                    .flex_1()
                    .h_full()
                    .px_5()
                    .flex()
                    .items_center()
                    .gap_3()
                    .window_control_area(WindowControlArea::Drag)
                    .child(
                        div()
                            .size(px(24.))
                            .rounded_lg()
                            .bg(rgb(0x949ce5))
                            .text_color(gpui::white())
                            .flex()
                            .items_center()
                            .justify_center()
                            .child("M"),
                    )
                    .child(div().text_sm().child("Mokawa")),
            )
            .child(control("minimize", "−", WindowControlArea::Min))
            .child(control(
                "maximize",
                if window.is_maximized() { "❐" } else { "□" },
                WindowControlArea::Max,
            ))
            .child(
                control("close", "×", WindowControlArea::Close)
                    .hover(|s| s.bg(gpui::rgb(0xc42b42)).text_color(gpui::white())),
            )
    }
    pub(super) fn navigation(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut nav = div()
            .w(px(196.))
            .flex_shrink_0()
            .h_full()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .bg(rgb(0x141621));
        for (icon, label, page) in [
            ("⌂", "首页", Page::Home),
            ("▤", "我的文件", Page::Files),
            ("◷", "最近访问", Page::Recent),
            ("☆", "收藏", Page::Favorites),
            ("↓", "离线下载", Page::Offline),
            ("↗", "分享管理", Page::Shares),
            ("▢", "回收站", Page::Trash),
            ("≡", "任务中心", Page::Tasks),
            ("⚙", "设置", Page::Settings),
        ] {
            nav = nav.child(
                Self::button(label)
                    .py_1()
                    .w_full()
                    .flex()
                    .gap_3()
                    .child(icon)
                    .when(self.page == page, |s| s.bg(rgb(0x30374f)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.pending_delete = None;
                        this.page = if !this.authenticated
                            && !matches!(page, Page::Settings | Page::Tasks)
                        {
                            Page::Login
                        } else {
                            page
                        };
                        cx.notify();
                    })),
            );
        }
        nav = nav.child(div().flex_1());
        if self.authenticated {
            nav = nav.child(
                Self::button("退出登录").on_click(cx.listener(|this, _, _, cx| this.sign_out(cx))),
            );
        } else {
            nav = nav.child(Self::button("连接 PikPak 账号").on_click(cx.listener(
                |this, _, _, cx| {
                    this.page = Page::Login;
                    cx.notify();
                },
            )));
        }
        nav.child(
            div()
                .text_xs()
                .p_2()
                .text_color(rgb(0x838ba4))
                .child("Mokawa · 0.1.0"),
        )
    }
    fn sign_out(&mut self, cx: &mut Context<Self>) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.download_scope.cancel();
        self.download_scope = tokio_util::sync::CancellationToken::new();
        self.transfers.cancel_all();
        self.authenticated = false;
        self.logging_out = true;
        self.browser.begin_navigation();
        self.browser.files.clear();
        self.browser.path.clear();
        self.pending_delete = None;
        self.page = Page::Login;
        self.busy = true;
        self.status = "正在清理本地会话…".into();
        let job = self.runtime.spawn_blocking(credentials::delete);
        cx.spawn(async move |this, cx| {
            let result = job.await.unwrap_or(Err(Error::Cancelled));
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.logging_out = false;
                this.status = match result {
                    Ok(()) => {
                        this.has_saved_session = false;
                        "已退出登录，本地会话已删除。".into()
                    }
                    Err(error) => format!("本地会话清理失败：{error} 请重试。"),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn save_preferences(&mut self, cx: &mut Context<Self>) {
        if self.saving_preferences {
            return;
        }
        self.saving_preferences = true;
        let preferences = Preferences {
            compact_rows: self.grid,
            hide_details: self.hide_details,
            theme: self.theme_mode,
        };
        let job = self.runtime.spawn(async move {
            preferences
                .save(&mokawa::settings::data_dir().join("preferences.json"))
                .await
        });
        cx.spawn(async move |this, cx| {
            let result = job.await.unwrap_or(Err(Error::Cancelled));
            let _ = this.update(cx, |this, cx| {
                this.saving_preferences = false;
                this.status = match result {
                    Ok(()) => "设置已保存。".into(),
                    Err(error) => format!("设置保存失败：{error}"),
                };
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn secondary_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut body = div()
            .id("page-body")
            .flex_1()
            .min_w_0()
            .h_full()
            .p_8()
            .flex()
            .flex_col()
            .gap_4()
            .overflow_y_scroll();
        body = match self.page {
            Page::Login => body.items_center().justify_start()
                .child(div().w_full().max_w(px(480.)).p_6().rounded_xl().bg(rgb(0x191c28)).flex().flex_col().gap_4()
                    .child(div().text_color(rgb(0x949ce5)).text_3xl().child("Mokawa"))
                    .child(div().text_2xl().child("连接 PikPak 账号"))
                    .child(div().text_sm().text_color(rgb(0xaab3ce)).child("使用 PikPak 官方授权连接网盘，凭据仅保存在 Windows 凭据管理器中。"))
                    .child(Self::button(if self.logging_out { "正在清理会话…" } else if self.busy { "正在验证会话…" } else { "使用 PikPak 授权登录" }).bg(rgb(0x30374f)).on_click(cx.listener(|this, _, _, cx| {
                        if !this.busy { this.status = Error::AuthorizationPending.to_string(); cx.notify(); }
                    })))
                    .child(div().p_4().rounded_lg().bg(rgb(0x202331)).text_sm().child("授权尚未开放：Mokawa 的官方客户端注册及回调规则仍待确认。当前无法新增登录，不提供未经确认的密码登录。"))
                    .child(Self::button("重试已保存的会话").on_click(cx.listener(|this, _, _, cx| this.request(Operation::List, cx))))
                    .when(self.busy && !self.logging_out, |card| card.child(Self::button("取消会话验证").on_click(cx.listener(|this, _, _, cx| {
                        if this.busy { if let Some(task) = this.task.take() { task.abort(); } this.browser.begin_navigation(); this.busy = false; this.status = "已取消会话验证。".into(); cx.notify(); }
                    }))))
                    .when(self.has_saved_session && !self.busy, |card| card.child(Self::button("清除保存的会话").on_click(cx.listener(|this, _, _, cx| this.sign_out(cx)))))
                    .child(Self::button("外观设置").on_click(cx.listener(|this, _, _, cx| { this.page = Page::Settings; cx.notify(); })))
                    .child(Self::button("查看 PikPak 官方授权说明").on_click(|_, _, cx| cx.open_url("https://mypikpak.com/en-US/help-center/connected_apps/managing_connected_apps/what_are_connected_apps")))) ,
            Page::Settings => {
                body = body.child(div().text_2xl().child("设置")).child(div().text_lg().child("外观主题"))
                    .child(div().text_sm().text_color(rgb(0xaab3ce)).child("主题立即生效并自动保存；跟随系统时响应 Windows 外观变化。"));
                let mut choices = div().flex().gap_3();
                for (label, mode) in [("跟随系统", mokawa::settings::ThemeMode::System), ("浅色", mokawa::settings::ThemeMode::Light), ("深色", mokawa::settings::ThemeMode::Dark)] {
                    choices = choices.child(Self::button(label).when(self.theme_mode == mode, |s| s.bg(rgb(0x30374f)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this.saving_preferences { return; }
                            this.theme_mode = mode; theme::apply(mode, window.appearance());
                            this.search.update(cx, |_, cx| cx.notify()); this.name.update(cx, |_, cx| cx.notify());
                            this.save_preferences(cx); cx.notify();
                        })));
                }
                body.child(choices).child(div().mt_4().text_lg().child("文件管理"))
                    .child(Self::button(if self.hide_details { "显示所选文件详情" } else { "隐藏所选文件详情" }).on_click(cx.listener(|this, _, _, cx| { this.hide_details = !this.hide_details; cx.notify(); })))
                    .child(Self::button("保存设置").on_click(cx.listener(|this, _, _, cx| this.save_preferences(cx))))
                    .child(div().mt_4().text_lg().child("关于 Mokawa"))
                    .child(div().text_sm().text_color(rgb(0xaab3ce)).child("版本 0.1.0 · Windows 10/11 x64 · 仅支持 PikPak"))
                    .child(div().text_sm().text_color(rgb(0x838ba4)).child("开发预览版，尚未签名。授权、上传和媒体播放尚未接入。"))
            },
            Page::Tasks => {
                let snapshot = self.transfers.snapshot();
                body = body.child(div().text_2xl().child("任务中心")).child(div().text_sm().text_color(rgb(0xaab3ce)).child("下载队列 · 最多同时下载 2 个文件，暂不支持暂停和续传。"));
                if !self.ready { body = body.child("正在读取任务…"); }
                else if snapshot.records.is_empty() { body = body.child(div().p_8().rounded_lg().bg(rgb(0x202331)).child("暂无下载任务。登录后，选择文件并点击下载。")); }
                if let Some(error) = snapshot.persistence_error { body = body.child(format!("任务记录保存失败：{error}")); }
                for record in snapshot.records.iter().rev() {
                    let id = record.id.clone();
                    let phase = match record.phase { TransferPhase::Queued => "排队中", TransferPhase::Running => "下载中", TransferPhase::Interrupted => "已中断，请重新下载", TransferPhase::Completed => "已完成", TransferPhase::Cancelled => "已取消", TransferPhase::Failed => "失败" };
                    let mut row = div().p_4().rounded_lg().bg(rgb(0x202331)).flex().flex_col().gap_2().child(record.file_name.clone()).child(format!("{phase} · 已接收 {} 字节", record.completed));
                    if let Some(total) = record.total && total > 0 { row = row.child(div().h(px(4.)).w_full().bg(rgb(0x30374f)).child(div().h_full().w(gpui::relative((record.completed as f32 / total as f32).clamp(0.,1.))).bg(rgb(0x949ce5)))); }
                    if let Some(speed) = snapshot.speeds.get(&id) { row = row.child(format!("平均 {speed} 字节/秒")); }
                    if let Some(error) = snapshot.errors.get(&id) { row = row.child(error.clone()); }
                    if matches!(record.phase, TransferPhase::Queued | TransferPhase::Running) { row = row.child(Self::button("取消下载").id(SharedString::from(format!("cancel-{id}"))).on_click(cx.listener(move |this, _, _, cx| { this.transfers.cancel(&id); cx.notify(); }))); }
                    body = body.child(row);
                }
                body
            },
            Page::Home => body.child(div().text_2xl().child("首页")).child("选择“我的文件”浏览 PikPak 网盘。"),
            Page::Recent | Page::Favorites | Page::Offline | Page::Shares | Page::Trash => {
                let title = match self.page { Page::Recent => "最近访问", Page::Favorites => "收藏", Page::Offline => "离线下载", Page::Shares => "分享管理", _ => "回收站" };
                body.child(div().text_2xl().child(title)).child(div().p_8().rounded_lg().bg(rgb(0x202331)).child("此功能尚未接入，当前无法操作。"))
            },
            Page::Files => body,
        };
        let shell = div()
            .size_full()
            .flex()
            .bg(rgb(0x141621))
            .text_color(rgb(0xe4e7f2));
        let shell = if self.page == Page::Login {
            shell
        } else {
            shell.child(self.navigation(cx))
        };
        shell.child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .child(body)
                .child(
                    div()
                        .px_8()
                        .py_3()
                        .text_sm()
                        .text_color(rgb(0xaab3ce))
                        .child(self.status.clone()),
                ),
        )
    }
    pub(super) fn choose_download(&mut self, cx: &mut Context<Self>) {
        if !self.ready || self.busy {
            return;
        }
        if self.browser.selected.len() != 1 {
            self.status = "请选择一个文件下载。".into();
            cx.notify();
            return;
        }
        let Some(file) = self
            .browser
            .files
            .iter()
            .find(|f| self.browser.selected.contains(&f.id) && !f.is_folder())
            .cloned()
        else {
            self.status = "请选择文件，暂不支持整文件夹下载。".into();
            cx.notify();
            return;
        };
        let Ok(admission) = self.download_admission.clone().try_acquire_owned() else {
            self.status = "下载队列已满，请等待现有任务完成。".into();
            cx.notify();
            return;
        };
        let preparation_slots = self.preparation_slots.clone();
        let epoch = credentials::generation();
        let download_scope = self.download_scope.child_token();
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("选择下载目录".into()),
        });
        cx.spawn(async move |this, cx| {
            let directory = match picker.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            if let Some(directory) = directory {
                let _ = this.update(cx, |this, cx| {
                    if credentials::generation() != epoch {
                        return;
                    }
                    let center = this.transfers.clone();
                    let job = this.runtime.spawn(async move {
                        let _admission = admission;
                        let preparation = tokio::select! {
                            _ = download_scope.cancelled() => return Err(Error::Cancelled),
                            slot = preparation_slots.acquire_owned() => slot.map_err(|_| Error::Cancelled)?,
                        };
                        let api = Api::new()?;
                        let mut session = credentials::load()?;
                        if session.expired() {
                            session = api.refresh(&Registration::from_env()?, &session).await?;
                            credentials::save_if_current(&session, epoch)?;
                        }
                        let details=tokio::select!{_ = download_scope.cancelled()=>return Err(Error::Cancelled),result=api.file_details(&session,&file.id)=>result?};
                        if credentials::generation() != epoch {
                            return Err(Error::Cancelled);
                        }
                        if details.web_content_link.is_empty() {
                            return Err(Error::Protocol);
                        }
                        let id = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos()
                            .to_string();
                        let record = TransferRecord {
                            id,
                            file_id: file.id,
                            file_name: file.name,
                            completed: 0,
                            total: Some(details.size),
                            phase: TransferPhase::Queued,
                        };
                        drop(preparation);
                        center
                            .run_cancellable(record, details.web_content_link, directory,download_scope)
                            .await?;
                        Ok(())
                    });
                    this.page = Page::Tasks;
                    this.status = "正在准备下载…".into();
                    cx.spawn(async move |this, cx| {
                        let result: Result<()> = job.await.unwrap_or(Err(Error::Cancelled));
                        let _ = this.update(cx, |this, cx| {
                            if credentials::generation()!=epoch{return;}
                            this.status = match result {
                                Ok(()) => "下载任务已结束，请查看任务状态。".into(),
                                Err(error) => error.to_string(),
                            };
                            cx.notify();
                        });
                    })
                    .detach();
                    cx.notify();
                });
            }
        })
        .detach();
    }
    pub(super) fn start_background(&mut self, cx: &mut Context<Self>) {
        let center = self.transfers.clone();
        let job = self.runtime.spawn(async move {
            let tasks = center.restore().await;
            let preferences =
                Preferences::load(&mokawa::settings::data_dir().join("preferences.json")).await;
            let saved_session = credentials::load();
            (tasks, preferences, saved_session)
        });
        cx.spawn(async move |this, cx| {
            if let Ok((tasks, preferences, saved_session)) = job.await {
                let _ = this.update(cx, |this, cx| {
                    this.ready = tasks.is_ok();
                    if let Err(error) = tasks {
                        this.status = format!("无法读取任务记录：{error}");
                    }
                    match preferences {
                        Ok(p) => {
                            this.grid = p.compact_rows;
                            this.hide_details = p.hide_details;
                            this.theme_mode = smoke_theme().unwrap_or(p.theme);
                        }
                        Err(error) => this.status = format!("无法读取设置：{error}"),
                    };
                    match saved_session {
                        Ok(_) => {
                            this.has_saved_session = true;
                            if this.page == Page::Login && !this.busy {
                                this.request(Operation::List, cx);
                            }
                        }
                        Err(Error::NoSession) => {}
                        Err(error) => {
                            this.has_saved_session = matches!(error, Error::InvalidSession);
                            this.status = error.to_string();
                        }
                    }
                    this.search.update(cx, |_, cx| cx.notify());
                    this.name.update(cx, |_, cx| cx.notify());
                    cx.notify();
                });
            }
        })
        .detach();
        let mut changes = self.transfers.subscribe();
        let lifetime = self.lifetime.clone();
        cx.spawn(async move |this, cx| {
            loop {
                tokio::select! {
                    _ = lifetime.cancelled() => break,
                    changed = changes.changed() => if changed.is_err() { break; },
                }
                // Coalesce network progress updates; no timer exists while idle.
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        if this.page == Page::Tasks {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }
    pub(super) fn new_transfers() -> TransferCenter {
        TransferCenter::new(mokawa::settings::data_dir().join("downloads.json"))
    }
}
