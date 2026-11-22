//! Caller-owned control state. Icons can be symbolic artwork or text.
use super::*;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PanelState {
    pub active: bool,
    pub running: bool,
    pub occupied: bool,
    pub disabled: bool,
}
impl PanelState {
    fn description(self) -> String {
        [
            (self.active, "Active"),
            (self.running, "Running"),
            (self.occupied, "Occupied"),
            (self.disabled, "Unavailable"),
        ]
        .into_iter()
        .filter_map(|(yes, label)| yes.then_some(label))
        .collect::<Vec<_>>()
        .join(", ")
    }
}
pub fn panel_button(
    label: impl Into<SharedString>,
    icon: impl IntoElement,
    focus: &FocusHandle,
    state: PanelState,
    t: Theme,
    activate: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let control = focus_ring(Role::Button.control(focus, label), focus, t)
        .aria_description(state.description())
        .rounded(tokens::radius::CONTROL)
        .min_w(rems(2.))
        .min_h(rems(2.))
        .px(tokens::space::SM)
        .gap(tokens::space::XS)
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(if state.disabled { t.disabled() } else { t.text }))
        .when(state.active, |el| el.bg(rgb(t.selected())))
        .when(!state.disabled, |el| {
            el.cursor_pointer()
                .hover(move |s| s.bg(rgb(t.selected())))
                .on_click(move |_, w, cx| activate(w, cx))
        })
        .child(icon)
        .when(state.running, |el| el.child("•"))
        .when(state.occupied, |el| el.child("◆"));
    unavailable(control, state.disabled)
}
pub fn toggle(
    label: impl Into<SharedString>,
    focus: &FocusHandle,
    on: bool,
    enabled: bool,
    t: Theme,
    change: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let label = label.into();
    let control = focus_ring(Role::Button.control(focus, label.clone()), focus, t)
        .aria_toggled(on.into())
        .aria_value(if on { "On" } else { "Off" })
        .rounded(tokens::radius::CONTROL)
        .px(tokens::space::MD)
        .py(tokens::space::XS)
        .text_size(tokens::text::CONTROL)
        .text_color(rgb(if enabled { t.text } else { t.disabled() }))
        .when(on, |el| el.bg(rgb(t.selected())))
        .child(if on { "✓  " } else { "○  " })
        .child(label)
        .when(enabled, |el| {
            el.cursor_pointer()
                .on_click(move |_, w, cx| change(!on, w, cx))
        });
    unavailable(control, !enabled)
}
pub fn badge(
    id: impl Into<ElementId>,
    name: impl Into<SharedString>,
    value: impl Into<SharedString>,
    t: Theme,
) -> Stateful<Div> {
    let value = value.into();
    Role::Status
        .control(id, name)
        .aria_value(value.clone())
        .text_size(tokens::text::CAPTION)
        .rounded(tokens::radius::CONTROL)
        .border_1()
        .border_color(rgb(t.border))
        .px(tokens::space::SM)
        .text_color(rgb(t.text))
        .bg(rgb(t.panel))
        .child(value)
}
pub fn tooltip(id: impl Into<ElementId>, text: impl Into<SharedString>, t: Theme) -> Stateful<Div> {
    let text = text.into();
    Role::Tooltip
        .control(id, text.clone())
        .text_size(tokens::text::CONTROL)
        .max_w(rems(24.))
        .px(tokens::space::SM)
        .py(tokens::space::XS)
        .rounded(tokens::radius::CONTROL)
        .bg(rgb(t.panel))
        .border_1()
        .border_color(rgb(t.border))
        .text_color(rgb(t.text))
        .child(text)
}
struct Hint(SharedString);
impl Render for Hint {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        tooltip("hint", self.0.clone(), Theme::current(cx))
    }
}
/// The same help text is available to assistive technology when the trigger receives focus.
pub fn with_tooltip(control: Stateful<Div>, text: impl Into<SharedString>) -> Stateful<Div> {
    let text = text.into();
    control
        .aria_description(text.clone())
        .tooltip(move |_, cx| cx.new(|_| Hint(text.clone())).into())
}
