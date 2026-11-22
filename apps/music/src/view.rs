use super::*;
use mundane_ui::{
    ControlRole, Role, TransportState, button, focus_ring, playback_time, text_label, unavailable,
};
use std::ops::Range;

#[derive(Clone, Copy)]
enum Action {
    Previous,
    Next,
    PlaySelected,
    MoveUp,
    MoveDown,
    Remove,
}
impl Action {
    fn label(self) -> &'static str {
        match self {
            Self::Previous => "Previous track",
            Self::Next => "Next track",
            Self::PlaySelected => "Play selected",
            Self::MoveUp => "Move up",
            Self::MoveDown => "Move down",
            Self::Remove => "Remove",
        }
    }
}
impl Music {
    fn enabled(&self, act: Action) -> bool {
        let q = &self.queue;
        match act {
            Action::Previous => q.neighbor(q.current, -1).is_some(),
            Action::Next => q.neighbor(q.current, 1).is_some(),
            Action::PlaySelected | Action::Remove => q.selected.is_some(),
            Action::MoveUp => q.neighbor(q.selected, -1).is_some(),
            Action::MoveDown => q.neighbor(q.selected, 1).is_some(),
        }
    }
    fn act(&mut self, act: Action, w: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled(act) {
            return;
        }
        match act {
            Action::Previous => self.skip(-1, cx),
            Action::Next => self.skip(1, cx),
            Action::PlaySelected => {
                if let Some(id) = self.queue.selected {
                    self.play(id, cx)
                }
            }
            Action::MoveUp | Action::MoveDown => {
                self.queue
                    .move_selected(if matches!(act, Action::MoveUp) { -1 } else { 1 });
                self.reveal(cx)
            }
            Action::Remove => self.remove(w, cx),
        }
    }
    fn action_button(&self, act: Action, cx: &Context<Self>) -> Stateful<Div> {
        let theme = Theme::current(cx);
        let view = cx.weak_entity();
        let enabled = self.enabled(act);
        unavailable(
            button(
                act.label(),
                &self.buttons[act as usize],
                theme,
                move |w, cx| {
                    let _ = view.update(cx, |this, cx| this.act(act, w, cx));
                },
            ),
            !enabled,
        )
        .when(!enabled, |el| el.text_color(rgb(theme.secondary)))
    }
    fn keys(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        match (k.key.as_str(), k.modifiers.control) {
            ("o", true) => self.choose(w, cx),
            ("w", true) => w.remove_window(),
            ("up", true) => self.act(Action::MoveUp, w, cx),
            ("down", true) => self.act(Action::MoveDown, w, cx),
            ("+" | "=" | "-" | "0", true) => {
                let size = match k.key.as_str() {
                    "-" => w.rem_size() - px(2.),
                    "0" => px(16.),
                    _ => w.rem_size() + px(2.),
                };
                w.set_rem_size(size.clamp(px(12.), px(24.)));
                cx.notify()
            }
            ("n", false) => self.skip(1, cx),
            ("p", false) => self.skip(-1, cx),
            ("space", false) if self.list.is_focused(w) => self.transport(TransportAction::Toggle),
            ("enter", false) if self.list.is_focused(w) => self.act(Action::PlaySelected, w, cx),
            ("delete", false) if self.list.is_focused(w) => self.remove(w, cx),
            ("up" | "down", false) if self.list.is_focused(w) => {
                if let Some(id) = self
                    .queue
                    .neighbor(self.queue.selected, if k.key == "up" { -1 } else { 1 })
                {
                    self.select(id, w, cx)
                }
            }
            ("home" | "end", false) if self.list.is_focused(w) => {
                self.transport(TransportAction::Seek(if k.key == "home" { 0. } else { 1. }))
            }
            ("left" | "right", false) if self.list.is_focused(w) => {
                self.transport(TransportAction::Step(if k.key == "left" {
                    -5000
                } else {
                    5000
                }))
            }
            ("f6", false) => self.add_button.focus(w, cx),
            ("escape", false) => self.list.focus(w, cx),
            ("tab", false) if k.modifiers.shift => w.focus_prev(cx),
            ("tab", false) => w.focus_next(cx),
            _ => return,
        }
        w.prevent_default();
        cx.stop_propagation();
    }
    fn rows(
        &mut self,
        range: Range<usize>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Stateful<Div>> {
        let theme = Theme::current(cx);
        range
            .map(|i| {
                let t = &self.queue.tracks[i];
                let id = t.id;
                let title = t.title();
                let current = Some(id) == self.queue.current;
                let selected = Some(id) == self.queue.selected;
                let detail = t.error.clone().unwrap_or_else(|| {
                    [t.info.artist.as_deref(), t.info.album.as_deref()]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(" · ")
                });
                let time = t.info.duration.map(playback_time).unwrap_or_default();
                mundane_ui::option(
                    ElementId::NamedInteger("track".into(), id),
                    title.clone(),
                    i,
                    selected,
                    self.list.is_focused(w),
                )
                .aria_description(format!(
                    "{}{} {time}",
                    if current { "Current track. " } else { "" },
                    detail
                ))
                .h(rems(3.4))
                .w_full()
                .flex()
                .items_center()
                .gap_3()
                .px_3()
                .border_l_2()
                .border_color(rgb(if selected { theme.focus } else { theme.surface }))
                .when(selected, |el| el.bg(rgb(theme.selection)))
                .when(!selected, |el| el.hover(move |s| s.bg(rgb(theme.panel))))
                .on_click(cx.listener(move |this, event: &ClickEvent, w, cx| {
                    this.select(id, w, cx);
                    if event.click_count() == 2 {
                        this.play(id, cx)
                    }
                }))
                .child(
                    div()
                        .w(rems(2.))
                        .text_right()
                        .text_sm()
                        .child(format!("{:02}", i + 1)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .truncate()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        )
                        .when(!detail.is_empty(), |el| {
                            el.child(
                                div()
                                    .truncate()
                                    .text_sm()
                                    .text_color(rgb(theme.secondary))
                                    .child(detail),
                            )
                        }),
                )
                .when(current, |el| {
                    el.child(div().text_sm().child("Current track"))
                })
                .child(div().w(rems(3.)).text_right().child(time))
            })
            .collect()
    }
}
impl Render for Music {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        let v = cx.weak_entity();
        let add = button("Add tracks…", &self.add_button, theme, move |w, cx| {
            let _ = v.update(cx, |this, cx| this.choose(w, cx));
        });
        let phase = match self.status.phase {
            Phase::Playing => "Playing",
            Phase::Paused => "Paused",
            Phase::Ended => "Queue finished",
            Phase::Failed => "Playback failed",
            Phase::Empty if self.queue.current.is_some() => "Opening track…",
            Phase::Empty => "Queue empty",
        };
        let current = self
            .queue
            .position(self.queue.current)
            .map(|i| &self.queue.tracks[i]);
        let title = current
            .map(|t| t.title())
            .unwrap_or_else(|| "Add tracks to start playback".into());
        let info = current
            .map(|t| {
                [t.info.artist.as_deref(), t.info.album.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · ")
            })
            .unwrap_or_default();
        let state = TransportState {
            available: matches!(
                self.status.phase,
                Phase::Playing | Phase::Paused | Phase::Ended
            ),
            playing: self.status.phase == Phase::Playing,
            ended: self.status.phase == Phase::Ended,
            position: self.status.position,
            duration: self.status.duration,
        };
        let view = cx.weak_entity();
        let list = uniform_list(
            "music-rows",
            self.queue.tracks.len(),
            cx.processor(Self::rows),
        )
        .track_scroll(&self.scroll)
        .size_full();
        let actions = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .items_center()
            .child(self.action_button(Action::PlaySelected, cx))
            .child(self.action_button(Action::MoveUp, cx))
            .child(self.action_button(Action::MoveDown, cx))
            .child(self.action_button(Action::Remove, cx));
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(theme.surface))
            .text_color(rgb(theme.text))
            .font_family("DejaVu Sans")
            .on_key_down(cx.listener(Self::keys))
            .child(self.header.render("Music", add, &self.list, theme, w))
            .child(
                div()
                    .p_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                Role::Status
                                    .control("playback-status", "Playback status")
                                    .aria_description(format!("{phase} · {title} · {info}"))
                                    .text_sm()
                                    .text_color(rgb(theme.secondary))
                                    .child(phase),
                            )
                            .child(
                                text_label("current-title", title.clone())
                                    .truncate()
                                    .text_xl()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .when(!info.is_empty(), |el| {
                                el.child(
                                    text_label("current-info", info.clone())
                                        .truncate()
                                        .text_sm()
                                        .child(info),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(self.action_button(Action::Previous, cx))
                            .child(self.action_button(Action::Next, cx)),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .bg(rgb(theme.panel))
                    .child(
                        mundane_ui::heading("queue-heading", "Play queue")
                            .font_weight(FontWeight::SEMIBOLD),
                    )
                    .child(actions),
            )
            .child(
                focus_ring(
                    Role::ListBox
                        .control("play-queue", "Play queue")
                        .aria_size_of_set(self.queue.tracks.len()),
                    &self.list,
                    theme,
                )
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .child(list),
            )
            .child(self.transport.render(state, theme, move |cmd, _, cx| {
                let _ = view.update(cx, |this, _| this.transport(cmd));
            }))
            .when_some(
                self.notice.clone().or_else(|| self.status.error.clone()),
                |el, msg| {
                    el.child(
                        Role::Status
                            .control("music-error", msg.clone())
                            .px_3()
                            .py_1()
                            .child(msg),
                    )
                },
            )
    }
}
