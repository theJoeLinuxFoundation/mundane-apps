use gpui::{prelude::*, *};
use mundane_ui::{self as ui, Theme};
pub struct Showcase {
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
        Self {
            group,
            header: ui::WindowHeader::new(cx),
            handles,
            on: true,
            launches: 0,
        }
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
            "panel" => ("Panel controls", self.panel(cx)),
            _ => unreachable!(),
        };
        div()
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
            )
    }
}
