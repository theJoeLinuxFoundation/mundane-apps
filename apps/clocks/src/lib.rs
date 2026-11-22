use gpui::{prelude::*, *};
use mundane_services::clocks::{self, City, Countdown, Notification};
use mundane_ui::{
    BrowserList, BrowserRow, ControlRole, Role, TextField, Theme, WindowHeader, browser_key,
    browser_window, button, heading,
};
use std::{path::PathBuf, time::Duration};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    World,
    Search,
    Timer,
}
struct Clocks {
    page: Page,
    theme: Theme,
    header: WindowHeader,
    list: BrowserList,
    search: Entity<TextField>,
    duration: Entity<TextField>,
    navigation: FocusHandle,
    action: FocusHandle,
    timer_link: FocusHandle,
    saving: bool,
    zones: Vec<String>,
    cities: Vec<City>,
    path: Option<PathBuf>,
    fixed: Option<i64>,
    minute: i64,
    countdown: Countdown,
    remaining: String,
    notice: String,
    notification: Option<Notification>,
    generation: u64,
}
impl Clocks {
    fn new(
        fixed: Option<i64>,
        path: Option<PathBuf>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        mundane_ui::bind_text_keys(cx);
        let theme = Theme::current();
        let search = cx.new(|cx| TextField::new("Find a city or time zone", theme, cx));
        let duration =
            cx.new(|cx| TextField::new("Timer duration", theme, cx).select_value("5:00".into()));
        cx.observe(&search, |this, _, cx| {
            if this.page == Page::Search {
                this.refresh(cx)
            }
        })
        .detach();
        let list = BrowserList::new(cx);
        list.focus.focus(w, cx);
        let loaded = path.as_deref().map(clocks::load).transpose();
        let notice = loaded
            .as_ref()
            .err()
            .map(ToString::to_string)
            .unwrap_or_default();
        let cities = loaded
            .ok()
            .flatten()
            .unwrap_or_default()
            .iter()
            .filter_map(|s| City::get(s).ok())
            .collect();
        let mut this = Self {
            page: Page::World,
            theme,
            header: WindowHeader::new(cx),
            list,
            search,
            duration,
            navigation: cx.focus_handle().tab_stop(true),
            action: cx.focus_handle().tab_stop(true),
            timer_link: cx.focus_handle().tab_stop(true),
            saving: false,
            zones: clocks::zones(),
            cities,
            path,
            fixed,
            minute: 0,
            countdown: Countdown::Idle,
            remaining: String::new(),
            notice,
            notification: None,
            generation: 0,
        };
        this.refresh(cx);
        this
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let row = |zone: &str, detail: String, value: String| BrowserRow {
            key: zone.into(),
            label: clocks::city_name(zone),
            detail,
            value,
            depth: 0,
            fraction: None,
        };
        let wall = self.fixed.unwrap_or_else(clocks::wall_now);
        self.minute = wall.div_euclid(60);
        let rows = match self.page {
            Page::World => self
                .cities
                .iter()
                .map(|c| {
                    let (time, date) = c.display(wall);
                    row(&c.zone, date, time)
                })
                .collect(),
            Page::Search => {
                let q = self
                    .search
                    .read(cx)
                    .text()
                    .trim()
                    .to_lowercase()
                    .replace(' ', "_");
                self.zones
                    .iter()
                    .filter(|s| s.to_lowercase().contains(&q))
                    .map(|z| row(z, z.clone(), String::new()))
                    .collect()
            }
            Page::Timer => vec![],
        };
        self.list.replace(rows);
        cx.notify()
    }
    fn page(&mut self, page: Page, w: &mut Window, cx: &mut Context<Self>) {
        self.page = page;
        self.notice.clear();
        self.refresh(cx);
        match page {
            Page::Search => self.search.focus_handle(cx),
            Page::Timer if self.countdown == Countdown::Idle => self.duration.focus_handle(cx),
            Page::Timer => self.action.clone(),
            Page::World => self.list.focus.clone(),
        }
        .focus(w, cx)
    }
    fn city(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(zone) = self
            .list
            .selected
            .and_then(|i| self.list.rows.get(i))
            .and_then(|r| r.key.to_str())
        else {
            return;
        };
        let Some(path) = &self.path else {
            self.notice = "Settings directory unavailable.".into();
            cx.notify();
            return;
        };
        if self.saving {
            return;
        }
        self.saving = true;
        let path = path.clone();
        let zone = zone.to_owned();
        let page = self.page;
        let job = cx
            .background_executor()
            .spawn(async move { clocks::save_change(&path, &zone, page == Page::Search) });
        cx.spawn_in(w, async move |view, cx| {
            let result = job.await;
            let _ = view.update_in(cx, |this, w, cx| {
                this.saving = false;
                match result {
                    Ok(list) => {
                        this.cities = list.iter().filter_map(|z| City::get(z).ok()).collect();
                        if this.page == page {
                            this.page(Page::World, w, cx)
                        } else {
                            this.refresh(cx)
                        }
                    }
                    Err(e) => {
                        this.notice = e.to_string();
                        cx.notify()
                    }
                }
            });
        })
        .detach();
    }
    fn timer(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.generation += 1;
        if self.countdown == Countdown::Idle {
            let now = clocks::boot_now();
            if let Err(e) = self.countdown.start(self.duration.read(cx).text(), now) {
                self.notice = e.to_string();
                cx.notify();
                return;
            }
            self.remaining = self.countdown.remaining(now);
            self.action.focus(w, cx);
        } else {
            self.countdown = Countdown::Idle;
            if let Some(n) = self.notification.take() {
                cx.background_executor()
                    .spawn(async move {
                        let _ = n.close();
                    })
                    .detach()
            }
            self.duration.focus_handle(cx).focus(w, cx)
        }
        self.notice.clear();
        cx.notify()
    }
    fn tick(&mut self, now: Duration, w: &mut Window, cx: &mut Context<Self>) {
        if self.countdown.expire(now) {
            self.page(Page::Timer, w, cx);
            let token = self.generation;
            let job = cx
                .background_executor()
                .spawn(async { Notification::send() });
            cx.spawn(async move |view, cx| {
                if let Ok(n) = job.await {
                    let mut result = Some(n);
                    let _ = view.update(cx, |this, _| {
                        if this.generation == token {
                            this.notification = result.take()
                        }
                    });
                    if let Some(n) = result {
                        cx.background_executor()
                            .spawn(async move {
                                let _ = n.close();
                            })
                            .detach()
                    }
                }
            })
            .detach();
        }
        let remaining = self.countdown.remaining(now);
        if remaining != self.remaining {
            self.remaining = remaining;
            cx.notify()
        }
        if self.page == Page::World
            && self.fixed.unwrap_or_else(clocks::wall_now).div_euclid(60) != self.minute
        {
            self.refresh(cx)
        }
    }
    fn keys(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        match (k.key.as_str(), k.modifiers.control) {
            ("n", true) => self.page(Page::Search, w, cx),
            ("t", true) => self.page(Page::Timer, w, cx),
            ("escape", false) => self.page(Page::World, w, cx),
            ("enter", false)
                if self.page == Page::Search
                    && (self.search.focus_handle(cx).is_focused(w)
                        || self.list.focus.is_focused(w)) =>
            {
                self.city(w, cx)
            }
            ("enter", false)
                if self.page == Page::Timer
                    && (self.duration.focus_handle(cx).is_focused(w)
                        || self.action.is_focused(w)) =>
            {
                self.timer(w, cx)
            }
            ("delete", false) if self.page == Page::World && self.list.focus.is_focused(w) => {
                self.city(w, cx)
            }
            ("down", false)
                if self.page == Page::Search && self.search.focus_handle(cx).is_focused(w) =>
            {
                self.list.focus.focus(w, cx)
            }
            _ if self.list.focus.is_focused(w) && self.list.navigate(&k.key) => cx.notify(),
            _ if browser_key(k, w, cx) => {}
            _ => return,
        }
        w.prevent_default();
        cx.stop_propagation()
    }
}
impl Render for Clocks {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let world = self.page == Page::World;
        let search = self.page == Page::Search;
        let v = cx.weak_entity();
        let nav = button(
            if world { "Add city" } else { "World clocks" },
            &self.navigation,
            t,
            move |w, cx| {
                let _ = v.update(cx, |c, cx| {
                    c.page(if world { Page::Search } else { Page::World }, w, cx)
                });
            },
        );
        let v = cx.weak_entity();
        let tools = div()
            .flex()
            .gap_2()
            .child(nav)
            .when(self.page != Page::Timer, |e| {
                e.child(button("Timer", &self.timer_link, t, move |w, cx| {
                    let _ = v.update(cx, |c, cx| c.page(Page::Timer, w, cx));
                }))
            });
        let timer = self.page == Page::Timer;
        let action = if timer {
            match self.countdown {
                Countdown::Idle => "Start",
                Countdown::Running(_) => "Cancel timer",
                Countdown::Finished => "Dismiss",
            }
        } else if search {
            "Add selected city"
        } else {
            "Remove city"
        };
        let v = cx.weak_entity();
        let enabled = timer || self.list.selected.is_some();
        let act = mundane_ui::unavailable(
            button(action, &self.action, t, move |w, cx| {
                if enabled {
                    let _ = v.update(
                        cx,
                        |c, cx| if timer { c.timer(w, cx) } else { c.city(w, cx) },
                    );
                }
            }),
            !enabled,
        );
        let title = if world {
            "World clocks"
        } else if search {
            "Add a world clock"
        } else {
            "Countdown"
        };
        let mut root = div()
            .size_full()
            .flex()
            .flex_col()
            .on_key_down(cx.listener(Self::keys))
            .font_family("DejaVu Sans")
            .bg(rgb(t.surface))
            .text_color(rgb(t.text))
            .child(self.header.render("Clocks", tools, &self.list.focus, t, w))
            .child(
                div()
                    .bg(rgb(t.panel))
                    .p_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(heading("section", title))
                    .child(act),
            );
        if search {
            root = root.child(div().p_3().child(self.search.clone()))
        }
        if timer {
            let status = match self.countdown {
                Countdown::Idle => "Duration in seconds, m:ss or h:mm:ss".into(),
                Countdown::Running(_) => self.remaining.clone(),
                Countdown::Finished => "Timer finished".into(),
            };
            root = root.child(
                div()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        mundane_ui::text_label("countdown", status.clone())
                            .text_xl()
                            .child(status),
                    )
                    .when(self.countdown == Countdown::Idle, |e| {
                        e.child(self.duration.clone())
                    }),
            );
        } else {
            let v = cx.weak_entity();
            root = root.child(self.list.render(
                if world {
                    "Saved cities"
                } else {
                    "Matching time zones"
                },
                t,
                w,
                move |i, double, w, cx| {
                    let _ = v.update(cx, |c, cx| {
                        c.list.select(i);
                        c.list.focus.focus(w, cx);
                        if double && search {
                            c.city(w, cx)
                        }
                        cx.notify()
                    });
                },
            ));
        }
        let msg = if !self.notice.is_empty() {
            self.notice.clone()
        } else if world {
            format!("{} saved cities", self.cities.len())
        } else if search {
            format!("{} matching time zones", self.list.rows.len())
        } else {
            String::new()
        };
        root.when(!msg.is_empty(), |e| {
            e.child(
                Role::Status
                    .control("status", msg.clone())
                    .bg(rgb(t.panel))
                    .px_3()
                    .py_2()
                    .child(msg),
            )
        })
    }
}
pub fn run(fixed: Option<i64>) {
    browser_window("Clocks", "org.joelinux.mundane.Clocks", move |w, cx| {
        let app = Clocks::new(fixed, clocks::settings_path(), w, cx);
        let executor = cx.background_executor().clone();
        cx.spawn_in(w, async move |view, cx| {
            loop {
                executor.timer(Duration::from_millis(250)).await;
                if view
                    .update_in(cx, |v, w, cx| v.tick(clocks::boot_now(), w, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        app
    })
}
#[cfg(test)]
mod tests {
    use super::{Clocks, Countdown, Page};
    use gpui::{Context, TestAppContext};
    #[gpui::test]
    fn cities_and_timer(cx: &mut TestAppContext) {
        let dir =
            tempfile::tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target")).unwrap();
        let path = dir.path().join("cities");
        let (view, ui) = cx.add_window_view(|w, cx: &mut Context<Clocks>| {
            Clocks::new(Some(1741503600), Some(path.clone()), w, cx)
        });
        ui.simulate_keystrokes("ctrl-n n e w space y o r k enter");
        ui.run_until_parked();
        assert!(
            std::fs::read_to_string(path)
                .unwrap()
                .contains("America/New_York")
        );
        ui.simulate_keystrokes("ctrl-t ctrl-a 1 enter");
        ui.update(|w, cx| {
            view.update(cx, |c, cx| {
                let Countdown::Running(end) = c.countdown else {
                    panic!("timer did not start")
                };
                c.tick(end, w, cx);
                assert!(c.page == Page::Timer && c.countdown == Countdown::Finished);
            })
        });
        ui.simulate_keystrokes("enter");
        assert!(view.read_with(ui, |c, _| c.countdown == Countdown::Idle));
    }
}
