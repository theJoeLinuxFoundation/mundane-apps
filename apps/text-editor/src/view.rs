use super::*;
use mundane_ui::ControlRole as _;
use mundane_ui::{Role, button};

impl Editor {
    fn tool(&self, cmd: Command, focus: &FocusHandle, cx: &Context<Self>) -> Stateful<Div> {
        let view = cx.weak_entity();
        button(cmd.label(), focus, self.theme, move |window, cx| {
            let _ = view.update(cx, |this, cx| this.command(cmd, window, cx));
        })
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
                ("o", true) => self.command(Command::Open, window, cx),
                ("s", true) if key.modifiers.shift => self.command(Command::SaveAs, window, cx),
                ("s", true) => self.command(Command::Save, window, cx),
                ("f", true) => self.command(Command::Find, window, cx),
                ("w", true) => self.request(Pending::Close, window, cx),
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
            self.body.read(cx).text().matches(term).count()
        };
        div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap_2()
            .px_3()
            .py_2()
            .bg(rgb(self.theme.panel))
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
            .child(self.tool(Command::Next, &self.find_buttons[0], cx))
            .child(self.tool(Command::Done, &self.find_buttons[1], cx))
    }

    fn prompt(&self, confirm: &Confirmation, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme;
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
        let theme = self.theme;
        let name = self.name();
        let dirty = self.dirty(cx);
        let title = format!("{}{} — Text Editor", if dirty { "• " } else { "" }, name);
        if window.window_title() != title {
            window.set_window_title(&title);
        }
        let body = self.body.read(cx);
        let focus = body.focus_handle(cx);
        let (row, col) = body.line_column();
        let ending = body.line_ending();
        let tools = div().flex().gap_1().children(
            [Command::Open, Command::Find, Command::Save, Command::SaveAs]
                .into_iter()
                .zip(&self.tools)
                .map(|(cmd, focus)| self.tool(cmd, focus, cx)),
        );
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
            .child(
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
            .when(self.searching, |el| el.child(self.find_bar(cx)))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.body.clone()),
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
                    .child(format!("Line {row}, Column {col}"))
                    .child(format!("UTF-8   ·   {ending}")),
            );
        div()
            .relative()
            .size_full()
            .on_key_down(cx.listener(Self::keys))
            .font_family("DejaVu Sans")
            .text_color(rgb(self.theme.text))
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
