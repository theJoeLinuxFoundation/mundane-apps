use gpui::{prelude::*, *};
use mundane_ui::{self as ui, Theme};
pub struct Showcase {
    history: ui::NotificationHistory,
    banner: Option<ui::NotificationCard>,
    posted: u32,
    notice_action: SharedString,
    slider: ui::ValueSlider,
    level: f64,
    connected: bool,
    confirmed: bool,
    confirmation: Entity<ui::ConfirmDialog>,
    covered: ui::ApplicationContent,
    popover: ui::Popover,
    menus: Vec<Entity<ui::ChoiceMenu>>,
    chosen: SharedString,
    group: String,
    header: ui::WindowHeader,
    handles: Vec<FocusHandle>,
    on: bool,
    launches: usize,
}
impl Showcase {
    pub fn new(group: String, w: &mut Window, cx: &mut Context<Self>) -> Self {
        let handles: Vec<_> = (0..20).map(|_| cx.focus_handle().tab_stop(true)).collect();
        handles[0].focus(w, cx);
        let mut popover = ui::Popover::new(cx);
        let mut menus = vec![];
        if group == "menus" {
            popover.show(&handles[6], w, cx);
            for (name, options, searchable) in [
                ("Actions", vec!["Open", "Rename", "Delete"], false),
                (
                    "Locations",
                    vec!["North quay", "South quay", "Harbour"],
                    true,
                ),
            ] {
                let menu = cx.new(|cx| {
                    ui::ChoiceMenu::new(
                        name,
                        options
                            .into_iter()
                            .map(|label| {
                                let mut c = ui::MenuChoice::new(label, label);
                                c.enabled = label != "Rename";
                                c
                            })
                            .collect(),
                        searchable,
                        cx,
                    )
                });
                cx.observe(&menu, |_, _, cx| cx.notify()).detach();
                cx.subscribe(&menu, |s, _, event, cx| {
                    if let ui::MenuEvent::Selected(key) = event {
                        s.chosen = key.clone();
                        cx.notify();
                    }
                })
                .detach();
                menu.update(cx, |m, cx| m.show(w, cx));
                menus.push(menu);
            }
        }
        let confirmation = cx.new(|cx| {
            ui::ConfirmDialog::new(
                "Reset preferences?",
                "Restore the saved defaults for this profile.",
                "Reset",
                cx,
            )
        });
        cx.observe(&confirmation, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&confirmation, |s, _, event, cx| {
            s.confirmed = *event == ui::Confirmation::Confirmed;
            cx.notify();
        })
        .detach();
        let mut history = ui::NotificationHistory::new(cx);
        history.replace(
            vec![
                Self::notice(
                    "survey",
                    "Survey saved",
                    "Observations are ready to review.",
                ),
                Self::notice("sync", "Sync complete", "Three changes were saved."),
            ],
            w,
            cx,
        );
        Self {
            history,
            banner: Some(ui::NotificationCard::new(
                Self::notice(
                    "download",
                    "Download complete",
                    "The report is ready to open.",
                ),
                cx,
            )),
            posted: 0,
            notice_action: "None".into(),
            slider: ui::ValueSlider::new(cx),
            level: 50.,
            connected: true,
            confirmed: false,
            confirmation,
            covered: Default::default(),
            popover,
            menus,
            chosen: "None".into(),
            group,
            header: ui::WindowHeader::new(cx),
            handles,
            on: true,
            launches: 0,
        }
    }
    fn menus(&self, cx: &Context<Self>) -> Div {
        let trigger = cx.weak_entity();
        let dismiss = cx.weak_entity();
        let close = cx.weak_entity();
        let popup = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                ui::button(
                    "Details",
                    &self.popover.trigger(),
                    Theme::current(cx),
                    move |w, cx| {
                        let _ = trigger.update(cx, |s, cx| {
                            s.popover.toggle(&s.handles[6], w, cx);
                            cx.notify();
                        });
                    },
                )
                .aria_expanded(self.popover.is_open()),
            )
            .when(self.popover.is_open(), |el| {
                el.child(ui::popover_contents(
                    "Details popup",
                    Theme::current(cx),
                    ui::button(
                        "Dismiss details",
                        &self.handles[6],
                        Theme::current(cx),
                        move |w, cx| {
                            let _ = close.update(cx, |s, cx| {
                                s.popover.dismiss(w, cx);
                                cx.notify();
                            });
                        },
                    ),
                    move |w, cx| {
                        let _ = dismiss.update(cx, |s, cx| {
                            s.popover.dismiss(w, cx);
                            cx.notify();
                        });
                    },
                ))
            });

