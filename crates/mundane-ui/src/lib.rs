//! Shared GPUI controls for mundaneDE applications.
use gpui::{prelude::*, *};

mod header;
pub use header::WindowHeader;
mod text_field;
pub use text_field::{TextField, bind_text_keys};

/// Local control declarations. The GPUI accessibility adapter can consume the role
/// and name here when its Linux API is available.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    TitleBar,
    Button,
    TextInput,
    MultilineTextInput,
    Dialog,
    List,
    ListItem,
    Terminal,
    Status,
}

impl Role {
    fn id(self, name: impl Into<SharedString>) -> ElementId {
        let role = match self {
            Self::TitleBar => "title bar",
            Self::Button => "button",
            Self::TextInput => "text input",
            Self::MultilineTextInput => "multiline text input",
            Self::Dialog => "dialog",
            Self::List => "list",
            Self::ListItem => "list item",
            Self::Terminal => "terminal",
            Self::Status => "status",
        };
        ElementId::NamedChild(std::sync::Arc::new(role.into()), name.into())
    }

    pub fn control(self, name: impl Into<SharedString>) -> Stateful<Div> {
        div().id(self.id(name))
    }

    /// Distinguish rows whose displayed names coincide, including lossy Unix filenames.
    pub fn row(self, name: impl Into<SharedString>, index: usize) -> Stateful<Div> {
        div().id(ElementId::NamedChild(
            std::sync::Arc::new(self.id(name)),
            index.to_string().into(),
        ))
    }
}

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
    pub fn current() -> Self {
        if std::env::var_os("MUNDANE_HIGH_CONTRAST").as_deref() == Some("1".as_ref()) {
            Self::HIGH_CONTRAST
        } else {
            Self::DARK
        }
    }

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

pub fn button(
    label: &'static str,
    focus: &FocusHandle,
    theme: Theme,
    activate: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    button_base(label, focus, theme, activate)
        .px_3()
        .py_1()
        .child(label)
}

fn button_base(
    label: &'static str,
    focus: &FocusHandle,
    theme: Theme,
    activate: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    focus_ring(Role::Button.control(label), focus, theme)
        .rounded_md()
        .cursor_pointer()
        .text_sm()
        .text_color(rgb(theme.text))
        .hover(move |style| style.bg(rgb(theme.selection)))
        .on_click(move |_, window, cx| activate(window, cx))
}
