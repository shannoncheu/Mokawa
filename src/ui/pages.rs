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
    Files,
    Tasks,
    Settings,
}
impl Mokawa {
    pub(super) fn navigation(&self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .w(px(180.))
            .flex_shrink_0()
            .h_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .bg(rgb(0x141621))
            .child(
                div()
                    .text_2xl()
                    .text_color(rgb(0xe2e6ff))
                    .mb_2()
                    .child("Mokawa"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x838ba4))
                    .mb_4()
                    .child("YOUR PIKPAK, AT EASE"),
            )
            .children(
                [
                    ("Account", Page::Login),
                    ("All files", Page::Files),
                    ("Task center", Page::Tasks),
                    ("Settings", Page::Settings),
                ]
                .into_iter()
                .map(|(label, page)| {
                    Self::button(label)
                        .id(SharedString::from(format!("nav-{label}")))
                        .when(self.page == page, |button| button.bg(rgb(0x3b4058)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.page = page;
                            this.pending_delete = None;
                            cx.notify();
                        }))
                }),
            )
            .child(
                div()
                    .mt_4()
                    .text_xs()
                    .text_color(rgb(0x6c758d))
                    .child("Offline tasks, shares and trash management are not available yet."),
            )
            .child(div().flex_1())
            .child(
                Self::button("Sign out").on_click(cx.listener(|this, _, _, cx| {
                    if let Some(task) = this.task.take() {
                        task.abort();
                    }
                    this.transfers.cancel_all();
                    this.busy = false;
                    this.browser.begin_navigation();
                    this.browser.files.clear();
                    this.browser.path.clear();
                    this.pending_delete = None;
                    this.page = Page::Login;
                    this.status = match credentials::delete() {
                        Ok(()) => "Local session removed.".into(),
                        Err(error) => error.to_string(),
                    };
                    cx.notify();
                })),
            )
            .child(
                div()
                    .text_xs()
                    .mt_4()
                    .text_color(rgb(0x838ba4))
                    .child("A calmer way to manage your files."),
            )
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
        body=match self.page {
            Page::Login=>body
                .child(div().text_3xl().child("Your files. A little calmer."))
                .child(div().text_color(rgb(0xaab3ce)).child("Connect your PikPak account to Mokawa."))
                .child(div().p_5().rounded_lg().bg(rgb(0x202331)).flex().flex_col().gap_4()
                    .child("PikPak now offers official connected-app authorization.")
                    .child(div().text_sm().text_color(rgb(0xaab3ce)).child("Mokawa's desktop authorization integration is still being verified. This build does not ask for your PikPak password or an exported token."))
                    .child(Self::button("About PikPak connected apps").on_click(|_,_,cx|cx.open_url("https://mypikpak.com/en-US/help-center/connected_apps/managing_connected_apps/what_are_connected_apps")))
                    .child(Self::button("Open saved account").on_click(cx.listener(|this,_,_,cx|{this.page=Page::Files;this.request(Operation::List,cx);}))))
                .child(div().text_sm().text_color(rgb(0x838ba4)).child("A saved session uses Windows Credential Manager. No account data is sent to a Mokawa server.")),
            Page::Settings=>body
                .child(div().text_3xl().child("Settings"))
                .child("Appearance")
                .child(div().text_sm().text_color(rgb(0xaab3ce)).child("Dark · System window frame · Solid background"))
                .child(Self::button(if self.grid{"Use detailed rows"}else{"Use compact rows"}).on_click(cx.listener(|this,_,_,cx|{this.grid=!this.grid;cx.notify();})))
                .child(Self::button(if self.hide_details{"Show file details"}else{"Hide file details"}).on_click(cx.listener(|this,_,_,cx|{this.hide_details=!this.hide_details;cx.notify();})))
                .child(Self::button("Save preferences").on_click(cx.listener(|this,_,_,cx|{
                    if this.saving_preferences{return;}this.saving_preferences=true;
                    let preferences=Preferences{compact_rows:this.grid,hide_details:this.hide_details};
                    let job=this.runtime.spawn(async move{preferences.save(&mokawa::settings::data_dir().join("preferences.json")).await});
                    cx.spawn(async move|this,cx|{let result=job.await.unwrap_or(Err(Error::Cancelled));let _=this.update(cx,|this,cx|{this.saving_preferences=false;this.status=match result{Ok(())=>"Preferences saved.".into(),Err(error)=>error.to_string()};cx.notify();});}).detach();
                })))
                .child(div().mt_4().text_lg().child("Mokawa 0.1.0"))
                .child(div().text_sm().text_color(rgb(0xaab3ce)).child("Windows 10/11 x64 · GPUI · PikPak only"))
                .child(div().text_sm().text_color(rgb(0x838ba4)).child("This development build is unsigned. Background blur and media playback have not been enabled.")),
            Page::Tasks=>{
                let snapshot=self.transfers.snapshot();
                body=body.child(div().text_3xl().child("Task center"))
                    .child(div().text_sm().text_color(rgb(0xaab3ce)).child("Downloads · Up to two at a time. Pause and resume are unavailable."));
                if !self.ready {body=body.child("Loading saved tasks…");}
                else if snapshot.records.is_empty(){body=body.child(div().p_8().rounded_lg().bg(rgb(0x202331)).child("No downloads yet. Select a file, then choose Download."));}
                if let Some(error)=snapshot.persistence_error{body=body.child(format!("Task history could not be saved: {error}"));}
                for record in snapshot.records.iter().rev(){
                    let id=record.id.clone();
                    let phase=match record.phase{TransferPhase::Queued=>"Queued",TransferPhase::Running=>"Downloading",TransferPhase::Interrupted=>"Interrupted · start a new download",TransferPhase::Completed=>"Completed",TransferPhase::Cancelled=>"Cancelled",TransferPhase::Failed=>"Failed"};
                    let mut row=div().p_4().rounded_lg().bg(rgb(0x202331)).flex().flex_col().gap_2().child(record.file_name.clone()).child(format!("{phase} · {} bytes received",record.completed));
                    if let Some(total)=record.total {if total>0 {row=row.child(div().h(px(4.)).w_full().bg(rgb(0x30374f)).child(div().h_full().w(gpui::relative((record.completed as f32/total as f32).clamp(0.,1.))).bg(rgb(0x949ce5))));}}
                    if let Some(speed)=snapshot.speeds.get(&id){row=row.child(format!("{speed} bytes/s average"));}
                    if let Some(error)=snapshot.errors.get(&id){row=row.child(error.clone());}
                    if matches!(record.phase,TransferPhase::Queued|TransferPhase::Running){row=row.child(Self::button("Cancel download").id(SharedString::from(format!("cancel-{id}"))).on_click(cx.listener(move|this,_,_,cx|{this.transfers.cancel(&id);cx.notify();})));}
                    body=body.child(row);
                }
                body.child(div().text_sm().text_color(rgb(0x838ba4)).child("Uploads and offline cloud tasks are not implemented in this build."))
            },
            Page::Files=>body,
        };
        div()
            .size_full()
            .flex()
            .bg(rgb(0x191c28))
            .text_color(rgb(0xe4e7f2))
            .font_family("Segoe UI")
            .child(self.navigation(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(body)
                    .child(
                        div()
                            .p_4()
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
            self.status = "Select one file to download.".into();
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
            self.status = "Choose a file rather than a folder.".into();
            cx.notify();
            return;
        };
        let epoch = credentials::generation();
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose download folder".into()),
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
                        let api = Api::new()?;
                        let mut session = credentials::load()?;
                        if session.expired() {
                            session = api.refresh(&Registration::from_env()?, &session).await?;
                            credentials::save_if_current(&session, epoch)?;
                        }
                        let details = api.file_details(&session, &file.id).await?;
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
                        center
                            .run(record, details.web_content_link, directory)
                            .await;
                        Ok(())
                    });
                    this.page = Page::Tasks;
                    this.status = "Preparing download…".into();
                    cx.spawn(async move |this, cx| {
                        let result: Result<()> = job.await.unwrap_or(Err(Error::Cancelled));
                        let _ = this.update(cx, |this, cx| {
                            this.status = match result {
                                Ok(()) => "Download task finished. See its status above.".into(),
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
            (tasks, preferences)
        });
        cx.spawn(async move |this, cx| {
            if let Ok((tasks, preferences)) = job.await {
                let _ = this.update(cx, |this, cx| {
                    this.ready = tasks.is_ok();
                    if let Err(error) = tasks {
                        this.status = format!("Task history unavailable: {error}");
                    }
                    match preferences {
                        Ok(p) => {
                            this.grid = p.compact_rows;
                            this.hide_details = p.hide_details;
                        }
                        Err(error) => this.status = format!("Preferences unavailable: {error}"),
                    };
                    cx.notify();
                });
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            loop {
                let timer = cx
                    .background_executor()
                    .timer(std::time::Duration::from_millis(250));
                timer.await;
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
