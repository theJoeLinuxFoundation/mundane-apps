use super::{Files, Job, Paste};
use gpui::{prelude::*, *};
use mundane_ui::{TextField, button};
use std::path::{Path, PathBuf};

pub(super) enum Prompt {
    Location,
    Rename(PathBuf),
    Trash(PathBuf),
    Conflict(Paste),
}
pub(super) struct Dialog {
    pub(super) prompt: Prompt,
    field: Option<Entity<TextField>>,
    pub(super) cancel: FocusHandle,
    accept: FocusHandle,
    return_focus: FocusHandle,
}

fn name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

impl Files {
    pub(super) fn prompt(&mut self, prompt: Prompt, window: &mut Window, cx: &mut Context<Self>) {
        let value = match &prompt {
            Prompt::Location => Some(("Folder path", self.path.to_string_lossy().into_owned())),
            Prompt::Rename(path) => Some(("File name", name(path))),
            _ => None,
        };
        let field = value.map(|(label, value)| {
            cx.new(|cx| TextField::new(label, self.theme, cx).select_value(value))
        });
        let cancel = cx.focus_handle();
        let accept = cx.focus_handle();
        let return_focus = window
            .focused(cx)
            .unwrap_or_else(|| self.list_focus.clone());
        field
            .as_ref()
            .map(|f| f.read(cx).focus_handle(cx))
            .unwrap_or_else(|| cancel.clone())
            .focus(window, cx);
        self.dialog = Some(Dialog {
            prompt,
            field,
            cancel,
            accept,
            return_focus,
        });
        cx.notify();
    }

    fn finish_dialog(&mut self, yes: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        dialog.return_focus.focus(window, cx);
        cx.notify();
        if !yes {
            return;
        }
        let text = dialog
            .field
            .as_ref()
            .map(|f| f.read(cx).text())
            .unwrap_or("");
        match dialog.prompt {
            Prompt::Location => {
                let home = std::env::var_os("HOME").map(PathBuf::from);
                let dest = match (text, home) {
                    ("~", Some(home)) => home,
                    (s, Some(home)) if s.starts_with("~/") => home.join(&s[2..]),
                    (s, _) => self.path.join(s),
                };
                self.load(dest, None, cx);
            }
            Prompt::Rename(path) => {
                if text != name(&path) {
                    self.execute(Job::Rename(path, text.into()), window, cx);
                }
            }
            Prompt::Trash(path) => self.execute(Job::Trash(path), window, cx),
            Prompt::Conflict(paste) => self.execute(Job::Paste(paste, true), window, cx),
        }
    }

    pub(super) fn dialog_view(&self, dialog: &Dialog, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let (title, detail, action) = match &dialog.prompt {
            Prompt::Location => ("Open folder", None, "Open"),
            Prompt::Rename(_) => ("Rename item", None, "Rename"),
            Prompt::Trash(path) => ("Move to Trash?", Some(name(path)), "Move to Trash"),
            Prompt::Conflict(paste) => (
                "Name already in use",
                Some(format!("“{}” already exists here.", name(&paste.dest))),
                "Keep both",
            ),
        };
        let mut stops: Vec<_> = dialog
            .field
            .iter()
            .map(|f| f.read(cx).focus_handle(cx))
            .collect();
        stops.extend([dialog.cancel.clone(), dialog.accept.clone()]);
        let field = dialog.field.clone();
        let cancel = cx.weak_entity();
        let accept = cancel.clone();
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x00000070))
            .flex()
            .items_center()
            .justify_center()
            .on_key_down(cx.listener(move |this, key: &KeyDownEvent, window, cx| {
                match key.keystroke.key.as_str() {
                    "tab" => {
                        let i = stops.iter().position(|f| f.is_focused(window)).unwrap_or(0);
                        let step = if key.keystroke.modifiers.shift {
                            stops.len() - 1
                        } else {
                            1
                        };
                        stops[(i + step) % stops.len()].focus(window, cx);
                    }
                    "escape" => this.finish_dialog(false, window, cx),
                    "enter"
                        if field
                            .as_ref()
                            .is_some_and(|f| f.read(cx).focus_handle(cx).is_focused(window)) =>
                    {
                        this.finish_dialog(true, window, cx)
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(
                div()
                    .w(px(320.))
                    .max_w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .rounded_lg()
                    .bg(rgb(theme.panel))
                    .border_1()
                    .border_color(rgb(theme.border))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                    .children(detail)
                    .children(dialog.field.clone())
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(button(
                                "Cancel",
                                &dialog.cancel,
                                theme,
                                move |window, cx| {
                                    let _ = cancel.update(cx, |this, cx| {
                                        this.finish_dialog(false, window, cx)
                                    });
                                },
                            ))
                            .child(button(action, &dialog.accept, theme, move |window, cx| {
                                let _ = accept
                                    .update(cx, |this, cx| this.finish_dialog(true, window, cx));
                            })),
                    ),
            )
    }
}
