use super::*;
use mundane_ui::ControlRole as _;
use mundane_ui::{Role, button};

impl Editor {
    pub(super) fn tool(
        &self,
        cmd: Command,
        focus: &FocusHandle,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let view = cx.weak_entity();
        button(cmd.label(), focus, Theme::current(cx), move |window, cx| {
            let _ = view.update(cx, |this, cx| this.command(cmd, window, cx));
        })
    }

    fn cycle_indent(&mut self, cx: &mut Context<Self>) {
        match self.indent.next() {
            Ok(i) => self.indent = i,
            Err(e) => self.error = Some(format!("Could not save indentation. {e}")),
        }
        cx.notify()
    }
    fn keys(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if self.busy.is_some() {
            return;
        }
        if let Some(confirm) = &self.confirmation {
            match key.key.as_str() {
                "escape" => self.command(Command::Cancel, window, cx),
                "tab" => {
                    let i = confirm
                        .buttons
                        .iter()
                        .position(|f| f.is_focused(window))
                        .unwrap_or(0);
                    confirm.buttons[(i + if key.modifiers.shift { 2 } else { 1 }) % 3]
                        .focus(window, cx);
                }
                _ => return,
            }
        } else {
            match (key.key.as_str(), key.modifiers.control) {
                ("t", true) if key.modifiers.shift => self.command(Command::Reopen, window, cx),
                ("p", true) if key.modifiers.shift => self.command(Command::Print, window, cx),
                ("n" | "t", true) => self.command(Command::New, window, cx),
                ("i", true) => self.command(Command::Line, window, cx),
                ("pageup" | "pagedown", true) => {
                    let n = self.pages.len();
                    let delta = if key.key == "pageup" { n - 1 } else { 1 };
                    self.switch((self.active + delta) % n, window, cx);
                }
                ("enter", false)
                    if self.going && self.line.read(cx).focus_handle(cx).is_focused(window) =>
                {
                    self.command(Command::Jump, window, cx)
                }
                ("escape", false) if self.going => self.command(Command::Done, window, cx),
                ("tab", false)
                    if !key.modifiers.shift
                        && self.replacing
                        && self.query.read(cx).focus_handle(cx).is_focused(window) =>
                {
                    self.replacement.read(cx).focus_handle(cx).focus(window, cx)
                }
                ("p", true) => self.command(Command::Recent, window, cx),
                ("h", true) => self.command(Command::Replace, window, cx),
                ("escape", false) if self.showing_recent => self.command(Command::Done, window, cx),
                ("enter", false) if self.showing_recent => {
                    if let Some(i) = self.recent.selected {
                        self.open_recent(i, window, cx);
                    }
                }
                (k, false) if self.showing_recent && self.recent.navigate(k) => cx.notify(),
                ("enter", false)
                    if self.replacing
                        && self
                            .replacement
                            .read(cx)
                            .focus_handle(cx)
                            .is_focused(window) =>
                {
                    self.command(Command::ReplaceOne, window, cx)
                }
                ("o", true) => self.command(Command::Open, window, cx),
                ("s", true) if key.modifiers.shift => self.command(Command::SaveAs, window, cx),
                ("s", true) => self.command(Command::Save, window, cx),
                ("f", true) => self.command(Command::Find, window, cx),
                ("w", true) => self.request(Pending::CloseTab, window, cx),
                ("f3", false) if self.searching => self.find_match(true, cx),
                ("enter", false)
                    if self.searching
                        && self.query.read(cx).focus_handle(cx).is_focused(window) =>
                {
                    self.find_match(true, cx)
                }
                ("escape", false) if self.searching => self.command(Command::Done, window, cx),
                ("f6", false) => self.tools[0].focus(window, cx),
                ("tab", false) if key.modifiers.shift => window.focus_prev(cx),
                ("tab", false) => window.focus_next(cx),
                _ => return,
            }
        }
        window.prevent_default();
        cx.stop_propagation();
    }

