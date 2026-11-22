//! Shared GPUI controls for mundaneDE applications.
use gpui::{prelude::*, *};

mod header;
pub use header::WindowHeader;
mod text_field;
pub use text_field::{TextColor, TextField, bind_text_keys};

pub use gpui::Role;

pub trait ControlRole {
    fn control(self, id: impl Into<ElementId>, name: impl Into<SharedString>) -> Stateful<Div>;
}

impl ControlRole for Role {
    fn control(self, id: impl Into<ElementId>, name: impl Into<SharedString>) -> Stateful<Div> {
        div()
            .id(id)
            .role(self)
            .aria_label(name)
            .when(self == Role::Dialog, |el| {
                el.a11y_synthetic_children(|tree| tree.parent_node().set_modal())
            })
    }
}

pub fn option(
    id: ElementId,
    name: impl Into<SharedString>,
    index: usize,
    selected: bool,
    focused: bool,
) -> Stateful<Div> {
    Role::ListBoxOption
        .control(id, name)
        .aria_selected(selected)
        .aria_position_in_set(index)
        .when(selected && focused, |el| el.aria_active_descendant())
}

mod appearance;
pub mod tokens;
pub use appearance::{Theme, run};

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
        .px(tokens::space::MD)
        .py(tokens::space::XS)
        .child(label)
}

pub fn button_base(
    label: impl Into<SharedString>,
    focus: &FocusHandle,
    theme: Theme,
    activate: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    focus_ring(Role::Button.control(focus, label), focus, theme)
        .rounded(tokens::radius::CONTROL)
        .cursor_pointer()
        .text_size(tokens::text::CONTROL)
        .text_color(rgb(theme.text))
        .hover(move |style| style.bg(rgb(theme.selection)))
        .on_click(move |_, window, cx| activate(window, cx))
}

/// The AT-SPI bridge reads a static label's value from its text runs.
pub fn text_label(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    let text = text.into();
    div()
        .id(id)
        .role(Role::Label)
        .a11y_synthetic_children(move |tree| {
            let run_id = tree.synthetic_node_id("text");
            let mut run = accesskit::Node::new(Role::TextRun);
            if let Some(bounds) = tree.parent_node().bounds() {
                run.set_bounds(bounds);
            }
            run.set_value(text.as_ref());
            run.set_character_lengths(text.chars().map(|c| c.len_utf8() as u8).collect::<Vec<_>>());
            tree.push_child(run_id, run);
        })
}

/// Orca browses the heading by level and reads its static label via AT-SPI Text.
pub fn heading(id: &'static str, text: &'static str) -> Stateful<Div> {
    Role::Heading
        .control(id, text)
        .aria_level(1)
        .child(text_label("label", text).child(text))
}

/// Keep the modal's covered controls painted while excluding them from AT navigation.
#[derive(Default)]
pub struct ApplicationContent {
    covered: bool,
    generation: u64,
}

impl ApplicationContent {
    pub fn render(&mut self, covered: bool) -> Stateful<Div> {
        // AT-SPI clients retain defunct references. On remapping the content after
        // a modal, give the new subtree a fresh lifetime, including its descendants.
        if self.covered && !covered {
            self.generation += 1;
        }
        self.covered = covered;
        div()
            .id(ElementId::NamedInteger(
                "application-content".into(),
                self.generation,
            ))
            .when(covered, |el| {
                el.role(Role::Group)
                    .a11y_synthetic_children(|tree| tree.parent_node().set_hidden())
            })
    }
}

pub fn unavailable(control: Stateful<Div>, disabled: bool) -> Stateful<Div> {
    control.when(disabled, |el| {
        el.a11y_synthetic_children(|tree| {
            tree.parent_node().set_disabled();
            tree.parent_node().remove_action(accesskit::Action::Click);
        })
    })
}

#[cfg(test)]
mod tests {
    use super::option;
    use gpui::{Element, ElementId};
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt, path::Path, sync::Arc};

    #[test]
    fn option_identity_survives_reorder_and_lossy_display_names() {
        let path = |bytes: &[u8]| ElementId::Path(Arc::from(Path::new(OsStr::from_bytes(bytes))));
        let first = path(b"/dir/\xff");
        let second = path(b"/dir/\xfe");
        let before = Element::id(&option(first.clone(), "�", 0, false, false));
        let after = Element::id(&option(first, "renamed label", 1, true, true));
        let other = Element::id(&option(second, "�", 0, false, false));
        assert_eq!(before, after);
        assert_ne!(before, other);
    }
}

mod playback;
pub use playback::{PlaybackControls, TransportAction, TransportState, playback_time};

mod browser;
pub use browser::{BrowserList, BrowserRow, RowKey, browser_key, browser_window, byte_size};

/// External attribution and licence links share the suite's keyboard focus styling.
pub fn external_link(
    label: &'static str,
    url: &'static str,
    focus: &FocusHandle,
    theme: Theme,
) -> Stateful<Div> {
    focus_ring(Role::Link.control(focus, label), focus, theme)
        .px_1()
        .py_1()
        .text_sm()
        .underline()
        .cursor_pointer()
        .child(label)
        .on_click(move |_, _, cx| cx.open_url(url))
}

mod tabs;
pub use tabs::tab_strip;

mod panel;
pub use panel::{PanelState, badge, panel_button, toggle, tooltip, with_tooltip};
