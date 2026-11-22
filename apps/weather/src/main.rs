use gpui::{prelude::*, *};
use mundane_services::{
    provider,
    weather::{self, Location, Report, Weather},
};
use mundane_ui::{
    BrowserList, BrowserRow, ControlRole, Role, TextField, Theme, WindowHeader, browser_key,
    browser_window, button, external_link, text_label,
};
struct View {
    service: Weather,
    cities: Vec<Location>,
    visible: Vec<Location>,
    saved: Vec<Location>,
    daily: bool,
    day_focus: FocusHandle,
    here: FocusHandle,
    location: Option<Location>,
    report: Option<Report>,
    choosing: bool,
    loading: bool,
    status: String,
    header: WindowHeader,
    list: BrowserList,
    query: Entity<TextField>,
    change: FocusHandle,
    action: FocusHandle,
    source: FocusHandle,
    licence: FocusHandle,
}
impl View {
    fn new(service: Weather, w: &mut Window, cx: &mut Context<Self>) -> Self {
        mundane_ui::bind_text_keys(cx);
        let t = Theme::current(cx);
        let query = cx.new(|cx| TextField::new("City or coordinates", t, cx));
        cx.observe(&query, |v, _, cx| {
            if v.choosing {
                v.filter(cx)
            }
        })
        .detach();
        let location = service.selected();
        let mut v = Self {
            saved: service.saved().unwrap_or_default(),
            daily: false,
            day_focus: cx.focus_handle().tab_stop(true),
            here: cx.focus_handle().tab_stop(true),
            service,
            cities: weather::cities(),
            visible: vec![],
            location,
            report: None,
            choosing: false,
            loading: false,
            status: String::new(),
            header: WindowHeader::new(cx),
            list: BrowserList::new(cx),
            query,
            change: cx.focus_handle().tab_stop(true),
            action: cx.focus_handle().tab_stop(true),
            source: cx.focus_handle().tab_stop(true),
            licence: cx.focus_handle().tab_stop(true),
        };
        if v.location.is_some() {
            v.list.focus.focus(w, cx);
            v.refresh(false, w, cx)
        } else {
            v.choose(w, cx)
        }
        v
    }
    fn choose(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.saved = self.service.saved().unwrap_or_default();
        self.choosing = true;
        self.query.update(cx, |f, cx| {
            f.select_match(0..f.text().len(), cx);
            f.replace_text_in_range(None, "", w, cx);
        });
        self.filter(cx);
        self.query.focus_handle(cx).focus(w, cx);
        self.query
            .update(cx, |f, cx| f.select_match(0..f.text().len(), cx));
    }
    fn filter(&mut self, cx: &mut Context<Self>) {
        let q = self.query.read(cx).text().trim().to_lowercase();
        let matches =
            |l: &Location| l.name.to_lowercase().contains(&q) || l.zone.to_lowercase().contains(&q);
        self.visible = self
            .saved
            .iter()
            .filter(|l| matches(l))
            .chain(
                self.cities
                    .iter()
                    .filter(|l| matches(l) && !self.saved.contains(l)),
            )
            .cloned()
            .collect();
        if let Ok(l) = Location::custom(self.query.read(cx).text()) {
            self.visible.insert(0, l)
        }
        self.list.replace(
            self.visible
                .iter()
                .map(|l| BrowserRow {
                    key: format!("{}:{}:{}", l.zone, l.lat, l.lon).into(),
                    label: l.name.clone(),
                    detail: if self.saved.contains(l) {
                        format!("Saved · {}", l.zone)
                    } else {
                        l.zone.clone()
                    },
                    value: format!("{:.2}°, {:.2}°", l.lat, l.lon),
                    fraction: None,
                    depth: 0,
                })
                .collect(),
        );
        self.status = if self.visible.is_empty() {
            "No matching locations".into()
        } else {
            "Choose a city or enter latitude longitude and IANA time zone".into()
        };
        cx.notify()
    }
    fn select(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(l) = self.list.selected.and_then(|i| self.visible.get(i)) {
            self.location = Some(l.clone());
            self.report = None;
            self.choosing = false;
            self.list.replace(vec![]);
            self.list.focus.focus(w, cx);
            self.refresh(true, w, cx)
        }
    }
    fn refresh(&mut self, save: bool, w: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let Some(l) = self.location.clone() else {
            return;
        };
        let service = self.service.clone();
        self.loading = true;
        self.status = "Checking forecast…".into();
        cx.notify();
        let job = cx.background_executor().spawn(async move {
            if save {
                service.select(&l)?
            }
            service.fetch(&l)
        });
        cx.spawn_in(w, async move |v, cx| {
            let r = job.await;
            let _ = v.update_in(cx, |v, w, cx| {
                v.loading = false;
                match r {
                    Ok(report) => {
                        v.status = report.status.clone();
                        v.report = Some(report);
                        v.show(w, cx)
                    }
                    Err(e) => {
                        v.status = format!("Forecast unavailable · {e}");
                        cx.notify()
                    }
                }
            });
        })
        .detach()
    }
    fn show(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let (Some(r), Some(l)) = (&self.report, &self.location) else {
            return;
        };
        self.choosing = false;
        self.status = r.status.clone();
        let hours = &r.forecast.hours;
        let start = hours
            .iter()
            .position(|h| h.time >= (provider::now() as i64 / 3600) * 3600)
            .unwrap_or(0);
        if self.daily {
            let days = weather::daily(&hours[start..], &l.zone).unwrap_or_default();
            self.list.replace(
                days.into_iter()
                    .map(|d| BrowserRow {
                        key: d.label.clone().into(),
                        label: d.label,
                        detail: d.condition,
                        value: format!("{:.1} – {:.1} °C", d.low, d.high),
                        fraction: None,
                        depth: 0,
                    })
                    .collect(),
            );
        } else {
            self.list.replace(
                hours[start..]
                    .iter()
                    .map(|h| BrowserRow {
                        key: h.time.to_string().into(),
                        label: weather::local(h.time, &l.zone).unwrap_or_default(),
                        detail: weather::condition_label(&h.condition),
                        value: format!(
                            "{:.1} °C{}",
                            h.temperature,
                            h.wind
                                .map(|v| format!("   ·   {v:.1} m/s"))
                                .unwrap_or_default()
                        ),
                        depth: 0,
                        fraction: None,
                    })
                    .collect(),
            );
        }
        w.set_window_title(&format!("{} — Weather", l.name));
        cx.notify()
    }
    fn locate(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.status = "Waiting for location permission…".into();
        cx.notify();
        cx.spawn_in(w, async move |v, cx| {
            let result = mundane_services::location::locate(true).await;
            let _ = v.update_in(cx, |v, w, cx| {
                v.loading = false;
                match result {
                    Ok((lat, lon)) => {
                        v.location = Some(Location {
                            name: "Current location".into(),
                            lat,
                            lon,
                            zone: weather::system_zone(),
                        });
                        v.report = None;
                        v.choosing = false;
                        v.list.focus.focus(w, cx);
                        v.refresh(false, w, cx);
                    }
                    Err(e) => {
                        v.status = format!("Location unavailable · {e}");
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
    fn toggle_days(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.report.is_some() && !self.choosing {
            self.daily = !self.daily;
            self.show(w, cx);
        }
    }
    fn keys(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        match (k.key.as_str(), k.modifiers.control) {
            ("f6", false) => if self.choosing {
                &self.action
            } else {
                &self.change
            }
            .focus(w, cx),
            ("d", true) => self.toggle_days(w, cx),
            ("g", true) => self.locate(w, cx),
            ("l", true) => self.choose(w, cx),
            ("f5", false) if !self.choosing => self.refresh(false, w, cx),
            ("escape", false) if self.choosing && self.report.is_some() => {
                self.show(w, cx);
                self.list.focus.focus(w, cx)
            }
            ("enter", false)
                if self.choosing
                    && (self.query.focus_handle(cx).is_focused(w)
                        || self.list.focus.is_focused(w)) =>
            {
                self.select(w, cx)
            }
            ("down", false) if self.choosing && self.query.focus_handle(cx).is_focused(w) => {
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
impl Render for View {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        let choose = self.choosing;
        let v = cx.weak_entity();
        let change = button("Locations", &self.change, t, move |w, cx| {
            let _ = v.update(cx, |v, cx| v.choose(w, cx));
        });
        let v = cx.weak_entity();
        let action = button(
            if choose { "Show forecast" } else { "Refresh" },
            &self.action,
            t,
            move |w, cx| {
                let _ = v.update(cx, |v, cx| {
                    if choose {
                        v.select(w, cx)
                    } else {
                        v.refresh(false, w, cx)
                    }
                });
            },
        );
        let v = cx.weak_entity();
        let list = self.list.render(
            if choose {
                "Locations"
            } else {
                if self.daily {
                    "Daily forecast"
                } else {
                    "Forecast periods"
                }
            },
            t,
            w,
            move |i, double, w, cx| {
                let _ = v.update(cx, |v, cx| {
                    v.list.select(i);
                    v.list.focus.focus(w, cx);
                    if double && v.choosing {
                        v.select(w, cx)
                    }
                    cx.notify()
                });
            },
        );
        let mut top = div().p_3().bg(rgb(t.panel)).flex().flex_col().gap_1();
        if choose {
            top = top.child(self.query.clone())
        } else if let Some(l) = &self.location {
            top = top.child(
                text_label("city", l.name.clone())
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(l.name.clone()),
            );
            if let Some(r) = &self.report {
                let updated = format!(
                    "Updated {}",
                    weather::local(r.forecast.updated, &l.zone).unwrap_or_default()
                );
                top = top.child(
                    text_label("update", updated.clone())
                        .text_sm()
                        .child(updated),
                )
            }
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .font_family("DejaVu Sans")
            .bg(rgb(t.surface))
            .text_color(rgb(t.text))
            .on_key_down(cx.listener(Self::keys))
            .child(
                self.header.render(
                    "Weather",
                    div()
                        .flex()
                        .gap_2()
                        .when(!choose, |e| {
                            e.child(mundane_ui::unavailable(change, self.loading))
                        })
                        .when(!choose && self.report.is_some(), |el| {
                            let v = cx.weak_entity();
                            el.child(button(
                                if self.daily { "Hourly" } else { "Daily" },
                                &self.day_focus,
                                t,
                                move |w, cx| {
                                    let _ = v.update(cx, |v, cx| v.toggle_days(w, cx));
                                },
                            ))
                        })
                        .when(choose, |el| {
                            let v = cx.weak_entity();
                            el.child(mundane_ui::unavailable(
                                button("Current location", &self.here, t, move |w, cx| {
                                    let _ = v.update(cx, |v, cx| v.locate(w, cx));
                                }),
                                self.loading,
                            ))
                        })
                        .child(mundane_ui::unavailable(
                            action,
                            self.loading || choose && self.visible.is_empty(),
                        )),
                    &if choose {
                        self.query.focus_handle(cx)
                    } else {
                        self.list.focus.clone()
                    },
                    t,
                    w,
                ),
            )
            .child(top)
            .child(list)
            .child(
                div()
                    .bg(rgb(t.panel))
                    .px_3()
                    .py_1()
                    .flex()
                    .flex_wrap()
                    .justify_between()
                    .items_center()
                    .child(
                        Role::Status
                            .control("weather-status", self.status.clone())
                            .child(self.status.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(external_link(
                                "MET Norway",
                                "https://www.met.no/en",
                                &self.source,
                                t,
                            ))
                            .child(external_link(
                                "CC BY 4.0",
                                "https://creativecommons.org/licenses/by/4.0/",
                                &self.licence,
                                t,
                            )),
                    ),
            )
    }
}
fn main() {
    let (Ok(dir), Ok(data)) = (
        provider::cache_dir("weather"),
        provider::data_dir("weather"),
    ) else {
        eprintln!("Weather needs a cache directory.");
        return;
    };
    let endpoint =
        std::env::var("MUNDANE_WEATHER_ENDPOINT").unwrap_or_else(|_| weather::ENDPOINT.into());
    browser_window("Weather", "org.joelinux.mundane.Weather", move |w, cx| {
        View::new(
            Weather {
                dir,
                data,
                endpoint,
            },
            w,
            cx,
        )
    })
}
#[cfg(test)]
mod tests;