    fn find_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        let term = self.query.read(cx).text();
        let total = if term.is_empty() {
            0
        } else {
            self.page().body.read(cx).text().matches(term).count()
        };
        div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap_2()
            .px_3()
            .py_2()
            .bg(rgb(Theme::current(cx).panel))
            .child(div().flex_1().min_w(px(100.)).child(self.query.clone()))
            .when(!term.is_empty(), |el| {
                let label: SharedString =
                    format!("{total} {}", if total == 1 { "match" } else { "matches" }).into();
                el.child(
                    Role::Status
                        .control("match-count", label.clone())
                        .text_sm()
                        .child(label),
                )
            })
            .when(!self.replacing, |el| {
                el.child(self.tool(Command::Replace, &self.replace_tools[0], cx))
            })
            .child(self.tool(Command::Next, &self.find_buttons[0], cx))
            .child(self.tool(Command::Done, &self.find_buttons[1], cx))
    }

    fn prompt(&self, confirm: &Confirmation, cx: &Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        Role::Dialog
            .control("save-dialog", "Save changes?")
            .aria_description(format!("{} has unsaved changes.", self.name()))
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x00000070))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .rounded_lg()
                    .p_3()
                    .max_w_full()
                    .bg(rgb(theme.panel))
                    .border_1()
                    .border_color(rgb(theme.border))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Save changes?"),
                    )
                    .child(format!("{} has unsaved changes.", self.name()))
                    .child(
                        div().flex().flex_wrap().gap_2().justify_end().children(
                            [Command::Cancel, Command::Discard, Command::ConfirmSave]
                                .into_iter()
                                .zip(&confirm.buttons)
                                .map(|(cmd, focus)| self.tool(cmd, focus, cx)),
                        ),
                    ),
            )
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        let name = self.name();
        let dirty = self.dirty(cx);
        let title = format!("{}{} — Text Editor", if dirty { "• " } else { "" }, name);
        if window.window_title() != title {
            window.set_window_title(&title);
        }
        let body = self.page().body.read(cx);
        let focus = body.focus_handle(cx);
        let (row, col) = body.line_column();
        let ending = body.line_ending();
        let tools = div()
            .flex()
            .gap_1()
            .child(self.tool(Command::Print, &self.print_focus, cx))
            .child(self.tool(Command::Recent, &self.recent_focus, cx))
            .child(self.tool(Command::New, &self.new_focus, cx))
            .children(
                [Command::Open, Command::Find, Command::Save, Command::SaveAs]
                    .into_iter()
                    .zip(&self.tools)
                    .map(|(cmd, focus)| self.tool(cmd, focus, cx)),
            );
        let recent_view = cx.weak_entity();
        let recent = self
            .recent
            .render("Recent files", theme, window, move |i, open, w, cx| {
                let _ = recent_view.update(cx, |e, cx| {
                    e.recent.select(i);
                    if open {
                        e.open_recent(i, w, cx);
                    }
                    cx.notify();
                });
            });
        let body = self
            .content
            .render(self.confirmation.is_some() || self.busy.is_some())
            .size_full()
            .flex()
            .flex_col()
            .font_family("DejaVu Sans")
            .bg(rgb(theme.surface))
            .text_color(rgb(theme.text))
            .child(
                self.header
                    .render("Text Editor", tools, &focus, theme, window),
            )
            .when(self.pages.len() == 1, |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .bg(rgb(theme.panel))
                        .child(
                            div()
                                .flex_1()
                                .truncate()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(name),
                        )
                        .when(dirty, |el| {
                            el.child(
                                Role::Status
                                    .control("dirty", "Unsaved changes")
                                    .text_sm()
                                    .text_color(rgb(theme.focus))
                                    .child("●  Unsaved changes"),
                            )
                        }),
                )
            })
            .when(self.pages.len() > 1, |el| el.child(self.strip(cx)))
            .when(self.going, |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .bg(rgb(theme.panel))
                        .child("Go to line")
                        .child(div().w(px(110.)).child(self.line.clone()))
                        .child(self.tool(Command::Jump, &self.find_buttons[0], cx))
                        .child(self.tool(Command::Done, &self.find_buttons[1], cx)),
                )
            })
            .when(self.page().recovered, |el| {
                el.child(
                    Role::Status
                        .control("recovered", "Recovered draft")
                        .px_3()
                        .child("Recovered draft · Save to keep these changes."),
                )
            })
            .when(self.showing_recent, |el| {
                el.child(
                    div()
                        .h(px(150.))
                        .flex()
                        .flex_col()
                        .child("Recent files")
                        .when(self.recent.rows.is_empty(), |el| {
                            el.child("No recent files.")
                        })
                        .when(!self.closed.is_empty(), |el| {
                            el.child(self.tool(Command::Reopen, &self.reopen_focus, cx))
                        })
                        .child(recent),
                )
            })
            .when(self.searching, |el| el.child(self.find_bar(cx)))
            .when(self.searching && self.replacing, |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_1()
                        .child(div().flex_1().child(self.replacement.clone()))
                        .child(self.tool(Command::ReplaceOne, &self.replace_tools[1], cx))
                        .child(self.tool(Command::ReplaceAll, &self.replace_tools[2], cx))
                        .when_some(self.result.clone(), |el, s| {
                            el.child(Role::Status.control("replaced", s.clone()).child(s))
                        }),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.page().body.clone()),
            )
            .when_some(self.error.clone(), |el, err| {
                el.child(
                    Role::Status
                        .control("error", "File error")
                        .aria_description(err.clone())
                        .px_3()
                        .py_1()
                        .child(err),
                )
            })
            .child(
                Role::Status
                    .control("status", "Document status")
                    .aria_description(format!("Line {row}, Column {col}, UTF-8, {ending}"))
                    .flex()
                    .justify_between()
                    .px_3()
                    .py_1()
                    .bg(rgb(theme.panel))
                    .text_color(rgb(theme.secondary))
                    .text_sm()
                    .items_center()
                    .child(format!("Line {row}, Column {col}"))
                    .when(!self.going, |el| {
                        el.child(self.tool(Command::Line, &self.line_focus, cx))
                    })
                    .child(
                        mundane_ui::button_base(
                            format!("Indentation: {}", self.indent.label()),
                            &self.indent_focus,
                            theme,
                            {
                                let v = cx.weak_entity();
                                move |_, cx| {
                                    let _ = v.update(cx, |s, cx| s.cycle_indent(cx));
                                }
                            },
                        )
                        .px_2()
                        .child(self.indent.label()),
                    )
                    .child(format!("UTF-8   ·   {ending}")),
            );
        div()
            .relative()
            .size_full()
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, w, cx| {
                if this.indent_key(&event.keystroke, w, cx) {
                    w.prevent_default();
                    cx.stop_propagation();
                    return;
                }
                if event.keystroke.modifiers.control
                    && event.keystroke.modifiers.shift
                    && event.keystroke.key == "i"
                    && this.confirmation.is_none()
                    && this.busy.is_none()
                {
                    this.cycle_indent(cx);
                    w.prevent_default();
                    cx.stop_propagation();
                    return;
                }
                if event.keystroke.modifiers.control
                    && matches!(event.keystroke.key.as_str(), "pageup" | "pagedown")
                {
                    this.keys(event, w, cx);
                }
            }))
            .on_key_down(cx.listener(Self::keys))
            .font_family("DejaVu Sans")
            .text_color(rgb(theme.text))
            .child(body)
            .children(
                self.confirmation
                    .as_ref()
                    .map(|dialog| self.prompt(dialog, cx)),
            )
            .when_some(self.busy, |el, message| {
                el.child(
                    Role::Status
                        .control("busy", message)
                        .track_focus(&self.busy_focus)
                        .absolute()
                        .inset_0()
                        .occlude()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgba(0x00000070))
                        .child(message),
                )
            })
    }
}
