use super::*;
use mundane_ui::{Role, button, focus_ring};
use std::ops::Range;

fn memory(bytes: u64) -> String {
    for (size, unit) in [(1 << 30, "GiB"), (1 << 20, "MiB"), (1 << 10, "KiB")] {
        if bytes >= size {
            return format!("{:.1} {unit}", bytes as f64 / size as f64);
        }
    }
    format!("{bytes} B")
}

fn cell(column: Column) -> Div {
    match column {
        Column::Name => div().flex_1().min_w_0().overflow_hidden(),
        Column::Pid => div().w(relative(0.16)).text_right(),
        Column::Cpu => div().w(relative(0.19)).text_right(),
        Column::Memory => div().w(relative(0.24)).text_right(),
    }
}

impl Monitor {
    fn keys(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &e.keystroke;
        if let Some(prompt) = &self.confirm {
            match key.key.as_str() {
                "tab" => {
                    if prompt.cancel.is_focused(window) && self.selected == Some(prompt.target) {
                        prompt.confirm.focus(window, cx);
                    } else {
                        prompt.cancel.focus(window, cx);
                    }
                }
                "escape" => self.finish(false, window, cx),
                _ => return,
            }
        } else {
            match key.key.as_str() {
                "tab" if key.modifiers.shift => window.focus_prev(cx),
                "tab" => window.focus_next(cx),
                "f6" => self.end_focus.focus(window, cx),
                "escape" => self.list_focus.focus(window, cx),
                "w" if key.modifiers.control => window.remove_window(),
                "+" | "=" | "-" | "0" if key.modifiers.control => {
                    self.text_size = match key.key.as_str() {
                        "-" => (self.text_size - 2.).max(12.),
                        "0" => 16.,
                        _ => (self.text_size + 2.).min(28.),
                    };
                }
                "n" => self.sort(Column::Name, cx),
                "p" => self.sort(Column::Pid, cx),
                "c" => self.sort(Column::Cpu, cx),
                "m" => self.sort(Column::Memory, cx),
                "delete" => self.ask(window, cx),
                "up" | "down" | "home" | "end" if !self.rows.is_empty() => {
                    let last = self.rows.len() - 1;
                    let i = match key.key.as_str() {
                        "up" => self.position().unwrap_or(0).saturating_sub(1),
                        "down" => self.position().map_or(0, |i| (i + 1).min(last)),
                        "home" => 0,
                        _ => last,
                    };
                    self.select(i, window, cx);
                }
                _ => return,
            }
        }
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }

    fn headings(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .px_3()
            .py_2()
            .gap_2()
            .bg(rgb(self.theme.panel))
            .children(
                Column::ALL
                    .into_iter()
                    .zip(&self.heading_focus)
                    .map(|(column, focus)| {
                        let view = cx.weak_entity();
                        let button = button(column.name(), focus, self.theme, move |_, cx| {
                            let _ = view.update(cx, |this, cx| this.sort(column, cx));
                        })
                        .w_full()
                        .px_1()
                        .flex()
                        .items_center()
                        .gap_1()
                        .when(column != Column::Name, |el| el.justify_end())
                        .when(column == self.column, |el| {
                            el.font_weight(FontWeight::SEMIBOLD)
                                .child(if self.descending { "↓" } else { "↑" })
                        });
                        cell(column).child(button)
                    }),
            )
    }

    fn rows(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Stateful<Div>> {
        range
            .map(|index| {
                let p = &self.rows[index];
                let id = p.id;
                let selected = self.selected == Some(id);
                let theme = self.theme;
                Role::ListItem
                    .row(format!("{} (PID {})", p.name, id.pid), id.pid as usize)
                    .w_full()
                    .h(px(self.text_size + 21.))
                    .px_3()
                    .gap_2()
                    .flex()
                    .items_center()
                    .border_l_2()
                    .border_color(rgb(if selected { theme.focus } else { theme.surface }))
                    .when(selected, |row| row.bg(rgb(theme.selection)))
                    .when(!selected, |row| {
                        row.hover(move |style| style.bg(rgb(theme.panel)))
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(index) = this.rows.iter().position(|r| r.id == id) {
                            this.select(index, window, cx);
                        }
                    }))
                    .child(cell(Column::Name).truncate().child(p.name.clone()))
                    .child(cell(Column::Pid).child(id.pid.to_string()))
                    .child(
                        cell(Column::Cpu).child(
                            p.cpu_percent
                                .map_or_else(|| "—".into(), |v| format!("{v:.1}%")),
                        ),
                    )
                    .child(cell(Column::Memory).child(memory(p.resident_bytes)))
            })
            .collect()
    }

    fn dialog(&self, confirm: &Confirmation, cx: &Context<Self>) -> impl IntoElement {
        let cancel = cx.weak_entity();
        let accept = cancel.clone();
        let present = self.selected == Some(confirm.target);
        Role::Dialog
            .control("End process?")
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x00000070))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(380.))
                    .max_w_full()
                    .p_3()
                    .gap_2()
                    .rounded_lg()
                    .flex()
                    .flex_col()
                    .bg(rgb(self.theme.panel))
                    .border_1()
                    .border_color(rgb(self.theme.border))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("End process?"),
                    )
                    .child(if present {
                        format!(
                            "Send SIGTERM to {} (PID {})?",
                            confirm.name, confirm.target.pid
                        )
                    } else {
                        format!(
                            "{} (PID {}) is no longer listed.",
                            confirm.name, confirm.target.pid
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .justify_end()
                            .child(button(
                                "Cancel",
                                &confirm.cancel,
                                self.theme,
                                move |w, cx| {
                                    let _ = cancel.update(cx, |this, cx| this.finish(false, w, cx));
                                },
                            ))
                            .when(present, |el| {
                                el.child(button(
                                    "End process",
                                    &confirm.confirm,
                                    self.theme,
                                    move |w, cx| {
                                        let _ =
                                            accept.update(cx, |this, cx| this.finish(true, w, cx));
                                    },
                                ))
                            }),
                    ),
            )
    }
}

impl Render for Monitor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let view = cx.weak_entity();
        let end = button(
            "End process…",
            &self.end_focus,
            theme,
            move |window, cx| {
                let _ = view.update(cx, |this, cx| this.ask(window, cx));
            },
        );
        let list = uniform_list("process-rows", self.rows.len(), cx.processor(Self::rows))
            .track_scroll(&self.scroll)
            .size_full();
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .font_family("DejaVu Sans")
            .bg(rgb(theme.surface))
            .text_color(rgb(theme.text))
            .on_key_down(cx.listener(Self::keys))
            .child(
                self.header
                    .render("System Monitor", end, &self.list_focus, theme, window),
            )
            .child(self.headings(cx))
            .child(
                focus_ring(Role::List.control("Processes"), &self.list_focus, theme)
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .text_size(px(self.text_size))
                    .child(list),
            )
            .child(
                Role::Status
                    .control("Process status")
                    .px_3()
                    .py_2()
                    .gap_2()
                    .flex()
                    .justify_between()
                    .items_center()
                    .bg(rgb(theme.panel))
                    .text_sm()
                    .text_color(rgb(theme.secondary))
                    .child(format!(
                        "{} {}",
                        self.rows.len(),
                        if self.rows.len() == 1 {
                            "process"
                        } else {
                            "processes"
                        }
                    ))
                    .child(
                        self.sample_error
                            .as_deref()
                            .or(self.status.as_deref())
                            .unwrap_or("Updates every 2 s")
                            .to_owned(),
                    ),
            )
            .children(self.confirm.as_ref().map(|prompt| self.dialog(prompt, cx)))
    }
}
