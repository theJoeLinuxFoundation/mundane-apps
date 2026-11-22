#[cfg(test)]
mod tests;
mod view;

use gpui::{prelude::*, *};
use mundane_services::packages::{
    Applied, Dave, Info, Listed, Operation, RefusalReason, Request, Review, Status,
};
use mundane_ui::{ApplicationContent, TextField, Theme, WindowHeader};
use std::time::Duration;

const PAGE_SIZE: usize = 6;

enum Page {
    Installed(Vec<Listed>),
    Results(String, mundane_services::packages::Search),
    Details(Info),
    Status(Status),
}
enum Job {
    Installed,
    Search(String),
    Info(String),
    Status,
    Preview(Request),
    Apply(Review),
}
enum Answer {
    Page(Page),
    Review(Review, String),
    Notice(String),
    Changed(String, anyhow::Result<Page>),
}

impl Job {
    fn execute(self, dave: &Dave) -> anyhow::Result<Answer> {
        Ok(match self {
            Self::Installed => Answer::Page(Page::Installed(dave.list()?)),
            Self::Search(term) => Answer::Page(Page::Results(term.clone(), dave.search(&term)?)),
            Self::Info(name) => Answer::Page(Page::Details(dave.info(&name)?)),
            Self::Status => Answer::Page(Page::Status(dave.status()?)),
            Self::Preview(req) => Answer::Review(dave.preview(req)?, String::new()),
            Self::Apply(review) => {
                let request = review.request().clone();
                let digest = review.plan().digest.clone();
                match dave.apply(review)? {
                    Applied::Dismissed => Answer::Notice("Authorization dismissed.".into()),
                    Applied::Denied => {
                        Answer::Notice("Authorization denied or unavailable.".into())
                    }
                    Applied::Refused(refusal) => {
                        let message = refusal.message;
                        let notice = format!("dave refused the change. {message}");
                        if refusal.reason == RefusalReason::Other {
                            return Ok(Answer::Notice(notice));
                        }
                        match dave.preview(request) {
                            Ok(next)
                                if refusal.reason == RefusalReason::Changed
                                    || next.plan().digest != digest =>
                            {
                                Answer::Review(
                                    next,
                                    format!("The plan changed. Review it again. {message}"),
                                )
                            }
                            Ok(_) => Answer::Notice(notice),
                            Err(error) => Answer::Notice(format!(
                                "{notice} Could not reload the plan. {error}"
                            )),
                        }
                    }
                    Applied::Success { rollback } => {
                        let (mut notice, page) = if request.operation() == Operation::Upgrade {
                            let status = dave.status();
                            let message = match &status {
                                Ok(s) if !s.managed => "System packages upgraded.",
                                Ok(s) if s.prepared_upgrade.is_some() => {
                                    "Upgrade prepared for shutdown."
                                }
                                _ => "dave accepted the upgrade.",
                            };
                            (message.to_owned(), status.map(Page::Status))
                        } else {
                            (
                                format!("{} completed.", request.title()),
                                dave.list().map(Page::Installed),
                            )
                        };
                        if let Some(id) = rollback {
                            notice.push_str(&format!(" Rollback point {id}."));
                        }
                        Answer::Changed(notice, page)
                    }
                }
            }
        })
    }
}
struct Packages {
    dave: Dave,
    page: Page,
    previous_search: Option<String>,
    offset: usize,
    review: Option<Review>,
    review_offset: usize,
    review_message: String,
    busy: bool,
    notice: String,
    field: Entity<TextField>,
    header: WindowHeader,
    content: ApplicationContent,
    theme: Theme,
    installed: FocusHandle,
    updates: FocusHandle,
    submit: FocusHandle,
    action: FocusHandle,
    back: FocusHandle,
    rows: [FocusHandle; PAGE_SIZE],
    previous: FocusHandle,
    next: FocusHandle,
    cancel: FocusHandle,
    confirm: FocusHandle,
    dialog_prev: FocusHandle,
    dialog_next: FocusHandle,
    scroll: ScrollHandle,
}

impl Packages {
    fn new(dave: Dave, w: &mut Window, cx: &mut Context<Self>) -> Self {
        let theme = Theme::current();
        let field = cx.new(|cx| TextField::new("Search packages", theme, cx));
        let focus = || cx.focus_handle().tab_stop(true);
        let mut this = Self {
            dave,
            page: Page::Installed(vec![]),
            previous_search: None,
            offset: 0,
            review: None,
            review_offset: 0,
            review_message: String::new(),
            busy: false,
            notice: String::new(),
            header: WindowHeader::new(cx),
            content: Default::default(),
            theme,
            field,
            installed: focus(),
            updates: focus(),
            submit: focus(),
            action: focus(),
            back: focus(),
            rows: std::array::from_fn(|_| focus()),
            previous: focus(),
            next: focus(),
            cancel: cx.focus_handle(),
            confirm: cx.focus_handle(),
            dialog_prev: cx.focus_handle(),
            dialog_next: cx.focus_handle(),
            scroll: ScrollHandle::new(),
        };
        this.field.focus_handle(cx).focus(w, cx);
        this.start(Job::Installed, w, cx);
        this
    }

