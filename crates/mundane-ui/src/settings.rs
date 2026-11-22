//! Stateless service controls. The caller publishes the actual value after a change.
use super::*;
use accesskit::{Action, ActionData};
use std::{cell::Cell, rc::Rc};

pub struct TileState<'a> {
    pub label: &'a str,
    pub status: &'a str,
    pub on: bool,
    pub enabled: bool,
    pub error: Option<&'a str>,
}
pub fn quick_tile(
    state: TileState<'_>,
    icon: impl IntoElement,
    f: &FocusHandle,
    t: Theme,
    request: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let on = state.on;
    let label: SharedString = state.label.to_string().into();
    let status = state.status.to_string();
    let error = state.error.map(str::to_string);
    let summary = error
        .as_ref()
        .map_or_else(|| status.clone(), |e| format!("{status}. {e}"));
    let tile = focus_ring(Role::Button.control(f, label.clone()), f, t)
        .aria_toggled(on.into())
        .aria_value(status.clone())
        .aria_description(summary)
        .rounded(tokens::radius::DIALOG)
        .p(tokens::space::MD)
        .flex()
        .items_center()
        .gap(tokens::space::SM)
        .bg(rgb(if on { t.selected() } else { t.surface }))
        .text_color(rgb(if state.enabled { t.text } else { t.disabled() }))
        .child(icon)
        .child(
            div()
                .flex()
                .flex_col()
                .child(label)
                .child(div().text_size(tokens::text::CONTROL).child(status))
                .when_some(error, |el, e| {
                    el.child(
                        Role::Status
                            .control("error", e.clone())
                            .text_color(rgb(t.error()))
                            .child(e),
                    )
                }),
        )
        .when(state.enabled, |el| {
            el.cursor_pointer()
                .on_click(move |_, w, cx| request(!on, w, cx))
        });
    unavailable(tile, !state.enabled)
}
pub fn labelled_switch(
    label: impl Into<SharedString>,
    f: &FocusHandle,
    on: bool,
    enabled: bool,
    t: Theme,
    request: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let label = label.into();
    let el = focus_ring(Role::Switch.control(f, label.clone()), f, t)
        .aria_toggled(on.into())
        .aria_value(if on { "On" } else { "Off" })
        .rounded(tokens::radius::CONTROL)
        .px(tokens::space::SM)
        .py(tokens::space::XS)
        .flex()
        .gap_2()
        .items_center()
        .justify_between()
        .text_color(rgb(if enabled { t.text } else { t.disabled() }))
        .child(label)
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(if on { "On" } else { "Off" })
                .child(
                    div()
                        .w(rems(2.))
                        .h(rems(1.))
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(t.border))
                        .flex()
                        .items_center()
                        .when(on, |el| el.justify_end().bg(rgb(t.selected())))
                        .child(div().size(rems(0.75)).rounded_full().bg(rgb(if enabled {
                            t.text
                        } else {
                            t.disabled()
                        }))),
                ),
        )
        .when(enabled, |el| {
            el.cursor_pointer()
                .on_click(move |_, w, cx| request(!on, w, cx))
        });
    unavailable(el, !enabled)
}

