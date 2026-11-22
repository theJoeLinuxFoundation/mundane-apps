//! Shared GPUI controls for mundaneDE applications.
use gpui::{prelude::*, *};

mod text_field;
pub use text_field::{TextField, bind_text_keys};

#[derive(Clone, Copy)]
pub struct Theme {
    pub surface: u32,
    pub panel: u32,
    pub text: u32,
    pub secondary: u32,
    pub border: u32,
    pub focus: u32,
    pub selection: u32,
}

impl Theme {
    pub const DARK: Self = Self {
        surface: 0x17201e,
        panel: 0x222e2a,
        text: 0xf0f3ed,
        secondary: 0xb4c3b9,
        border: 0x52675c,
        focus: 0xa0dfb3,
        selection: 0x406d58,
    };
    pub const HIGH_CONTRAST: Self = Self {
        surface: 0,
        panel: 0,
        text: 0xffffff,
        secondary: 0xffffff,
        border: 0xffffff,
        focus: 0xffff00,
        selection: 0x224488,
    };
}

pub fn focus_ring(control: Stateful<Div>, focus: &FocusHandle, theme: Theme) -> Stateful<Div> {
    control
        .track_focus(focus)
        .border_1()
        .border_color(rgb(theme.border))
        .focus(move |style| style.border_color(rgb(theme.focus)))
}

pub fn header(title: impl Into<SharedString>, theme: Theme) -> Div {
    div()
        .w_full()
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .py_1()
        .bg(rgb(theme.panel))
        .border_b_1()
        .border_color(rgb(theme.border))
        .text_color(rgb(theme.text))
        .child(div().font_weight(FontWeight::SEMIBOLD).child(title.into()))
}

pub fn button(
    label: &'static str,
    focus: &FocusHandle,
    theme: Theme,
    activate: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let activate = std::rc::Rc::new(activate);
    let keyboard = activate.clone();
    focus_ring(div().id(label), focus, theme)
        .rounded_md()
        .px_3()
        .py_1()
        .cursor_pointer()
        .text_sm()
        .text_color(rgb(theme.text))
        .hover(move |style| style.bg(rgb(theme.selection)))
        .on_click(move |_, window, cx| activate(window, cx))
        .on_key_down(move |event, window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                keyboard(window, cx);
                cx.stop_propagation();
            }
        })
        .child(label)
}
