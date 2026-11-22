use gpui::{prelude::*, *};
use mundane_ui::{self as ui, Theme};
pub struct Showcase {
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
        Self {
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
    fn panel(&self, cx: &Context<Self>) -> Div {
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
                    .child(ui::with_tooltip(
                        ui::button("Help", &self.handles[5], t, |_, _| {}),
                        "Show keyboard help",
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
            "settings" => ("Settings controls", self.settings(cx)),
            "menus" => ("Menus and searchable options", self.menus(cx)),
            "panel" => ("Panel controls", self.panel(cx)),
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