#[derive(Clone, Copy, Debug)]
pub struct SliderRange {
    min: f64,
    max: f64,
    step: f64,
}
impl SliderRange {
    pub fn new(min: f64, max: f64, step: f64) -> Option<Self> {
        (min.is_finite()
            && max.is_finite()
            && (max - min).is_finite()
            && min < max
            && step.is_finite()
            && step > 0.)
            .then_some(Self { min, max, step })
    }
    fn snap(self, v: f64) -> f64 {
        if !v.is_finite() || v <= self.min {
            return self.min;
        }
        if v >= self.max {
            return self.max;
        }
        (self.min + ((v - self.min) / self.step).round() * self.step).clamp(self.min, self.max)
    }
}
pub struct ValueSlider {
    focus: FocusHandle,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    dragging: Rc<Cell<bool>>,
}
impl ValueSlider {
    pub fn new(cx: &App) -> Self {
        Self {
            focus: cx.focus_handle().tab_stop(true),
            bounds: Default::default(),
            dragging: Default::default(),
        }
    }
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }
    pub fn render(
        &self,
        label: impl Into<SharedString>,
        range: SliderRange,
        value: f64,
        enabled: bool,
        t: Theme,
        change: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Stateful<Div> {
        let label = label.into();
        let value = range.snap(value);
        let frac = ((value - range.min) / (range.max - range.min)) as f32;
        let request = Rc::new(move |v: f64, w: &mut Window, cx: &mut App| {
            if enabled && v.is_finite() {
                change(range.snap(v), w, cx);
            }
        });
        let bounds = self.bounds.clone();
        let focus = self.focus.clone();
        let dragged = self.dragging.clone();
        if !enabled {
            dragged.set(false);
        }
        let point_change = {
            let request = request.clone();
            let b = bounds.clone();
            Rc::new(move |x: Pixels, w: &mut Window, cx: &mut App| {
                let b = b.get();
                let fraction = f32::from(x - b.left()) / f32::from(b.size.width).max(1.);
                request(
                    range.min + f64::from(fraction.clamp(0., 1.)) * (range.max - range.min),
                    w,
                    cx,
                );
            })
        };
        let control = focus_ring(
            Role::Slider.control(&self.focus, label.clone()),
            &self.focus,
            t,
        )
        .aria_min_numeric_value(range.min)
        .aria_max_numeric_value(range.max)
        .aria_numeric_value_step(range.step)
        .aria_numeric_value(value)
        .aria_value(value.to_string())
        .p(tokens::space::SM)
        .rounded(tokens::radius::CONTROL)
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .justify_between()
                .child(label)
                .child(value.to_string()),
        )
        .when(enabled, |mut el| {
            for (action, delta) in [
                (Action::Increment, range.step),
                (Action::Decrement, -range.step),
            ] {
                let r = request.clone();
                el = el.on_a11y_action(action, move |_, w, cx| r(value + delta, w, cx));
            }
            let set = request.clone();
            let keys = request.clone();
            let down = point_change.clone();
            let move_to = point_change.clone();
            let held = dragged.clone();
            let moving = dragged.clone();
            let up = dragged.clone();
            el.on_a11y_action(Action::SetValue, move |data, w, cx| {
                if let Some(ActionData::NumericValue(v)) = data {
                    set(*v, w, cx);
                }
            })
            .on_key_down(move |e, w, cx| {
                let next = match e.keystroke.key.as_str() {
                    "left" | "down" => value - range.step,
                    "right" | "up" => value + range.step,
                    "home" => range.min,
                    "end" => range.max,
                    _ => return,
                };
                keys(next, w, cx);
                w.prevent_default();
                cx.stop_propagation();
            })
            .on_mouse_down(MouseButton::Left, move |e, w, cx| {
                focus.focus(w, cx);
                held.set(true);
                down(e.position.x, w, cx);
            })
            .on_mouse_move(move |e, w, cx| {
                if moving.get() && e.pressed_button == Some(MouseButton::Left) {
                    move_to(e.position.x, w, cx);
                }
            })
            .on_mouse_up(MouseButton::Left, move |_, _, _| up.set(false))
            .on_mouse_up_out(MouseButton::Left, move |_, _, _| dragged.set(false))
        })
        .child(
            canvas(
                move |b, _, _| bounds.set(b),
                move |b, (), w, _| {
                    let y = b.center().y;
                    let x = b.left() + b.size.width * frac;
                    w.paint_quad(fill(
                        Bounds::new(point(b.left(), y - px(2.)), size(b.size.width, px(4.))),
                        rgb(t.border),
                    ));
                    w.paint_quad(fill(
                        Bounds::new(point(b.left(), y - px(2.)), size(x - b.left(), px(4.))),
                        rgb(if enabled { t.focus } else { t.disabled() }),
                    ));
                    w.paint_quad(fill(
                        Bounds::new(
                            point(
                                (x - px(3.)).clamp(b.left(), (b.right() - px(6.)).max(b.left())),
                                y - px(6.),
                            ),
                            size(px(6.), px(12.)),
                        ),
                        rgb(t.text),
                    ));
                },
            )
            .w_full()
            .h(rems(1.)),
        );
        unavailable(control, !enabled)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Confirmation {
    Cancelled,
    Confirmed,
}
pub struct ConfirmDialog {
    title: SharedString,
    message: SharedString,
    accept: &'static str,
    cancel: FocusHandle,
    confirm: FocusHandle,
    return_to: Option<FocusHandle>,
}
impl EventEmitter<Confirmation> for ConfirmDialog {}
impl ConfirmDialog {
    pub fn new(
        title: impl Into<SharedString>,
        message: impl Into<SharedString>,
        accept: &'static str,
        cx: &App,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            accept,
            cancel: cx.focus_handle().tab_stop(true),
            confirm: cx.focus_handle().tab_stop(true),
            return_to: None,
        }
    }
    pub fn is_open(&self) -> bool {
        self.return_to.is_some()
    }
    pub fn show(&mut self, trigger: &FocusHandle, w: &mut Window, cx: &mut Context<Self>) {
        self.return_to = Some(trigger.clone());
        self.cancel.focus(w, cx);
        cx.notify();
    }
    pub fn finish(&mut self, answer: Confirmation, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(f) = self.return_to.take() {
            f.focus(w, cx);
            cx.emit(answer);
            cx.notify();
        }
    }
}
impl Render for ConfirmDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        div().when(self.is_open(), |el| {
            el.absolute().inset_0().child(
                Role::Dialog
                    .control("confirmation", self.title.clone())
                    .aria_description(self.message.clone())
                    .size_full()
                    .occlude()
                    .bg(rgba(0x00000070))
                    .flex()
                    .items_center()
                    .justify_center()
                    .capture_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                        match e.keystroke.key.as_str() {
                            "escape" => s.finish(Confirmation::Cancelled, w, cx),
                            "tab" => {
                                if s.cancel.is_focused(w) {
                                    s.confirm.focus(w, cx)
                                } else {
                                    s.cancel.focus(w, cx)
                                }
                            }
                            _ => return,
                        }
                        w.prevent_default();
                        cx.stop_propagation();
                    }))
                    .child(
                        div()
                            .max_w_full()
                            .w(rems(26.))
                            .p(tokens::space::MD)
                            .rounded(tokens::radius::DIALOG)
                            .border_1()
                            .border_color(rgb(t.border))
                            .bg(rgb(t.panel))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                Role::Heading
                                    .control("title", self.title.clone())
                                    .aria_level(1)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.title.clone()),
                            )
                            .child(self.message.clone())
                            .child(
                                div().flex().gap_2().justify_end().children(
                                    [
                                        ("Cancel", &self.cancel, Confirmation::Cancelled),
                                        (self.accept, &self.confirm, Confirmation::Confirmed),
                                    ]
                                    .map(
                                        |(name, focus, answer)| {
                                            let v = cx.weak_entity();
                                            button(name, focus, t, move |w, cx| {
                                                let _ =
                                                    v.update(cx, |s, cx| s.finish(answer, w, cx));
                                            })
                                        },
                                    ),
                                ),
                            ),
                    ),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SliderRange;
    #[test]
    fn range_validation_and_quantization() {
        assert!(SliderRange::new(0., 0., 1.).is_none());
        assert!(SliderRange::new(0., 5., f64::NAN).is_none());
        let range = SliderRange::new(-10., 12., 5.).unwrap();
        assert_eq!(range.snap(2.9), 5.);
        assert_eq!(range.snap(13.), 12.);
        assert_eq!(range.snap(-25.), -10.);
    }
}
