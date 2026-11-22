use super::*;
use mundane_ui::{ControlRole, Role, button, focus_ring, unavailable};

pub(super) fn page_text(area: Stateful<Div>, text: SharedString) -> Stateful<Div> {
    area.a11y_synthetic_children(move |tree| {
        for (line, value) in text.split_inclusive('\n').enumerate() {
            let mut run = accesskit::Node::new(Role::TextRun);
            run.set_character_lengths(
                value
                    .chars()
                    .map(|c| c.len_utf8() as u8)
                    .collect::<Vec<_>>(),
            );
            run.set_value(value);
            tree.push_child(tree.synthetic_node_id(line), run);
        }
    })
}

impl Viewer {
    fn tool(&self, a: Action, cx: &Context<Self>) -> Stateful<Div> {
        let theme = Theme::current(cx);
        let (i, label) = match a {
            Action::Open => (0, "Open"),
            Action::Find => (1, "Find"),
            Action::Previous => (2, "Previous page"),
            Action::Next => (3, "Next page"),
            Action::Smaller => (4, "Zoom out"),
            Action::Larger => (5, "Zoom in"),
            Action::PreviousHit => (6, "Previous match"),
            Action::NextHit => (7, "Next match"),
            Action::Done => (8, "Done"),
        };
        let view = cx.weak_entity();
        let disabled = !self.enabled(a);
        unavailable(
            button(label, &self.tools[i], theme, move |w, cx| {
                let _ = view.update(cx, |this, cx| this.act(a, w, cx));
            }),
            disabled,
        )
        .when(disabled, |el| el.text_color(rgb(theme.secondary)))
    }

    fn keyboard(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let key = &e.keystroke;
        let ctrl = key.modifiers.control;
        if self.content.is_focused(w) && self.select_keys(key, cx) {
            w.prevent_default();
            cx.stop_propagation();
            return;
        }
        match (key.key.as_str(), ctrl) {
            ("j", true) => self.toggle_flow(w, cx),
            ("p", true) => self.print(cx),
            ("b", true)
                if self
                    .state
                    .as_ref()
                    .is_some_and(|s| !s.book.outline.is_empty()) =>
            {
                self.outline.update(cx, |m, cx| m.toggle(w, cx))
            }
            ("up" | "down", false) if self.flow.is_some() && self.content.is_focused(w) => {
                let list = self.flow.as_ref().unwrap().list.clone();
                list.scroll_by(px(if key.key == "up" { -48. } else { 48. }));
                self.reading_position(list.logical_scroll_top().item_ix as i32 + 1, w, cx);
                cx.notify();
            }
            ("w", true) => w.remove_window(),
            ("o", true) => self.act(Action::Open, w, cx),
            ("f", true) => self.act(Action::Find, w, cx),
            ("l", true) if self.state.is_some() => self.page_field.update(cx, |f, cx| {
                f.select_match(0..f.text().len(), cx);
                f.focus_handle(cx).focus(w, cx);
            }),
            ("enter", false)
                if self.page_field.read(cx).focus_handle(cx).is_focused(w) && !self.busy =>
            {
                self.jump(w, cx)
            }
            ("enter", false)
                if self.finding && self.query.read(cx).focus_handle(cx).is_focused(w) =>
            {
                self.act(
                    if key.modifiers.shift {
                        Action::PreviousHit
                    } else {
                        Action::NextHit
                    },
                    w,
                    cx,
                )
            }
            ("f3", false) if self.finding => self.act(
                if key.modifiers.shift {
                    Action::PreviousHit
                } else {
                    Action::NextHit
                },
                w,
                cx,
            ),
            ("pagedown", false) => self.act(Action::Next, w, cx),
            ("pageup", false) => self.act(Action::Previous, w, cx),
            ("+" | "=" | "plus" | "equal", true) => self.act(Action::Larger, w, cx),
            ("-" | "minus", true) => self.act(Action::Smaller, w, cx),
            ("0", true) if self.state.is_some() => self.request(Request::Zoom(NORMAL), w, cx),
            ("left" | "right" | "up" | "down", false) if self.content.is_focused(w) => {
                let (dx, dy) = match key.key.as_str() {
                    "left" => (48., 0.),
                    "right" => (-48., 0.),
                    "up" => (0., 48.),
                    _ => (0., -48.),
                };
                let max = self.scroll.max_offset();
                let pos = self.scroll.offset() + point(px(dx), px(dy));
                self.scroll.set_offset(point(
                    pos.x.clamp(-max.x, px(0.)),
                    pos.y.clamp(-max.y, px(0.)),
                ));
                cx.notify();
            }
            ("escape", false) if self.finding => self.act(Action::Done, w, cx),
            ("escape", false) => self.content.focus(w, cx),
            ("f6", false) => self.tools[0].focus(w, cx),
            ("tab", false) if key.modifiers.shift => w.focus_prev(cx),
            ("tab", false) => w.focus_next(cx),
            _ => return,
        }
        w.prevent_default();
        cx.stop_propagation();
    }

