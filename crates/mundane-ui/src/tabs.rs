//! A horizontally scrollable strip for open documents and PTYs.
use super::*;
use std::rc::Rc;

pub fn tab_strip(
    tabs: impl IntoIterator<Item = (SharedString, FocusHandle)>,
    active: usize,
    scroll: &ScrollHandle,
    select: impl Fn(usize, &mut Window, &mut App) + 'static,
    close: Stateful<Div>,
    theme: Theme,
) -> Div {
    let select = Rc::new(select);
    let strip = Role::TabList
        .control("open-tabs", "Open tabs")
        .flex()
        .flex_1()
        .min_w_0()
        .overflow_x_scroll()
        .track_scroll(scroll)
        .children(tabs.into_iter().enumerate().map(|(i, (label, focus))| {
            let select = select.clone();
            focus_ring(
                Role::Tab
                    .control(&focus, label.clone())
                    .aria_selected(i == active),
                &focus,
                theme,
            )
            .max_w(rems(15.))
            .min_w(rems(5.))
            .flex_none()
            .truncate()
            .text_sm()
            .px_2()
            .py_1()
            .when(i == active, |el| el.bg(rgb(theme.selection)))
            .cursor_pointer()
            .child(label)
            .on_click(move |_, w, cx| select(i, w, cx))
        }));
    div()
        .w_full()
        .flex_none()
        .flex()
        .items_center()
        .gap_1()
        .bg(rgb(theme.panel))
        .child(strip)
        .child(close)
}
