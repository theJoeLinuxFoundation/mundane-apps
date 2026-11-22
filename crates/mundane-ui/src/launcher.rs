//! Flat actions for a launcher or a scrolling list of commands.
use super::*;
use std::rc::Rc;

/// Center caller-owned artwork inside a non-shrinking logical icon slot.
pub fn icon_slot(size: Rems, artwork: impl IntoElement) -> Div {
    div()
        .size(size)
        .flex_none()
        .overflow_hidden()
        .flex()
        .items_center()
        .justify_center()
        .child(artwork)
}

/// Add the icon and visible label as children. The stable focus handle identifies
/// the row across renders. The same callback handles pointer, keyboard and AT.
pub fn launcher_row(
    name: impl Into<SharedString>,
    focus: &FocusHandle,
    selected: bool,
    enabled: bool,
    theme: Theme,
    activate: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let control = Role::Button
        .control(focus, name)
        .flex_none()
        .w_full()
        .px(tokens::space::SM)
        .py(tokens::space::XS)
        .border_1()
        .border_color(transparent_black())
        .rounded(tokens::radius::CONTROL)
        .text_color(rgb(if enabled {
            theme.text
        } else {
            theme.disabled()
        }))
        .when(selected, |row| {
            row.bg(rgb(theme.selected())).aria_value("Selected")
        });
    if !enabled {
        return unavailable(control, true);
    }
    let activate = Rc::new(activate);
    let accessible = activate.clone();
    control
        .track_focus(focus)
        .focus(move |s| s.border_color(rgb(theme.focus)))
        .cursor_pointer()
        .hover(move |s| {
            s.bg(if selected {
                rgb(theme.selected())
            } else {
                rgba((theme.focus << 8) | 0x32)
            })
        })
        .active(move |s| s.bg(rgb(theme.text)).text_color(rgb(theme.surface)))
        .on_click(move |_, w, cx| activate(w, cx))
        .on_a11y_action(accesskit::Action::Click, move |_, w, cx| accessible(w, cx))
}
