use super::*;
use mundane_ui::{ControlRole, Role, button, button_base, unavailable};

impl Calculator {
    fn row(&self, row: usize, cx: &Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        div()
            .flex()
            .flex_none()
            .gap_2()
            .children((row * 4..row * 4 + 4).map(|i| {
                let (glyph, name) = KEYS[i];
                let view = cx.weak_entity();
                button_base(name, &self.keys[i], theme, move |w, cx| {
                    let _ = view.update(cx, |this, cx| this.activate_key(i, w, cx));
                })
                .flex_1()
                .min_w_0()
                .h(px(36.))
                .flex()
                .items_center()
                .justify_center()
                .bg(rgb(if glyph == "=" {
                    theme.selection
                } else {
                    theme.panel
                }))
                .text_size(px(17.))
                .child(glyph)
            }))
    }

    fn history_row(&self, entry: &Entry, cx: &Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        let view = cx.weak_entity();
        let answer = arithmetic::operand(&entry.answer);
        button_base(
            format!("Reuse {} = {}", entry.expression, entry.answer),
            &entry.focus,
            theme,
            move |w, cx| {
                let _ = view.update(cx, |this, cx| this.edit(&answer, true, w, cx));
            },
        )
        .flex_none()
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .py_1()
        .child(
            div()
                .flex_1()
                .truncate()
                .text_color(rgb(theme.secondary))
                .child(entry.expression.clone()),
        )
        .child(
            div()
                .max_w(relative(0.55))
                .truncate()
                .font_weight(FontWeight::SEMIBOLD)
                .child(entry.answer.clone()),
        )
    }

    // The scrolling body has the display, five keypad rows, a history heading,
    // then one child per history entry. Follow the keyboard down a long tape.
    fn reveal_focus(&mut self, window: &Window, cx: &App) {
        let focused = window.focused(cx);
        if self.last_focus == focused {
            return;
        }
        self.last_focus = focused.clone();
        let Some(focus) = focused else { return };
        let index = if let Some(key) = self.keys.iter().position(|key| *key == focus) {
            1 + key / 4
        } else if focus == self.clear {
            6
        } else if let Some(index) = self.history.iter().position(|e| e.focus == focus) {
            7 + index
        } else {
            0
        };
        self.scroll.scroll_to_item(index);
    }
}

impl Render for Calculator {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.reveal_focus(window, cx);
        let theme = Theme::current(cx);
        let result = self.result(cx);
        let view = cx.weak_entity();
        let reuse = unavailable(
            button("Use result", &self.reuse, theme, move |w, cx| {
                let _ = view.update(cx, |this, cx| this.use_result(w, cx));
            }),
            !matches!(result, Some(Ok(_))),
        );
        let result_box = match result {
            Some(Ok(answer)) => Role::Status
                .control("result", "Result")
                .aria_description(answer.to_owned())
                .text_size(px(28.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(answer.to_owned()),
            Some(Err(message)) => Role::Status
                .control("result", "Calculation error")
                .aria_description(message.to_owned())
                .text_sm()
                .child(message.to_owned()),
            None => Role::Status.control("result", "Result"),
        }
        .min_h(px(40.))
        .flex()
        .items_center()
        .justify_end()
        .overflow_hidden();
        let mut body = div()
            .id("calculator-body")
            .track_scroll(&self.scroll)
            .overflow_y_scroll()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .p_3()
            .gap_2()
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(self.field.clone())
                    .child(result_box),
            );
        for row in 0..5 {
            body = body.child(self.row(row, cx));
        }
        let view = cx.weak_entity();
        let clear = button("Clear history", &self.clear, theme, move |w, cx| {
            let _ = view.update(cx, |this, cx| this.clear_history(w, cx));
        });
        body = body
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt_2()
                    .child(mundane_ui::heading("history-heading", "History"))
                    .child(unavailable(clear, self.history.is_empty())),
            )
            .children(self.history.iter().map(|entry| self.history_row(entry, cx)));
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(theme.surface))
            .text_color(rgb(theme.text))
            .font_family("DejaVu Sans")
            .on_key_down(cx.listener(Self::keyboard))
            .child(self.header.render(
                "Calculator",
                reuse,
                &self.field.read(cx).focus_handle(cx),
                theme,
                window,
            ))
            .child(body)
    }
}
