mod operations;
use gpui::{prelude::*, *};
use mundane_services::disks::{Item, UDisks, Volume};
use mundane_ui::{
    BrowserList, BrowserRow, ControlRole, Role, Theme, WindowHeader, browser_key, browser_window,
    button, byte_size, unavailable,
};
use operations::Prompt;
use std::time::Duration;

struct Disks {
    service: UDisks,
    prompt: Option<Prompt>,
    operations: [FocusHandle; 3],
    cancel: FocusHandle,
    unlock_button: FocusHandle,
    items: Vec<Item>,
    list: BrowserList,
    header: WindowHeader,
    refresh_button: FocusHandle,
    change_button: FocusHandle,
    notice: String,
    error: Option<String>,
    refreshing: bool,
    changing: bool,
    generation: u64,
}
impl Disks {
    fn new(service: UDisks, w: &mut Window, cx: &mut Context<Self>) -> Self {
        mundane_ui::bind_text_keys(cx);
        let list = BrowserList::new(cx);
        list.focus.focus(w, cx);
        let mut this = Self {
            service,
            prompt: None,
            operations: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            cancel: cx.focus_handle().tab_stop(true),
            unlock_button: cx.focus_handle().tab_stop(true),
            items: vec![],
            list,
            header: WindowHeader::new(cx),
            refresh_button: cx.focus_handle().tab_stop(true),
            change_button: cx.focus_handle().tab_stop(true),
            notice: String::new(),
            error: None,
            refreshing: false,
            changing: false,
            generation: 0,
        };
        this.refresh(w, cx);
        this
    }
    fn watch(&self, w: &mut Window, cx: &mut Context<Self>) {
        let timer = cx.background_executor().clone();
        cx.spawn_in(w, async move |v, cx| {
            loop {
                timer.timer(Duration::from_secs(2)).await;
                if v.update_in(cx, |this, w, cx| this.refresh(w, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }
    fn selected(&self) -> Option<&Volume> {
        self.list
            .selected
            .and_then(|i| self.items.get(i))
            .and_then(|n| n.volume.as_ref())
    }
    fn layout(&mut self, items: Vec<Item>, w: &mut Window, cx: &mut Context<Self>) {
        if self.items == items {
            return;
        }
        let selected = self.list.selected.and_then(|i| self.items.get(i));
        let removed = selected.is_some_and(|old| {
            !items.iter().any(|n| {
                n.path == old.path
                    && match (&n.volume, &old.volume) {
                        (Some(a), Some(b)) => a.same_device(b),
                        (None, None) => true,
                        _ => false,
                    }
            })
        });
        let keep_empty = !self.items.is_empty() && self.list.selected.is_none();
        self.items = items;
        self.list.replace(
            self.items
                .iter()
                .map(|n| BrowserRow {
                    key: n.path.clone().into(),
                    label: n.label.clone(),
                    detail: n.detail.clone(),
                    value: byte_size(n.size),
                    depth: n.depth,
                    fraction: None,
                })
                .collect(),
        );
        if removed || keep_empty {
            self.list.selected = None;
            if removed && !self.changing {
                self.notice = "The selected device is no longer available.".into()
            }
            self.list.focus.focus(w, cx)
        }
        w.set_window_title("Disks");
        cx.notify()
    }
    fn refresh(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        let generation = self.generation;
        let service = self.service.clone();
        let task = cx
            .background_executor()
            .spawn(async move { service.list() });
        cx.spawn_in(w, async move |v, cx| {
            let result = task.await;
            let _ = v.update_in(cx, |this, w, cx| {
                this.refreshing = false;
                if this.generation != generation {
                    this.refresh(w, cx);
                    return;
                }
                match result {
                    Ok(items) => {
                        let recovered = this.error.take().is_some();
                        this.layout(items, w, cx);
                        if recovered || this.items.is_empty() {
                            cx.notify()
                        }
                    }
                    Err(e) => {
                        let msg = format!("Could not read UDisks devices. {e:#}");
                        if this.error.as_ref() != Some(&msg) {
                            this.layout(vec![], w, cx);
                            this.error = Some(msg);
                            cx.notify()
                        }
                    }
                }
            });
        })
        .detach()
    }
    fn change(&mut self, target: Volume, w: &mut Window, cx: &mut Context<Self>) {
        if self.changing || self.selected() != Some(&target) {
            return;
        }
        self.run_change(move |s| s.change(&target), w, cx);
    }
    fn run_change(
        &mut self,
        operation: impl FnOnce(&UDisks) -> anyhow::Result<String> + Send + 'static,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.changing {
            return;
        }
        self.changing = true;
        self.generation += 1;
        self.error = None;
        self.notice = "Waiting for UDisks…".into();
        cx.notify();
        let service = self.service.clone();
        let task = cx.background_executor().spawn(async move {
            let reply = operation(&service);
            let snapshot = service.list();
            (reply, snapshot)
        });
        cx.spawn_in(w, async move |v, cx| {
            let (reply, snapshot) = task.await;
            let _ = v.update_in(cx, |this, w, cx| {
                this.generation += 1;
                this.notice = reply.unwrap_or_else(|e| format!("{e:#}"));
                match snapshot {
                    Ok(items) => {
                        this.error = None;
                        this.layout(items, w, cx)
                    }
                    Err(e) => {
                        this.layout(vec![], w, cx);
                        this.error =
                            Some(format!("{} Could not refresh devices. {e:#}", this.notice))
                    }
                }
                this.changing = false;
                cx.notify()
            });
        })
        .detach()
    }
    fn select(&mut self, i: usize, w: &mut Window, cx: &mut Context<Self>) {
        self.list.select(i);
        self.list.focus.focus(w, cx);
        if !self.changing {
            self.notice.clear()
        }
        cx.notify()
    }
    fn keys(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        if let Some(prompt) = &self.prompt {
            if let Prompt::Unlock(_, field) = prompt {
                match k.key.as_str() {
                    "escape" => {
                        self.prompt = None;
                        self.list.focus.focus(w, cx);
                        cx.notify();
                    }
                    "enter" if field.focus_handle(cx).is_focused(w) => self.unlock(w, cx),
                    "tab" => {
                        let order = [
                            field.focus_handle(cx),
                            self.cancel.clone(),
                            self.unlock_button.clone(),
                        ];
                        let i = order.iter().position(|f| f.is_focused(w)).unwrap_or(0);
                        order[(i + if k.modifiers.shift { 2 } else { 1 }) % 3].focus(w, cx);
                    }
                    _ => return,
                }
                w.prevent_default();
                cx.stop_propagation()
            }
            return;
        }
        match (k.key.as_str(), k.modifiers.control) {
            ("e" | "u", true) => {
                if let Some(action) = self
                    .list
                    .selected
                    .and_then(|i| self.items.get(i))
                    .and_then(|n| {
                        n.actions.iter().find(|a| {
                            matches!(
                                a.operation,
                                mundane_services::disks::Operation::Lock
                                    | mundane_services::disks::Operation::Unlock
                            ) == (k.key == "u")
                        })
                    })
                    .cloned()
                {
                    self.ask(action, w, cx)
                }
            }
            ("m", true) => {
                if let Some(t) = self.selected().cloned() {
                    self.change(t, w, cx)
                }
            }
            ("f5", false) => self.refresh(w, cx),
            ("f6", false) => self.refresh_button.focus(w, cx),
            ("escape", false) => self.list.focus.focus(w, cx),
            _ if self.list.focus.is_focused(w) && self.list.navigate(&k.key) => {
                if !self.changing {
                    self.notice.clear()
                }
                cx.notify()
            }
            _ if browser_key(k, w, cx) => {}
            _ => return,
        }
        w.prevent_default();
        cx.stop_propagation()
    }
}
impl Render for Disks {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        let view = cx.weak_entity();
        let header = button("Refresh", &self.refresh_button, theme, move |w, cx| {
            let _ = view.update(cx, |this, cx| this.refresh(w, cx));
        });
        let v = cx.weak_entity();
        let list = self
            .list
            .render("Drives and volumes", theme, w, move |i, _, w, cx| {
                let _ = v.update(cx, |this, cx| this.select(i, w, cx));
            });
        let target = self.selected().cloned();
        let enabled = target.is_some() && !self.changing && self.error.is_none();
        let action = if target.as_ref().is_some_and(|v| !v.mount_points.is_empty()) {
            "Unmount"
        } else {
            "Mount"
        };
        let view = cx.weak_entity();
        let change = unavailable(
            button(action, &self.change_button, theme, move |w, cx| {
                if enabled && let Some(t) = target.clone() {
                    let _ = view.update(cx, |this, cx| this.change(t, w, cx));
                }
            }),
            !enabled,
        );
        let selected = self.list.selected.and_then(|i| self.items.get(i));
        let mut controls = div().flex().gap_2().child(change);
        if let Some(item) = selected {
            for (i, target) in item.actions.iter().cloned().enumerate() {
                let v = cx.weak_entity();
                controls = controls.child(unavailable(
                    button(
                        target.operation.label(),
                        &self.operations[i],
                        theme,
                        move |w, cx| {
                            let _ = v.update(cx, |this, cx| this.ask(target.clone(), w, cx));
                        },
                    ),
                    self.changing,
                ));
            }
        }
        let table = self.list.selected.and_then(|i| {
            if !self.items[i].regions.is_empty() {
                return self.items.get(i);
            }
            if self.items[i].depth == 0 {
                return self.items[i + 1..]
                    .iter()
                    .take_while(|n| n.depth > 0)
                    .find(|n| !n.regions.is_empty());
            }
            None
        });
        let status = self.error.as_ref().cloned().unwrap_or_else(|| {
            if !self.notice.is_empty() {
                self.notice.clone()
            } else if self.items.is_empty() {
                if self.refreshing {
                    "Reading devices…"
                } else {
                    "No drives or volumes found."
                }
                .into()
            } else if let Some(v) = self.selected() {
                if v.mount_points.is_empty() {
                    "Volume is unmounted.".into()
                } else {
                    format!(
                        "Mounted at {}",
                        v.mount_points
                            .iter()
                            .map(|p| p.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            } else {
                "Select a filesystem to mount or unmount.".into()
            }
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
                    .render("Disks", header, &self.list.focus, theme, w),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .bg(rgb(theme.panel))
                    .child(
                        mundane_ui::heading("storage-heading", "Storage devices")
                            .font_weight(FontWeight::SEMIBOLD),
                    )
                    .child(controls),
            )
            .when_some(table, |el, table| {
                let mut bar = div().h(rems(1.)).w_full().flex().overflow_hidden();
                let mut legend = vec![];
                for r in &table.regions {
                    let color = if r.free {
                        theme.panel
                    } else if legend.len() % 2 == 0 {
                        theme.focus
                    } else {
                        theme.selection
                    };
                    bar = bar.child(
                        div()
                            .h_full()
                            .w(relative(r.bytes as f32 / table.size.max(1) as f32))
                            .bg(rgb(color))
                            .border_1()
                            .border_color(rgb(theme.border)),
                    );
                    legend.push(format!("{} {}", r.label, byte_size(r.bytes)));
                }
                let label = legend.join(" · ");
                el.child(
                    div().px_3().py_2().child(bar).child(
                        mundane_ui::text_label("partition-layout", label.clone())
                            .text_sm()
                            .child(label),
                    ),
                )
            })
            .child(list)
            .child(
                Role::Status
                    .control("disk-status", status.clone())
                    .px_3()
                    .py_2()
                    .bg(rgb(theme.panel))
                    .child(status),
            )
            .child(self.overlay(cx))
    }
}
fn main() {
    browser_window("Disks", "org.joelinux.mundane.Disks", |w, cx| {
        let app = Disks::new(UDisks::system(), w, cx);
        app.watch(w, cx);
        app
    })
}
#[cfg(test)]
mod tests;
