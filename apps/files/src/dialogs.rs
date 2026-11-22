use super::{Files, Job, Paste, PasteAction};
use gpui::{prelude::*, *};
use mundane_ui::ControlRole as _;
use mundane_ui::Theme;
use mundane_ui::{TextField, button};
use std::path::{Path, PathBuf};

pub(super) enum Prompt {
    Search,
    Location,
    Rename(PathBuf),
    Trash(Vec<PathBuf>),
    Conflict(Paste, mundane_services::files::Conflict),
}
pub(super) struct Dialog {
    pub(super) prompt: Prompt,
    field: Option<Entity<TextField>>,
    pub(super) cancel: FocusHandle,
    accept: FocusHandle,
    replace: FocusHandle,
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
            Prompt::Search => Some((
                "File name contains",
                self.nav.query.clone().unwrap_or_default(),
            )),
            Prompt::Location => Some(("Folder path", self.path.to_string_lossy().into_owned())),
            Prompt::Rename(path) => Some(("File name", name(path))),
            _ => None,
        };
        let field = value.map(|(label, value)| {
            cx.new(|cx| TextField::new(label, Theme::current(cx), cx).select_value(value))
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
            replace: cx.focus_handle(),
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
            if matches!(dialog.prompt, Prompt::Conflict(..)) {
                self.queue.clear();
                self.load(self.path.clone(), None, cx);
            }
            return;
        }
        let text = dialog
            .field
            .as_ref()
            .map(|f| f.read(cx).text())
            .unwrap_or("");
        match dialog.prompt {
            Prompt::Search => {
                let query = text.to_owned();
                self.list_focus.focus(window, cx);
                self.search(query, cx);
            }
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
            Prompt::Trash(paths) => {
                self.queue = paths.into_iter().map(Job::Trash).collect();
                if let Some(job) = self.queue.pop_front() {
                    self.execute(job, window, cx);
                }
            }
            Prompt::Conflict(paste, _) => {
                self.execute(Job::Paste(paste, PasteAction::KeepBoth), window, cx)
            }
        }
    }

    pub(super) fn dialog_view(&self, dialog: &Dialog, cx: &Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        let (title, detail, action) = match &dialog.prompt {
            Prompt::Search => ("Search this folder", None, "Search"),
            Prompt::Location => ("Open folder", None, "Open"),
            Prompt::Rename(_) => ("Rename item", None, "Rename"),
            Prompt::Trash(paths) => (
                "Move to Trash?",
                Some(if paths.len() == 1 {
                    name(&paths[0])
                } else {
                    format!("{} selected items", paths.len())
                }),
                "Move to Trash",
            ),
            Prompt::Conflict(paste, _) => (
                "Name already in use",
                Some(format!("“{}” already exists here.", name(&paste.dest))),
                "Keep both",
            ),
        };
        let review = if let Prompt::Conflict(_, info) = &dialog.prompt {
            Some(info)
        } else {
            None
        };
        let can_replace = review.is_some_and(|r| r.reason.is_none());
        let comparison = review.map(|r| {
            let mut rows = div().flex().flex_col().gap_2();
            for (id, label, info) in [
                ("incoming", "Incoming", &r.incoming),
                ("existing", "Existing", &r.existing),
            ] {
                let summary = info.description();
                let text = format!("{label} · {summary}");
                rows = rows.child(
                    mundane_ui::text_label(id, text)
                        .flex()
                        .flex_col()
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(label))
                        .child(div().text_sm().child(summary)),
                );
            }
            rows.children(r.reason)
        });
        let mut stops: Vec<_> = dialog
            .field
            .iter()
            .map(|f| f.read(cx).focus_handle(cx))
            .collect();
        stops.extend([dialog.cancel.clone(), dialog.accept.clone()]);
        if can_replace {
            stops.push(dialog.replace.clone())
        }
        let field = dialog.field.clone();
        let cancel = cx.weak_entity();
        let accept = cancel.clone();
        let replace_view = cancel.clone();
        mundane_ui::Role::Dialog
            .control("file-dialog", title)
            .aria_description(match review {
                Some(r) => format!(
                    "{} Incoming: {}. Existing: {}. {}",
                    detail.as_deref().unwrap_or(title),
                    r.incoming.description(),
                    r.existing.description(),
                    r.reason.unwrap_or("")
                ),
                None => detail.clone().unwrap_or_else(|| title.into()),
            })
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
                    .when(review.is_some(), |el| el.w(rems(27.)))
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
                    .children(comparison)
                    .children(dialog.field.clone())
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
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
                            }))
                            .when(review.is_some(), |el| {
                                el.child(mundane_ui::unavailable(
                                    button("Replace", &dialog.replace, theme, move |window, cx| {
                                        if can_replace {
                                            let _ = replace_view.update(cx, |this, cx| {
                                                if let Some(Dialog {
                                                    prompt: Prompt::Conflict(paste, info),
                                                    return_focus,
                                                    ..
                                                }) = this.dialog.take()
                                                {
                                                    return_focus.focus(window, cx);
                                                    this.execute(
                                                        Job::Paste(
                                                            paste,
                                                            PasteAction::Replace(info),
                                                        ),
                                                        window,
                                                        cx,
                                                    );
                                                }
                                            });
                                        }
                                    }),
                                    !can_replace,
                                ))
                            }),
                    ),
            )
    }
}
