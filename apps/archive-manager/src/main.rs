mod edit;
use gpui::{prelude::*, *};
use mundane_services::archive::{self, Entry, Kind};
use mundane_ui::{
    BrowserList, BrowserRow, ControlRole, Role, Theme, WindowHeader, browser_key, browser_window,
    button, byte_size, unavailable,
};
use std::{collections::BTreeSet, path::PathBuf};

struct Manager {
    archive: Option<PathBuf>,
    stamp: Option<archive::Stamp>,
    confirm: Option<Entity<mundane_ui::ConfirmDialog>>,
    entries: Vec<Entry>,
    marked: BTreeSet<PathBuf>,
    list: BrowserList,
    header: WindowHeader,
    buttons: [FocusHandle; 7],
    status: String,
    busy: bool,
}
impl Manager {
    fn new(w: &mut Window, cx: &mut Context<Self>) -> Self {
        let list = BrowserList::new(cx);
        list.focus.focus(w, cx);
        Self {
            archive: None,
            stamp: None,
            confirm: None,
            entries: vec![],
            marked: Default::default(),
            list,
            header: WindowHeader::new(cx),
            buttons: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            status: "Open a ZIP or tar archive.".into(),
            busy: false,
        }
    }
    fn open(&mut self, path: PathBuf, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.status = "Reading archive…".into();
        cx.notify();
        let source = path.clone();
        let task = cx
            .background_executor()
            .spawn(async move { archive::inspect(&source) });
        cx.spawn_in(w, async move |v, cx| {
            let result = task.await;
            let _ = v.update_in(cx, |this, w, cx| {
                this.busy = false;
                match result {
                    Ok((entries, stamp)) => {
                        this.stamp = Some(stamp);
                        w.set_window_title(&format!(
                            "{} — Archive Manager",
                            path.file_name()
                                .unwrap_or(path.as_os_str())
                                .to_string_lossy()
                        ));
                        this.archive = Some(path);
                        this.entries = entries;
                        this.marked.clear();
                        this.refresh();
                        this.list.select(0);
                        this.list.focus.focus(w, cx);
                        this.status = match this.entries.len() {
                            0 => "This archive is empty".into(),
                            1 => "1 entry".into(),
                            n => format!("{n} entries"),
                        };
                    }
                    Err(e) => this.status = format!("Could not open archive. {e:#}"),
                }
                cx.notify()
            });
        })
        .detach();
    }
    fn refresh(&mut self) {
        self.list.replace(
            self.entries
                .iter()
                .map(|e| {
                    let marked = self.marked.contains(&e.path);
                    let detail = match &e.kind {
                        Kind::Directory => "Folder".into(),
                        Kind::File => "File".into(),
                        Kind::HardLink(p) => format!("Hard link to {}", p.display()),
                        Kind::SymLink(p) => format!("Link to {}", p.display()),
                    };
                    BrowserRow {
                        key: e.path.clone(),
                        label: e.path.file_name().unwrap().to_string_lossy().into_owned(),
                        detail: if marked {
                            format!("Marked · {detail}")
                        } else {
                            detail
                        },
                        value: if e.kind == Kind::File {
                            byte_size(e.size)
                        } else {
                            String::new()
                        },
                        depth: e.path.components().count() - 1,
                        fraction: None,
                    }
                })
                .collect(),
        )
    }
    fn mark(&mut self, cx: &mut Context<Self>) {
        if let Some(i) = self.list.selected {
            let path = &self.entries[i].path;
            if !self.marked.remove(path) {
                self.marked.insert(path.clone());
            }
            self.refresh();
            self.status = format!("{} marked", self.marked.len());
            cx.notify()
        }
    }
    fn choose(&mut self, extract: bool, all: bool, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy || (extract && self.entries.is_empty()) {
            return;
        }
        let mut selection = self.marked.clone();
        if all {
            selection.clear()
        } else if extract && selection.is_empty() {
            let Some(i) = self.list.selected else { return };
            selection.insert(self.entries[i].path.clone());
        }
        let archive = self.archive.clone();
        self.busy = true;
        cx.notify();
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: !extract,
            directories: extract,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(w, async move |v, cx| {
            let result = choice.await;
            let _ = v.update_in(cx, |this, w, cx| {
                this.busy = false;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            if extract {
                                if let Some(src) = archive {
                                    this.extract(src, path, selection, w, cx)
                                }
                            } else {
                                this.open(path, w, cx)
                            }
                        }
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(e)) => this.status = format!("Chooser unavailable. {e}"),
                    Err(e) => this.status = e.to_string(),
                }
                cx.notify()
            });
        })
        .detach()
    }
    fn extract(
        &mut self,
        src: PathBuf,
        dest: PathBuf,
        selection: BTreeSet<PathBuf>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busy = true;
        self.status = "Extracting…".into();
        cx.notify();
        let task = cx
            .background_executor()
            .spawn(async move { archive::extract(&src, &dest, &selection) });
        cx.spawn_in(w, async move |v, cx| {
            let result = task.await;
            let _ = v.update_in(cx, |this, w, cx| {
                this.busy = false;
                this.status = match result {
                    Ok(p) => format!("Extracted to {}", p.display()),
                    Err(e) => format!("Extraction failed. {e:#}"),
                };
                this.list.focus.focus(w, cx);
                cx.notify()
            });
        })
        .detach()
    }
    fn action(&mut self, i: usize, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.confirm.is_some() {
            return;
        }
        if i == 4 || i == 5 {
            self.pick_inputs(i == 4, w, cx);
            return;
        }
        if i == 6 {
            self.remove(w, cx);
            return;
        }
        if i == 1 {
            self.mark(cx)
        } else {
            self.choose(i != 0, i == 3, w, cx)
        }
    }
    fn keys(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        if self.confirm.is_some() {
            return;
        }
        let k = &e.keystroke;
        match (k.key.as_str(), k.modifiers.control) {
            ("n", true) => self.action(4, w, cx),
            ("i", true) => self.action(5, w, cx),
            ("delete", false) => self.action(6, w, cx),
            ("o", true) => self.action(0, w, cx),
            ("e", true) => self.action(if k.modifiers.shift { 3 } else { 2 }, w, cx),
            ("space", false) if self.list.focus.is_focused(w) => self.action(1, w, cx),
            ("f6", false) => self.buttons[0].focus(w, cx),
            ("escape", false) => self.list.focus.focus(w, cx),
            _ if self.list.focus.is_focused(w) && self.list.navigate(&k.key) => cx.notify(),
            _ if browser_key(k, w, cx) => {}
            _ => return,
        }
        w.prevent_default();
        cx.stop_propagation()
    }
}
impl Render for Manager {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        let controls = ["Open…", "Mark", "Extract selected…", "Extract all…"]
            .into_iter()
            .enumerate()
            .map(|(i, label)| {
                let v = cx.weak_entity();
                unavailable(
                    button(label, &self.buttons[i], theme, move |w, cx| {
                        let _ = v.update(cx, |this, cx| this.action(i, w, cx));
                    }),
                    self.busy || (i != 0 && self.entries.is_empty()),
                )
            })
            .collect::<Vec<_>>();
        let editing = ["New archive…", "Add files…", "Remove…"]
            .into_iter()
            .enumerate()
            .map(|(j, label)| {
                let i = j + 4;
                let v = cx.weak_entity();
                unavailable(
                    button(label, &self.buttons[i], theme, move |w, cx| {
                        let _ = v.update(cx, |s, cx| s.action(i, w, cx));
                    }),
                    self.busy
                        || (i != 4 && self.archive.is_none())
                        || (i == 6 && self.entries.is_empty()),
                )
            })
            .collect::<Vec<_>>();
        let v = cx.weak_entity();
        let list = self
            .list
            .render("Archive entries", theme, w, move |i, _, w, cx| {
                let _ = v.update(cx, |this, cx| {
                    this.list.select(i);
                    this.list.focus.focus(w, cx);
                    cx.notify()
                });
            });
        let name = self
            .archive
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Archive contents".into());
        div()
            .size_full()
            .flex()
            .flex_col()
            .font_family("DejaVu Sans")
            .bg(rgb(theme.surface))
            .text_color(rgb(theme.text))
            .on_key_down(cx.listener(Self::keys))
            .child(self.header.render(
                "Archive Manager",
                div().flex().gap_2().children(controls),
                &self.list.focus,
                theme,
                w,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        mundane_ui::text_label("archive-name", name.clone())
                            .flex_1()
                            .px_3()
                            .py_2()
                            .bg(rgb(theme.panel))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(name),
                    )
                    .children(editing),
            )
            .child(list)
            .child(
                Role::Status
                    .control("archive-status", self.status.clone())
                    .px_3()
                    .py_2()
                    .bg(rgb(theme.panel))
                    .child(self.status.clone()),
            )
            .children(self.confirm.clone())
    }
}
fn main() {
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    browser_window(
        "Archive Manager",
        "org.joelinux.mundane.ArchiveManager",
        move |w, cx| {
            let mut app = Manager::new(w, cx);
            if let Some(p) = path {
                app.open(p, w, cx)
            }
            app
        },
    )
}
#[cfg(test)]
mod tests {
    use super::Manager;
    use gpui::TestAppContext;
    use std::{
        fs,
        path::{Path, PathBuf},
    };
    #[gpui::test]
    fn extract_marked_files_with_portal(cx: &mut TestAppContext) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dest = tempfile::tempdir_in(root.join("target")).unwrap();
        let (v, ui) = cx.add_window_view(|w, cx| {
            let mut m = Manager::new(w, cx);
            m.open(
                root.join("tests/fixtures/archive/Coastal survey.zip"),
                w,
                cx,
            );
            m
        });
        ui.run_until_parked();
        ui.simulate_keystrokes("down space down space ctrl-e");
        ui.run_until_parked();
        ui.simulate_path_prompt_response(|options| {
            assert!(options.directories && !options.files);
            Some(vec![dest.path().to_owned()])
        });
        ui.run_until_parked();
        let out = dest.path().join("Coastal survey (extracted)");
        assert_eq!(
            fs::read(out.join("Charts/channel.txt")).unwrap(),
            b"East channel\n"
        );
        assert!(out.join("Charts/tides.csv").is_file());
        assert!(!out.join("Notes").exists());
        assert!(v.read_with(ui, |m, _| m.status.starts_with("Extracted to")
            && m.marked.contains(&PathBuf::from("Charts/channel.txt"))))
    }
}
