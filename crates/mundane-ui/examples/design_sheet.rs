//! Interactive reference for the public controls. Uses the same portal and renderers as a client.
use gpui::{prelude::*, *};
use mundane_ui::{self as ui, ControlRole, Theme};

struct Sheet {
    header: ui::WindowHeader,
    content: ui::ApplicationContent,
    focus: [FocusHandle; 8],
    field: Entity<ui::TextField>,
    password: Entity<ui::TextField>,
    document: Entity<ui::TextField>,
    list: ui::BrowserList<SharedString>,
    tabs: Vec<(&'static str, FocusHandle)>,
    active: usize,
    scroll: ScrollHandle,
    transport: ui::PlaybackControls,
    state: ui::TransportState,
    details: bool,
}
impl Sheet {
    fn new(w: &mut Window, cx: &mut Context<Self>) -> Self {
        let t = Theme::current(cx);
        let field = cx.new(|cx| {
            ui::TextField::new("Display name", t, cx).select_value("Coastal survey".into())
        });
        let password =
            cx.new(|cx| ui::TextField::password("Passphrase", t, cx).select_value("sample".into()));
        let document = cx.new(|cx| {
            let mut f = ui::TextField::multiline("Notes", t, cx).with_line_numbers();
            f.set_document("North quay\nWind from the east".into(), cx);
            f
        });
        let mut list = ui::BrowserList::new(cx);
        list.replace(
            [
                ("north", "North quay", "Morning survey", Some(0.65), "65%"),
                ("south", "South quay", "Afternoon survey", None, ""),
            ]
            .map(|(key, name, detail, fraction, value)| ui::BrowserRow {
                key: SharedString::from(key),
                label: name.into(),
                detail: detail.into(),
                value: value.into(),
                depth: 0,
                fraction,
            })
            .into(),
        );
        field.read(cx).focus_handle(cx).focus(w, cx);
        Self {
            header: ui::WindowHeader::new(cx),
            content: Default::default(),
            focus: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            field,
            password,
            document,
            list,
            tabs: Self::tabs(cx),
            active: 0,
            scroll: ScrollHandle::new(),
            transport: ui::PlaybackControls::new(cx),
            state: ui::TransportState {
                available: true,
                playing: false,
                ended: false,
                position: 42000,
                duration: Some(120000),
            },
            details: false,
        }
    }
    fn tabs(cx: &App) -> Vec<(&'static str, FocusHandle)> {
        ["Overview", "Notes"]
            .map(|name| (name, cx.focus_handle().tab_stop(true)))
            .into()
    }
    fn keys(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        if self.details {
            if k.key == "escape" {
                self.details = false;
                self.focus[0].focus(w, cx);
            } else if k.key != "tab" {
                return;
            }
        } else if self.list.focus.is_focused(w) && self.list.navigate(&k.key) {
        } else if k.key == "tab" && k.modifiers.shift {
            w.focus_prev(cx);
        } else if k.key == "tab" {
            w.focus_next(cx);
        } else {
            return;
        }
        w.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }
}
fn section(title: &'static str, t: Theme) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .bg(rgb(t.panel))
        .rounded_md()
        .border_1()
        .border_color(rgb(t.border))
        .child(ui::heading(title, title).font_weight(FontWeight::SEMIBOLD))
}
impl Render for Sheet {
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
        // The capture waits for the actual rendered preferences.
        let title = format!("Design sheet · {mode} · {motion} · {:06x}", t.focus);
        if w.window_title() != title {
            w.set_window_title(&title);
        }
        let v = cx.weak_entity();
        let details = ui::button("Show dialog", &self.focus[0], t, move |w, cx| {
            let _ = v.update(cx, |s, cx| {
                s.details = true;
                s.focus[1].focus(w, cx);
                cx.notify();
            });
        });
        let v = cx.weak_entity();
        let close = ui::button("Close tab", &self.focus[2], t, move |_, cx| {
            let _ = v.update(cx, |s, cx| {
                if !s.tabs.is_empty() {
                    s.tabs.remove(s.active);
                    s.active = 0;
                }
                cx.notify();
            });
        });
        let v = cx.weak_entity();
        let strip = ui::tab_strip(
            self.tabs.iter().map(|(n, f)| ((*n).into(), f.clone())),
            self.active,
            &self.scroll,
            move |i, _, cx| {
                let _ = v.update(cx, |s, cx| {
                    s.active = i;
                    cx.notify();
                });
            },
            close,
            t,
        );
        let v = cx.weak_entity();
        let list = self
            .list
            .render("Survey locations", t, w, move |i, _, w, cx| {
                let _ = v.update(cx, |s, cx| {
                    s.list.select(i);
                    s.list.focus.focus(w, cx);
                    cx.notify();
                });
            });
        let v = cx.weak_entity();
        let transport = self.transport.render(self.state, t, move |action, _, cx| {
            let _ = v.update(cx, |s, cx| {
                let st = &mut s.state;
                let dur = st.duration.unwrap();
                match action {
                    ui::TransportAction::Toggle => st.playing = !st.playing,
                    ui::TransportAction::Step(ms) => {
                        st.position = st.position.saturating_add_signed(ms).min(dur)
                    }
                    ui::TransportAction::Seek(f) => st.position = (dur as f64 * f) as u64,
                }
                cx.notify();
            });
        });
        let colors = div().flex().gap_2().children(
            [
                ("Surface", t.surface),
                ("Panel", t.panel),
                ("Text", t.text),
                ("Muted", t.muted()),
                ("Border", t.border),
                ("Focus", t.focus),
                ("Selected", t.selected()),
                ("Disabled", t.disabled()),
                ("Error", t.error()),
            ]
            .map(|(name, color)| {
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .h(px(23.))
                            .bg(rgb(color))
                            .border_1()
                            .border_color(rgb(t.border)),
                    )
                    .child(name)
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(t.secondary))
                            .child(format!("#{color:06X}")),
                    )
            }),
        );
        let input = section("Text entry", t)
            .child("Display name")
            .child(self.field.clone())
            .child("Passphrase")
            .child(self.password.clone())
            .child("Notes")
            .child(div().h(px(88.)).child(self.document.clone()));
        let v = cx.weak_entity();
        let reset = ui::button("Reset tabs", &self.focus[3], t, move |_, cx| {
            let _ = v.update(cx, |s, cx| {
                s.tabs = Self::tabs(cx);
                s.active = 0;
                cx.notify();
            });
        });
        let navigation = section("Navigation and actions", t)
            .child(strip)
            .child(div().min_h_0().flex().h(px(104.)).child(list))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(reset)
                    .child(ui::unavailable(
                        ui::button("Unavailable", &self.focus[4], t, |_, _| {}).opacity(0.55),
                        true,
                    ))
                    .child(ui::external_link(
                        "Project website",
                        "https://joelinux.io",
                        &self.focus[5],
                        t,
                    )),
            )
            .child(transport);
        let body = self
            .content
            .render(self.details)
            .size_full()
            .flex()
            .flex_col()
            .child(self.header.render(
                "mundane-ui",
                details,
                &self.field.read(cx).focus_handle(cx),
                t,
                w,
            ))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .p_4()
                    .gap_3()
                    .child(
                        ui::heading("sheet", "joe design language")
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD),
                    )
                    .child(
                        ui::text_label("mode", format!("{mode} · {motion}"))
                            .text_sm()
                            .child(format!("{mode} · {motion}")),
                    )
                    .child(colors)
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap_3()
                            .child(input)
                            .child(navigation),
                    ),
            );
        let v = cx.weak_entity();
        div()
            .size_full()
            .relative()
            .font_family("DejaVu Sans")
            .text_color(rgb(t.text))
            .bg(rgb(t.surface))
            .on_key_down(cx.listener(Self::keys))
            .child(body)
            .when(self.details, |el| {
                el.child(
                    ui::Role::Dialog
                        .control("dialog", "Component details")
                        .absolute()
                        .inset_0()
                        .occlude()
                        .bg(rgba(0x00000070))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .p_3()
                                .rounded_lg()
                                .border_1()
                                .border_color(rgb(t.border))
                                .bg(rgb(t.panel))
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(ui::heading("dialog-heading", "Component details"))
                                .child("Keyboard focus returns to the opening button.")
                                .child(ui::button("Done", &self.focus[1], t, move |w, cx| {
                                    let _ = v.update(cx, |s, cx| {
                                        s.details = false;
                                        s.focus[0].focus(w, cx);
                                        cx.notify();
                                    });
                                })),
                        ),
                )
            })
    }
}
#[path = "design_sheet/showcase.rs"]
mod showcase;
fn main() {
    if let Some(group) = std::env::args().nth(1)
        && group != "core"
    {
        ui::browser_window(
            "Design sheet",
            "org.joelinux.mundane.DesignSheet",
            move |w, cx| {
                ui::bind_text_keys(cx);
                showcase::Showcase::new(group, w, cx)
            },
        );
        return;
    }
    ui::browser_window(
        "Design sheet",
        "org.joelinux.mundane.DesignSheet",
        |w, cx| {
            if let Ok(scale) = std::env::var("SHEET_TEXT_SCALE") {
                ui::tokens::set_text_scale(w, scale.parse().unwrap());
            }
            ui::bind_text_keys(cx);
            Sheet::new(w, cx)
        },
    )
}
