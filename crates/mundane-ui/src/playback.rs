//! Transport buttons and an accessible, keyboard-operable seek slider.
use super::{ControlRole, Theme, button, button_base, focus_ring, unavailable};
use accesskit::{Action, ActionData, Role};
use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy)]
pub enum TransportAction {
    Toggle,
    Step(i64),
    Seek(f64),
}

#[derive(Clone, Copy)]
pub struct TransportState {
    pub playing: bool,
    pub ended: bool,
    pub available: bool,
    pub position: u64,
    pub duration: Option<u64>,
}

pub fn playback_time(ms: u64) -> String {
    let seconds = ms / 1000;
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            (seconds / 60) % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

pub struct PlaybackControls {
    focus: [FocusHandle; 4],
    bounds: Rc<Cell<Bounds<Pixels>>>,
    drag: Rc<Cell<Option<f64>>>,
}
type Activate = Rc<dyn Fn(TransportAction, &mut Window, &mut App)>;

impl PlaybackControls {
    pub fn new(cx: &App) -> Self {
        Self {
            focus: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            bounds: Default::default(),
            drag: Default::default(),
        }
    }
    fn slider(&self, state: TransportState, theme: Theme, activate: Activate) -> impl IntoElement {
        let bounds = self.bounds.clone();
        let fraction_at = move |p: Point<Pixels>| {
            let b = bounds.get();
            (f64::from(f32::from(p.x - b.origin.x)) / f64::from(f32::from(b.size.width).max(1.)))
                .clamp(0., 1.)
        };
        let dur = state.duration.unwrap_or_default();
        let enabled = state.available && dur > 0;
        if !enabled {
            self.drag.set(None);
        }
        let fraction = self
            .drag
            .get()
            .unwrap_or_else(|| (state.position as f64 / dur.max(1) as f64).clamp(0., 1.));
        let position = (fraction * dur as f64).round() as u64;
        let focus = self.focus[0].clone();
        let control = focus_ring(
            Role::Slider.control(&focus, "Playback position"),
            &focus,
            theme,
        )
        .flex_1()
        .min_w(px(60.))
        .h(px(28.))
        .rounded_md()
        .px_2()
        .flex()
        .items_center()
        .aria_min_numeric_value(0.)
        .aria_max_numeric_value(dur as f64 / 1000.)
        .aria_numeric_value(position as f64 / 1000.)
        .aria_numeric_value_step(5.)
        .aria_value(format!(
            "{} of {}",
            playback_time(position),
            playback_time(dur)
        ));
        let control = control.when(enabled, |mut el| {
            for (action, cmd) in [
                (Action::Increment, TransportAction::Step(5000)),
                (Action::Decrement, TransportAction::Step(-5000)),
            ] {
                let activate = activate.clone();
                el = el.on_a11y_action(action, move |_, w, cx| activate(cmd, w, cx));
            }
            let keyboard = activate.clone();
            el = el.on_key_down(move |event, w, cx| {
                let cmd = match event.keystroke.key.as_str() {
                    "left" | "down" => TransportAction::Step(-5000),
                    "right" | "up" => TransportAction::Step(5000),
                    "home" => TransportAction::Seek(0.),
                    "end" => TransportAction::Seek(1.),
                    _ => return,
                };
                keyboard(cmd, w, cx);
                w.prevent_default();
                cx.stop_propagation();
            });
            let send = activate.clone();
            let down = self.drag.clone();
            let movement = self.drag.clone();
            let end = self.drag.clone();
            let at = Rc::new(fraction_at);
            let start = at.clone();
            let update = at.clone();
            let release = move |e: &MouseUpEvent, w: &mut Window, cx: &mut App| {
                if end.take().is_some() {
                    send(TransportAction::Seek(at(e.position)), w, cx);
                    w.refresh();
                }
            };
            el.cursor_pointer()
                .on_a11y_action(Action::SetValue, move |data, w, cx| {
                    if let Some(ActionData::NumericValue(v)) = data
                        && v.is_finite()
                    {
                        activate(TransportAction::Seek(v * 1000. / dur as f64), w, cx);
                    }
                })
                .on_mouse_down(MouseButton::Left, move |e, w, cx| {
                    focus.focus(w, cx);
                    down.set(Some(start(e.position)));
                    w.refresh();
                })
                .on_mouse_move(move |e, w, _| {
                    if movement.get().is_some() {
                        movement.set(Some(update(e.position)));
                        w.refresh();
                    }
                })
                .on_mouse_up(MouseButton::Left, release.clone())
                .on_mouse_up_out(MouseButton::Left, release)
        });
        let bounds = self.bounds.clone();
        unavailable(control, !enabled).child(
            canvas(
                move |b, _, _| bounds.set(b),
                move |b, (), w, _| {
                    let y = b.center().y;
                    let x = b.left() + b.size.width * fraction as f32;
                    w.paint_quad(fill(
                        Bounds::new(point(b.left(), y - px(2.)), size(b.size.width, px(4.))),
                        rgb(theme.border),
                    ));
                    w.paint_quad(fill(
                        Bounds::new(point(b.left(), y - px(2.)), size(x - b.left(), px(4.))),
                        rgb(theme.focus),
                    ));
                    let thumb = Bounds::new(point(x - px(4.), y - px(7.)), size(px(8.), px(14.)));
                    w.paint_quad(quad(
                        thumb,
                        px(4.),
                        rgb(theme.text),
                        px(1.),
                        rgb(theme.surface),
                        BorderStyle::Solid,
                    ));
                },
            )
            .size_full(),
        )
    }

    pub fn render(
        &self,
        state: TransportState,
        theme: Theme,
        activate: impl Fn(TransportAction, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let activate: Activate = Rc::new(activate);
        let toggle = activate.clone();
        let label = if state.playing {
            "Pause"
        } else if state.ended {
            "Replay"
        } else {
            "Play"
        };
        let step = |index, ms, label, glyph| {
            let send = activate.clone();
            unavailable(
                button_base(label, &self.focus[index], theme, move |w, cx| {
                    if state.available && state.duration.is_some() {
                        send(TransportAction::Step(ms), w, cx);
                    }
                })
                .px_3()
                .py_1()
                .child(glyph),
                !state.available || state.duration.is_none(),
            )
        };
        div()
            .flex()
            .flex_none()
            .flex_col()
            .gap_1()
            .px_3()
            .py_2()
            .bg(rgb(theme.panel))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .child(playback_time(state.position))
                    .child(self.slider(state, theme, activate.clone()))
                    .child(
                        state
                            .duration
                            .map(playback_time)
                            .unwrap_or_else(|| "–:––".into()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(step(1, -5000, "Back 5 seconds", "−5 s"))
                    .child(unavailable(
                        button(label, &self.focus[2], theme, move |w, cx| {
                            if state.available {
                                toggle(TransportAction::Toggle, w, cx);
                            }
                        })
                        .min_w(px(84.))
                        .text_center(),
                        !state.available,
                    ))
                    .child(step(3, 5000, "Forward 5 seconds", "+5 s")),
            )
    }
}
