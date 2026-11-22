use gpui::{prelude::*, *};
use mundane_services::usage::{self, Scan};
use mundane_ui::{
    BrowserList, BrowserRow, ControlRole, Role, Theme, WindowHeader, browser_key, browser_window,
    button, byte_size, unavailable,
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

fn items(n: usize) -> String {
    format!("{n} {}", if n == 1 { "item" } else { "items" })
}

struct Usage {
    scan: Option<Scan>,
    current: usize,
    order: Vec<usize>,
    list: BrowserList,
    active: Option<Arc<AtomicBool>>,
    choosing: bool,
    apparent: bool,
    notice: String,
    header: WindowHeader,
    buttons: [FocusHandle; 4],
    theme: Theme,
}
impl Drop for Usage {
    fn drop(&mut self) {
        if let Some(flag) = &self.active {
            flag.store(true, Ordering::Relaxed)
        }
    }
}
impl Usage {
    fn new(w: &mut Window, cx: &mut Context<Self>) -> Self {
        let list = BrowserList::new(cx);
        list.focus.focus(w, cx);
        Self {
            scan: None,
            current: 0,
            order: vec![],
            list,
            active: None,
            choosing: false,
            apparent: false,
            notice: "Choose a folder to scan.".into(),
            header: WindowHeader::new(cx),
            buttons: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            theme: Theme::current(),
        }
    }
    fn start(&mut self, path: PathBuf, w: &mut Window, cx: &mut Context<Self>) {
        self.cancel(cx);
        let flag = Arc::new(AtomicBool::new(false));
        self.active = Some(flag.clone());
        self.notice = format!("Scanning {}…", path.display());
        cx.notify();
        let cancel = flag.clone();
        let task = cx
            .background_executor()
            .spawn(async move { usage::scan(&path, &cancel) });
        cx.spawn_in(w, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |this, w, cx| {
                if !this.active.as_ref().is_some_and(|f| Arc::ptr_eq(f, &flag)) {
                    return;
                }
                this.active = None;
                match result {
                    Ok(scan) => {
                        w.set_window_title(&format!(
                            "{} — Disk Usage",
                            scan.root
                                .file_name()
                                .unwrap_or(scan.root.as_os_str())
                                .to_string_lossy()
                        ));
                        this.scan = Some(scan);
                        this.current = 0;
                        this.notice.clear();
                        this.refresh();
                        this.list.focus.focus(w, cx)
                    }
                    Err(e) => this.notice = format!("Could not scan folder. {e}"),
                }
                cx.notify()
            });
        })
        .detach()
    }
    fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(flag) = self.active.take() {
            flag.store(true, Ordering::Relaxed);
            self.notice = "Scan cancelled.".into();
            cx.notify()
        }
    }
    fn refresh(&mut self) {
        let Some(scan) = &self.scan else { return };
        let parent = &scan.nodes[self.current];
        let total = parent.bytes.get(self.apparent);
        self.order = parent.children.clone();
        self.order.sort_unstable_by(|a, b| {
            let a = &scan.nodes[*a];
            let b = &scan.nodes[*b];
            b.bytes
                .get(self.apparent)
                .cmp(&a.bytes.get(self.apparent))
                .then(a.name.cmp(&b.name))
        });
        let rows = self
            .order
            .iter()
            .map(|&i| {
                let n = &scan.nodes[i];
                let amount = n.bytes.get(self.apparent);
                let detail = n.note.clone().unwrap_or_else(|| {
                    if n.directory {
                        format!(
                            "Folder · {}{}",
                            items(n.children.len()),
                            if n.partial { " · incomplete" } else { "" }
                        )
                    } else {
                        "File".into()
                    }
                });
                BrowserRow {
                    key: scan.path(i),
                    label: n.name.to_string_lossy().into_owned(),
                    detail,
                    value: format!("{}{}", if n.partial { "≥ " } else { "" }, byte_size(amount)),
                    fraction: Some(if total == 0 {
                        0.
                    } else {
                        amount as f32 / total as f32
                    }),
                    depth: 0,
                }
            })
            .collect();
        self.list.replace(rows)
    }
    fn enter(&mut self, cx: &mut Context<Self>) {
        let Some(i) = self.list.selected.map(|n| self.order[n]) else {
            return;
        };
        let n = &self.scan.as_ref().unwrap().nodes[i];
        if n.directory && (n.note.is_none() || !n.children.is_empty()) {
            self.current = i;
            self.notice.clear();
            self.refresh();
            self.list.select(0);
            cx.notify()
        }
    }
    fn back(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self
            .scan
            .as_ref()
            .and_then(|s| s.nodes[self.current].parent)
        else {
            return;
        };
        let old = self.current;
        self.current = p;
        self.notice.clear();
        self.refresh();
        if let Some(i) = self.order.iter().position(|&n| n == old) {
            self.list.select(i)
        }
        cx.notify()
    }
    fn choose(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.choosing {
            return;
        }
        self.choosing = true;
        let choice = cx.prompt_for_paths(PathPromptOptions {
            directories: true,
            files: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(w, async move |v, cx| {
            let result = choice.await;
            let _ = v.update_in(cx, |this, w, cx| {
                this.choosing = false;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            this.start(path, w, cx)
                        }
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(e)) => this.notice = format!("Chooser unavailable. {e}"),
                    Err(e) => this.notice = e.to_string(),
                }
                cx.notify()
            });
        })
        .detach()
    }
    fn action(&mut self, i: usize, w: &mut Window, cx: &mut Context<Self>) {
        match i {
            0 => self.choose(w, cx),
            1 if self.active.is_some() => self.cancel(cx),
            1 => {
                if let Some(s) = &self.scan {
                    self.start(s.root.clone(), w, cx)
                }
            }
            2 => self.back(cx),
            3 => {
                self.apparent = !self.apparent;
                self.refresh();
                cx.notify()
            }
            _ => {}
        }
    }
    fn control(
        &self,
        i: usize,
        label: &'static str,
        enabled: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let v = cx.weak_entity();
        unavailable(
            button(label, &self.buttons[i], self.theme, move |w, cx| {
                if enabled {
                    let _ = v.update(cx, |this, cx| this.action(i, w, cx));
                }
            }),
            !enabled,
        )
    }
    fn keys(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        match (k.key.as_str(), k.modifiers.control) {
            ("o", true) => self.choose(w, cx),
            ("f5", false) if self.active.is_none() => self.action(1, w, cx),
            ("escape", false) if self.active.is_some() => self.cancel(cx),
            ("escape", false) => self.list.focus.focus(w, cx),
            ("backspace", false) => self.back(cx),
            ("enter", false) if self.list.focus.is_focused(w) => self.enter(cx),
            ("a", false) => self.action(3, w, cx),
            ("f6", false) => self.buttons[0].focus(w, cx),
            _ if self.list.focus.is_focused(w) && self.list.navigate(&k.key) => cx.notify(),
            _ if browser_key(k, w, cx) => {}
            _ => return,
        }
        w.prevent_default();
        cx.stop_propagation()
    }
}
impl Render for Usage {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let scan = self.scan.as_ref();
        let root = scan.map(|s| &s.nodes[self.current]);
        let name = scan.map_or_else(
            || "Folder contents".into(),
            |s| {
                let path = s.path(self.current);
                let base = s.root.file_name().unwrap_or(s.root.as_os_str());
                let relative = path.strip_prefix(&s.root).unwrap();
                if relative.as_os_str().is_empty() {
                    base.to_string_lossy().into_owned()
                } else {
                    PathBuf::from(base).join(relative).display().to_string()
                }
            },
        );
        let mode = if self.apparent {
            "File sizes"
        } else {
            "Allocated space"
        };
        let status = if !self.notice.is_empty() {
            self.notice.clone()
        } else if let Some(n) = root {
            format!(
                "{mode} · {}{} · {}{}",
                if n.partial { "At least " } else { "" },
                byte_size(n.bytes.get(self.apparent)),
                items(n.children.len()),
                if n.partial { " · Incomplete scan" } else { "" }
            )
        } else {
            String::new()
        };
        let nav = div()
            .flex()
            .gap_2()
            .child(self.control(0, "Open folder…", true, cx))
            .child(self.control(
                1,
                if self.active.is_some() {
                    "Cancel scan"
                } else {
                    "Rescan"
                },
                self.active.is_some() || scan.is_some(),
                cx,
            ));
        let v = cx.weak_entity();
        let list = self
            .list
            .render("Folder contents", theme, w, move |i, double, w, cx| {
                let _ = v.update(cx, |this, cx| {
                    this.list.select(i);
                    this.list.focus.focus(w, cx);
                    if double {
                        this.enter(cx)
                    }
                    cx.notify()
                });
            });
        div()
            .size_full()
            .flex()
            .flex_col()
            .font_family("DejaVu Sans")
            .bg(rgb(theme.surface))
            .text_color(rgb(theme.text))
            .on_key_down(cx.listener(Self::keys))
            .child(
                self.header
                    .render("Disk Usage", nav, &self.list.focus, theme, w),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .flex()
                    .items_center()
                    .bg(rgb(theme.panel))
                    .child(self.control(2, "Back", self.current != 0, cx))
                    .child(
                        mundane_ui::text_label("folder", name.clone())
                            .flex_1()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(name),
                    )
                    .child(self.control(
                        3,
                        if self.apparent {
                            "Show allocated space"
                        } else {
                            "Show file sizes"
                        },
                        scan.is_some(),
                        cx,
                    )),
            )
            .child(list)
            .child(
                Role::Status
                    .control("usage-status", status.clone())
                    .px_3()
                    .py_2()
                    .bg(rgb(theme.panel))
                    .child(status),
            )
    }
}
fn main() {
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    browser_window(
        "Disk Usage",
        "org.joelinux.mundane.DiskUsage",
        move |w, cx| {
            let mut app = Usage::new(w, cx);
            if let Some(path) = path {
                app.start(path, w, cx)
            }
            app
        },
    )
}
#[cfg(test)]
mod tests {
    use super::Usage;
    use gpui::TestAppContext;
    use std::{fs, path::Path};
    #[gpui::test]
    fn drill_back_and_cancel_rescan(cx: &mut TestAppContext) {
        let d = tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"))
            .unwrap();
        fs::create_dir(d.path().join("Charts")).unwrap();
        fs::write(d.path().join("Charts/data"), [2; 8192]).unwrap();
        let (v, ui) = cx.add_window_view(|w, cx| {
            let mut a = Usage::new(w, cx);
            a.start(d.path().into(), w, cx);
            a
        });
        ui.run_until_parked();
        ui.simulate_keystrokes("enter");
        assert!(v.read_with(ui, |a, _| a.list.rows[0].label == "data"));
        ui.simulate_keystrokes("backspace");
        assert!(v.read_with(ui, |a, _| a.list.rows[0].label == "Charts"));
        ui.update(|w, cx| {
            v.update(cx, |a, cx| {
                a.action(1, w, cx);
                assert!(a.active.is_some());
                a.cancel(cx)
            })
        });
        ui.run_until_parked();
        assert!(v.read_with(ui, |a, _| a.active.is_none()
            && a.notice == "Scan cancelled."
            && a.scan.is_some()));
    }
}