        let t = Theme::current(cx);
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(popup)
            .child(
                div().flex().gap_3().items_start().children(
                    ["Actions", "Locations"]
                        .into_iter()
                        .zip(&self.menus)
                        .map(|(label, m)| {
                            div()
                                .w(rems(20.))
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(ui::menu_trigger(label, m, t, cx))
                                .child(m.clone())
                        }),
                ),
            )
            .child(
                ui::text_label("selected", format!("Selected: {}", self.chosen))
                    .child(format!("Selected: {}", self.chosen)),
            )
    }
    fn confirm(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.confirmation
            .update(cx, |c, cx| c.show(&self.handles[2], w, cx));
    }
    fn settings(&self, cx: &Context<Self>) -> Div {
        let t = Theme::current(cx);
        let v = cx.weak_entity();
        let sw = cx.weak_entity();
        let slider = cx.weak_entity();
        let confirm = cx.weak_entity();
        let tile = ui::quick_tile(
            ui::TileState {
                label: "Connection",
                status: if self.connected {
                    "Connected"
                } else {
                    "Disconnected"
                },
                on: self.connected,
                enabled: true,
                error: None,
            },
            "⌁",
            &self.handles[0],
            t,
            move |value, _, cx| {
                let _ = v.update(cx, |s, cx| {
                    s.connected = value;
                    cx.notify();
                });
            },
        );
        let error = ui::quick_tile(
            ui::TileState {
                label: "Updates",
                status: "Unavailable",
                on: false,
                enabled: false,
                error: Some("Service unavailable"),
            },
            "↻",
            &self.handles[3],
            t,
            |_, _, _| {},
        );
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(ui::heading("tiles", "Quick settings"))
            .child(div().flex().gap_2().child(tile).child(error))
            .child(ui::labelled_switch(
                "Quiet mode",
                &self.handles[1],
                self.on,
                true,
                t,
                move |v, w, cx| {
                    let _ = sw.update(cx, |s, cx| {
                        s.on = v;
                        s.handles[1].focus(w, cx);
                        cx.notify();
                    });
                },
            ))
            .child(self.slider.render(
                "Level",
                ui::SliderRange::new(0., 100., 5.).unwrap(),
                self.level,
                true,
                t,
                move |v, _, cx| {
                    let _ = slider.update(cx, |s, cx| {
                        s.level = v;
                        cx.notify();
                    });
                },
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(ui::button(
                        "Reset preferences",
                        &self.handles[2],
                        t,
                        move |w, cx| {
                            let _ = confirm.update(cx, |s, cx| s.confirm(w, cx));
                        },
                    ))
                    .when(self.confirmed, |el| el.child("Defaults restored")),
            )
    }
    fn notice(key: &str, title: &str, body: &str) -> ui::Notice {
        ui::Notice {
            key: key.to_string().into(),
            title: title.to_string().into(),
            body: body.to_string().into(),
            source: Some("Activity".into()),
            actions: vec![("open".into(), "Open".into())],
        }
    }
    fn post(&mut self, cx: &mut Context<Self>) {
        self.posted += 1;
        let n = Self::notice(
            "incoming",
            "New message",
            &format!("Message {} arrived.", self.posted),
        );
        self.banner = Some(ui::NotificationCard::new(n, cx));
        cx.notify();
    }
    fn notices(&self, cx: &Context<Self>) -> Div {
        let t = Theme::current(cx);
        let send = cx.weak_entity();
        let history = cx.weak_entity();
        let banner = cx.weak_entity();
        let cards = div()
            .flex()
            .gap_3()
            .items_start()
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(ui::heading("banner-heading", "Banner"))
                    .when_some(self.banner.as_ref(), |el, card| {
                        el.child(card.render(t, move |_, a, _, cx| {
                            let _ = banner.update(cx, |s, cx| {
                                match a {
                                    ui::NoticeAction::Dismiss => s.banner = None,
                                    ui::NoticeAction::Invoke(id) => s.notice_action = id,
                                }
                                cx.notify();
                            });
                        }))
                    })
                    .child(ui::level_osd(
                        "volume",
                        "Volume",
                        self.level as u8,
                        false,
                        t,
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(ui::heading("history-heading", "History"))
                    .child(
                        div()
                            .h(rems(18.))
                            .flex()
                            .flex_col()
                            .child(self.history.render(
                                "Notification history",
                                t,
                                move |key, a, w, cx| {
                                    let _ = history.update(cx, |s, cx| {
                                        if a == ui::NoticeAction::Dismiss {
                                            s.history.remove(&key, w, cx);
                                        } else if let ui::NoticeAction::Invoke(id) = a {
                                            s.notice_action = id;
                                        }
                                        cx.notify();
                                    });
                                },
                            )),
                    ),
            );
        div().flex().flex_col().gap_3().child(cards).child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(ui::button(
                    "Post notification",
                    &self.handles[0],
                    t,
                    move |_, cx| {
                        let _ = send.update(cx, |s, cx| s.post(cx));
                    },
                ))
                .child(
                    ui::text_label("last", format!("Action: {}", self.notice_action))
                        .child(format!("Action: {}", self.notice_action)),
                ),
        )
    }
    fn panel(&self, w: &mut Window, cx: &mut Context<Self>) -> Div {
        let t = Theme::current(cx);
        let v = cx.weak_entity();
        let on = ui::toggle(
            "Show labels",
            &self.handles[3],
            self.on,
            true,
            t,
            move |next, _, cx| {
                let _ = v.update(cx, |s, cx| {
                    s.on = next;
                    cx.notify();
                });
            },
        );
        let row = div().flex().items_center().gap_3().children(
            [
                (
                    "Overview",
                    "▦",
                    ui::PanelState {
                        active: true,
                        ..Default::default()
                    },
                ),
                (
                    "Editor",
                    "T",
                    ui::PanelState {
                        running: true,
                        ..Default::default()
                    },
                ),
                (
                    "Workspace 2",
                    "2",
                    ui::PanelState {
                        occupied: true,
                        ..Default::default()
                    },
                ),
                (
                    "Unavailable command",
                    "−",
                    ui::PanelState {
                        disabled: true,
                        ..Default::default()
                    },
                ),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, (name, icon, state))| {
                let v = cx.weak_entity();
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(ui::panel_button(
                        name,
                        icon,
                        &self.handles[if i == 3 { 4 } else { i }],
                        state,
                        t,
                        move |_, cx| {
                            let _ = v.update(cx, |s, cx| {
                                s.launches += 1;
                                cx.notify();
                            });
                        },
                    ))
                    .when(self.on, |el| el.child(div().text_sm().child(name)))
            }),
        );
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(ui::heading("buttons", "Panel buttons"))
            .child(row)
            .child(ui::heading("states", "Toggle and status"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(on)
                    .child(ui::badge("count", "Count", "3", t))
                    .child(ui::badge("status", "Connection", "Connected", t)),
            )
            .child(ui::heading("hints", "Tooltip"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(ui::with_keyboard_tooltip(
                        ui::button("Help", &self.handles[5], t, |_, _| {}),
                        &self.handles[5],
                        "Show keyboard help",
                        w,
                        cx,
                    ))
                    .child(ui::tooltip("tooltip-sample", "Show keyboard help", t)),
            )
            .child(
                ui::text_label("commands", format!("Commands activated: {}", self.launches))
                    .child(format!("Commands activated: {}", self.launches)),
            )
    }
}
impl Render for Showcase {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        let mode = match (t.light, t.high_contrast) {
            (true, false) => "Light",
            (false, false) => "Dark",
            (true, true) => "Light high contrast",
            _ => "Dark high contrast",
        };
        let motion = if cx.reduce_motion() {
            "Reduced motion"
        } else {
            "Standard motion"
        };
        let title = format!("Design sheet · {mode} · {motion} · {:06x}", t.focus);
        if w.window_title() != title {
            w.set_window_title(&title);
        }
        let (heading, body) = match self.group.as_str() {
            "notifications" => ("Notifications and levels", self.notices(cx)),
            "settings" => ("Settings controls", self.settings(cx)),
            "menus" => ("Menus and searchable options", self.menus(cx)),
            "panel" => ("Panel controls", self.panel(w, cx)),
            _ => unreachable!(),
        };
        let dialog = self.confirmation.read(cx).is_open();
        let page = self
            .covered
            .render(dialog)
            .size_full()
            .flex()
            .flex_col()
            .font_family("DejaVu Sans")
            .text_color(rgb(t.text))
            .bg(rgb(t.surface))
            .on_key_down(|e, w, cx| {
                if e.keystroke.key == "tab" {
                    if e.keystroke.modifiers.shift {
                        w.focus_prev(cx)
                    } else {
                        w.focus_next(cx)
                    }
                    cx.stop_propagation();
                }
            })
            .child(
                self.header
                    .render("mundane-ui", div(), &self.handles[0], t, w),
            )
            .child(
                div()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        ui::heading("section", heading)
                            .text_size(ui::tokens::text::TITLE)
                            .font_weight(FontWeight::SEMIBOLD),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(t.muted()))
                            .child(format!("{mode} · {motion}")),
                    )
                    .child(
                        div()
                            .p_4()
                            .max_w(rems(57.))
                            .rounded(ui::tokens::radius::DIALOG)
                            .border_1()
                            .border_color(rgb(t.border))
                            .bg(rgb(t.panel))
                            .child(body),
                    ),
            );
        div()
            .relative()
            .size_full()
            .text_color(rgb(t.text))
            .font_family("DejaVu Sans")
            .capture_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                if s.group == "menus" && e.keystroke.key == "f9" {
                    let mut rows: Vec<_> = (0..36)
                        .map(|i| {
                            let name = format!("Option {i}");
                            ui::MenuChoice::new(name.clone(), name)
                        })
                        .collect();
                    rows.push(ui::MenuChoice::new("Delete", "Delete"));
                    s.menus[0].update(cx, |menu, cx| menu.replace(rows, w, cx));
                }
                if s.group == "notifications" && e.keystroke.key == "f8" {
                    let mut notices = vec![Self::notice(
                        "sync",
                        "Sync complete",
                        "Three changes were saved.",
                    )];
                    notices.extend((0..24).map(|i| {
                        Self::notice(
                            &format!("message-{i}"),
                            &format!("Message {i}"),
                            "Saved to history.",
                        )
                    }));
                    notices.push(Self::notice(
                        "survey",
                        "Survey saved",
                        "Observations are ready to review.",
                    ));
                    s.history.replace(notices, w, cx);
                    cx.notify();
                }
                if s.group == "notifications" && e.keystroke.key == "f9" {
                    s.post(cx);
                    cx.stop_propagation();
                    return;
                }
                if s.group == "notifications" && e.keystroke.key == "f10" {
                    s.level = (s.level + 5.).min(100.);
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                if s.group == "settings"
                    && e.keystroke.key == "f8"
                    && !s.confirmation.read(cx).is_open()
                {
                    s.confirm(w, cx);
                    cx.stop_propagation();
                }
            }))
            .child(page)
            .when(dialog, |el| el.child(self.confirmation.clone()))
    }
}