    fn start(&mut self, job: Job, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.review.is_some() {
            return;
        }
        self.busy = true;
        self.notice = match job {
            Job::Apply(_) => "Waiting for package authorization…",
            _ => "Loading…",
        }
        .into();
        let dave = self.dave.clone();
        let work = cx
            .background_executor()
            .spawn(async move { job.execute(&dave) });
        cx.spawn_in(w, async move |view, cx| {
            let result = work.await;
            let _ = view.update_in(cx, |this, w, cx| {
                this.busy = false;
                this.notice.clear();
                match result {
                    Err(error) => this.notice = format!("{error:#}"),
                    Ok(Answer::Notice(text)) => this.notice = text,
                    Ok(Answer::Review(review, message)) => {
                        this.review_offset = 0;
                        this.review_message = message;
                        this.review = Some(review);
                        this.cancel.focus(w, cx);
                    }
                    Ok(Answer::Page(page)) => this.show(page, w, cx),
                    Ok(Answer::Changed(text, page)) => {
                        this.notice = text;
                        match page {
                            Ok(page) => this.show(page, w, cx),
                            Err(e) => this.notice.push_str(&format!(" Refresh failed. {e}")),
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn show(&mut self, page: Page, w: &mut Window, cx: &mut Context<Self>) {
        self.offset = 0;
        self.scroll.set_offset(point(px(0.), px(0.)));
        match &page {
            Page::Results(term, results) => {
                self.previous_search = Some(term.clone());
                if !results.packages.is_empty() {
                    self.rows[0].focus(w, cx)
                } else {
                    self.field.focus_handle(cx).focus(w, cx)
                }
            }
            Page::Installed(_) => {
                self.previous_search = None;
                if !self.field.focus_handle(cx).is_focused(w) {
                    self.installed.focus(w, cx)
                }
            }
            Page::Details(_) | Page::Status(_) => self.action.focus(w, cx),
        }
        self.page = page;
    }
    fn search(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let term = self.field.read(cx).text().trim().to_owned();
        self.start(
            if term.is_empty() {
                Job::Installed
            } else {
                Job::Search(term)
            },
            w,
            cx,
        )
    }
    fn back(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.start(
            self.previous_search
                .clone()
                .map_or(Job::Installed, Job::Search),
            w,
            cx,
        );
    }
    fn preview(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let req = match &self.page {
            Page::Details(i) => Request::package(
                if i.installed.is_some() {
                    Operation::Remove
                } else {
                    Operation::Install
                },
                &i.name,
            ),
            Page::Status(_) => Ok(Request::upgrade()),
            _ => return,
        };
        match req {
            Ok(req) => self.start(Job::Preview(req), w, cx),
            Err(e) => {
                self.notice = e.to_string();
                cx.notify()
            }
        }
    }
    fn finish(&mut self, yes: bool, w: &mut Window, cx: &mut Context<Self>) {
        let Some(review) = self.review.take() else {
            return;
        };
        self.action.focus(w, cx);
        if yes && review.plan().digest.is_some() {
            self.start(Job::Apply(review), w, cx)
        }
        cx.notify()
    }
    fn refresh(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let job = match &self.page {
            Page::Installed(_) => Job::Installed,
            Page::Results(t, _) => Job::Search(t.clone()),
            Page::Details(i) => Job::Info(i.name.clone()),
            Page::Status(_) => Job::Status,
        };
        self.start(job, w, cx)
    }
}

pub fn run(dave: Dave) {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("Packages requires a Wayland session.");
        std::process::exit(1)
    }
    Application::with_platform(gpui_linux::current_platform(false)).run(move |cx| {
        mundane_ui::bind_text_keys(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit()
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(780.), px(550.)), cx);
        if let Err(e) = cx.open_window(
            WindowOptions {
                app_id: Some("org.joelinux.mundane.Packages".into()),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            |w, cx| {
                w.set_window_title("Packages");
                let view = cx.new(|cx| Packages::new(dave, w, cx));
                let weak = view.downgrade();
                cx.spawn(async move |cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_secs(15))
                            .await;
                        // Poll without stealing focus or blocking a user's review.
                        let Ok(Some((source, revision))) =
                            weak.read_with(cx, |v, _| match &v.page {
                                Page::Status(s) if !v.busy && v.review.is_none() => {
                                    Some((v.dave.clone(), s.revision.clone()))
                                }
                                _ => None,
                            })
                        else {
                            if !weak.is_upgradable() {
                                break;
                            }
                            continue;
                        };
                        let status = cx
                            .background_executor()
                            .spawn(async move { source.status() })
                            .await;
                        if weak
                            .update(cx, |v, cx| {
                                if !v.busy
                                    && v.review.is_none()
                                    && matches!(&v.page,Page::Status(s) if s.revision==revision)
                                {
                                    match status {
                                        Ok(s) if s.revision != revision => {
                                            v.page = Page::Status(s);
                                            cx.notify()
                                        }
                                        Err(e) => {
                                            v.notice = format!("Status refresh failed. {e}");
                                            cx.notify()
                                        }
                                        _ => {}
                                    }
                                }
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                })
                .detach();
                view
            },
        ) {
            eprintln!("Could not open Packages. {e}");
            std::process::exit(1)
        }
    });
}