    fn viewport(&self, cx: &Context<Self>) -> Stateful<Div> {
        let name = self.state.as_ref().map_or_else(
            || "Document".to_owned(),
            |s| format!("Page {} of {}", s.page, s.book.pages),
        );
        let area = focus_ring(
            Role::Document.control("viewport", name),
            &self.content,
            Theme::current(cx),
        )
        .flex_1()
        .min_h_0()
        .overflow_scroll()
        .track_scroll(&self.scroll)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, w, cx| this.content.focus(w, cx)),
        );
        if let Some(flow) = &self.flow {
            let view = cx.weak_entity();
            let s = self.state.as_ref().unwrap();
            let width = px(s.book.max_width * ZOOMS[s.zoom] + 24.);
            return focus_ring(
                Role::Document.control("continuous-document", "Continuous document"),
                &self.content,
                Theme::current(cx),
            )
            .overflow_x_scroll()
            .track_scroll(&self.scroll)
            .flex_1()
            .min_h_0()
            .child(
                list(flow.list.clone(), move |i, w, cx| {
                    view.update(cx, |v, cx| v.flow_row(i, w, cx))
                        .unwrap_or_else(|_| div().into_any_element())
                })
                .size_full()
                .min_w(width),
            );
        }
        let Some(sheet) = &self.sheet else {
            return area
                .flex()
                .items_center()
                .justify_center()
                .child(if self.state.is_some() {
                    "Page unavailable"
                } else if self.error.is_some() {
                    "Document unavailable"
                } else {
                    "Open a PDF document"
                });
        };
        let text = sheet.text.clone();
        let area = page_text(area, text);
        let page = self.paper(self.rendered_page, sheet, cx);
        area.child(
            div()
                .min_w_full()
                .w(px(sheet.logical.0 + 24.))
                .p_3()
                .flex()
                .justify_center()
                .child(page),
        )
    }
}

impl Render for Viewer {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.raster_scale != w.scale_factor()
            && !self.busy
            && let Some(s) = &self.state
        {
            self.raster_scale = w.scale_factor();
            self.request(Request::Zoom(s.zoom), w, cx);
        }
        let theme = Theme::current(cx);
        let row = || {
            div()
                .flex()
                .flex_none()
                .items_center()
                .flex_wrap()
                .gap_1()
                .px_2()
                .py_1()
                .bg(rgb(theme.panel))
        };
        let actions = div()
            .flex()
            .gap_1()
            .child(self.tool(Action::Open, cx))
            .child(self.tool(Action::Find, cx))
            .when(self.state.is_some(), |bar| {
                let v = cx.weak_entity();
                let print = v.clone();
                bar.child(
                    button("Continuous", &self.flow_button, theme, move |w, cx| {
                        let _ = v.update(cx, |v, cx| v.toggle_flow(w, cx));
                    })
                    .aria_toggled(self.flow.is_some().into()),
                )
                .when(
                    self.state
                        .as_ref()
                        .is_some_and(|s| !s.book.outline.is_empty()),
                    |el| {
                        el.child(mundane_ui::menu_trigger(
                            "Outline",
                            &self.outline,
                            theme,
                            cx,
                        ))
                    },
                )
                .child(button(
                    "Print…",
                    &self.print_button,
                    theme,
                    move |_, cx| {
                        let _ = print.update(cx, |v, cx| v.print(cx));
                    },
                ))
            });
        div()
            .size_full()
            .flex()
            .flex_col()
            .font_family("DejaVu Sans")
            .text_color(rgb(theme.text))
            .bg(rgb(theme.surface))
            .on_key_down(cx.listener(Self::keyboard))
            .child(
                self.header
                    .render("Document Viewer", actions, &self.content, theme, w),
            )
            .when_some(self.state.as_ref(), |el, s| {
                el.child(
                    row()
                        .child(self.tool(Action::Previous, cx))
                        .child(div().w(px(54.)).child(self.page_field.clone()))
                        .child(format!("/ {}", s.book.pages))
                        .child(self.tool(Action::Next, cx))
                        .child(div().flex_1())
                        .child(self.tool(Action::Smaller, cx))
                        .child(format!("{:.0}%", ZOOMS[s.zoom] * 100.))
                        .child(self.tool(Action::Larger, cx)),
                )
            })
            .when(self.finding, |el| {
                el.child(
                    row()
                        .child(div().flex_1().min_w(px(110.)).child(self.query.clone()))
                        .child(self.tool(Action::PreviousHit, cx))
                        .child(self.tool(Action::NextHit, cx))
                        .child(self.tool(Action::Done, cx)),
                )
            })
            .child(self.outline.clone())
            .child(self.viewport(cx))
            .when_some(self.state.as_ref(), |el, s| {
                let progress = if self.finding && !s.matches.query.is_empty() {
                    match s.matches.index {
                        Some(i) => format!("Match {} of {}", i + 1, s.matches.hits.len()),
                        None if s.matches.hits.is_empty() => "No matches".into(),
                        None => format!("{} matches", s.matches.hits.len()),
                    }
                } else {
                    format!("Page {} of {}", s.page, s.book.pages)
                };
                let description = format!(
                    "{} · {} · {:.0}%",
                    s.book.name,
                    progress,
                    ZOOMS[s.zoom] * 100.
                );
                el.child(
                    Role::Status
                        .control("status", "Document status")
                        .aria_description(description)
                        .flex()
                        .justify_between()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_1()
                        .bg(rgb(theme.panel))
                        .text_sm()
                        .child(div().truncate().child(s.book.name.clone()))
                        .child(div().flex_none().child(progress)),
                )
            })
            .when_some(
                self.error
                    .clone()
                    .or_else(|| self.busy.then(|| "Loading…".into())),
                |el, msg| {
                    el.child(
                        Role::Status
                            .control("message", msg.clone())
                            .px_3()
                            .py_1()
                            .child(msg),
                    )
                },
            )
    }
}
