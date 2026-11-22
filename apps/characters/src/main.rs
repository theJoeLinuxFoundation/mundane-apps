mod catalog;
use catalog::{Catalog, Entry};
use gpui::{prelude::*, *};
use mundane_ui::{
    BrowserList, BrowserRow, ControlRole, Role, TextField, Theme, WindowHeader, browser_key,
    browser_window, button, text_label, unavailable,
};
use std::sync::Arc;
const PAGE: usize = 64;

struct Characters {
    header: WindowHeader,
    query: Entity<TextField>,
    list: BrowserList,
    copy: FocusHandle,
    prev: FocusHandle,
    next: FocusHandle,
    catalog: Option<Arc<Catalog>>,
    matches: Vec<usize>,
    exact: Option<Entry>,
    page: usize,
    notice: String,
    last_query: Option<String>,
    generation: u64,
}
impl Characters {
    fn new(w: &mut Window, cx: &mut Context<Self>) -> Self {
        mundane_ui::bind_text_keys(cx);
        let theme = Theme::current(cx);
        let query = cx.new(|cx| TextField::new("Name or code point", theme, cx));
        query.focus_handle(cx).focus(w, cx);
        cx.observe(&query, |this, _, cx| this.search(cx)).detach();
        let load = cx.background_executor().spawn(async { Catalog::load() });
        cx.spawn(async move |v, cx| {
            let result = load.await;
            let _ = v.update(cx, |this, cx| match result {
                Ok(c) => {
                    this.catalog = Some(Arc::new(c));
                    this.search(cx)
                }
                Err(e) => {
                    this.notice = e;
                    cx.notify()
                }
            });
        })
        .detach();
        Self {
            header: WindowHeader::new(cx),
            query,
            list: BrowserList::new(cx),
            copy: cx.focus_handle().tab_stop(true),
            prev: cx.focus_handle().tab_stop(true),
            next: cx.focus_handle().tab_stop(true),
            catalog: None,
            matches: vec![],
            exact: None,
            page: 0,
            notice: "Reading Unicode names…".into(),
            last_query: None,
            generation: 0,
        }
    }
    fn count(&self) -> usize {
        self.matches.len() + usize::from(self.exact.is_some())
    }
    fn entry(&self, index: usize) -> Option<&Entry> {
        if self.exact.is_some() {
            self.exact.as_ref().filter(|_| index == 0)
        } else {
            self.catalog
                .as_ref()
                .zip(self.matches.get(index))
                .map(|(c, &i)| c.get(i))
        }
    }
    fn rows(&mut self, cx: &mut Context<Self>) {
        let start = self.page * PAGE;
        let rows = (start..self.count().min(start + PAGE))
            .filter_map(|i| self.entry(i))
            .map(|e| BrowserRow {
                key: e.code().into(),
                label: e.name.clone(),
                detail: e.code(),
                value: e.glyph.to_string(),
                depth: 0,
                fraction: None,
            })
            .collect();
        self.list.replace(rows);
        self.list.select(0);
        cx.notify()
    }
    fn search(&mut self, cx: &mut Context<Self>) {
        let Some(catalog) = self.catalog.clone() else {
            return;
        };
        let query = self.query.read(cx).text().trim().to_owned();
        if self.last_query.as_ref() == Some(&query) {
            return;
        }
        self.last_query = Some(query.clone());
        self.generation += 1;
        let generation = self.generation;
        self.page = 0;
        self.matches.clear();
        self.exact = None;
        self.notice.clear();
        if let Some(result) = catalog.exact(&query) {
            match result {
                Ok(e) => self.exact = Some(e),
                Err(e) => self.notice = e.into(),
            }
            self.rows(cx);
            return;
        }
        self.notice = "Searching…".into();
        self.rows(cx);
        let job = cx
            .background_executor()
            .spawn(async move { catalog.find(&query) });
        cx.spawn(async move |v, cx| {
            let rows = job.await;
            let _ = v.update(cx, |this, cx| {
                if this.generation == generation {
                    this.notice.clear();
                    this.matches = rows;
                    this.rows(cx)
                }
            });
        })
        .detach()
    }
    fn turn(&mut self, forward: bool, cx: &mut Context<Self>) {
        let last = self.count().saturating_sub(1) / PAGE;
        self.page = if forward {
            (self.page + 1).min(last)
        } else {
            self.page.saturating_sub(1)
        };
        self.notice.clear();
        self.rows(cx)
    }
    fn selected(&self) -> Option<&Entry> {
        self.list
            .selected
            .and_then(|i| self.entry(self.page * PAGE + i))
    }
    fn copy(&mut self, cx: &mut Context<Self>) {
        if let Some(entry) = self.selected() {
            cx.write_to_clipboard(ClipboardItem::new_string(entry.glyph.to_string()));
            self.notice = format!("Copied {}", entry.code());
            cx.notify()
        }
    }
    fn keys(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        match (k.key.as_str(), k.modifiers.control) {
            ("f6", false) => self.copy.focus(w, cx),
            ("f", true) => {
                self.query.focus_handle(cx).focus(w, cx);
                self.query
                    .update(cx, |f, cx| f.select_match(0..f.text().len(), cx))
            }
            ("enter" | "down", false) if self.query.focus_handle(cx).is_focused(w) => {
                self.list.focus.focus(w, cx)
            }
            ("enter", false) | ("c", true) if self.list.focus.is_focused(w) => self.copy(cx),
            ("pagedown", false) if self.list.focus.is_focused(w) => self.turn(true, cx),
            ("pageup", false) if self.list.focus.is_focused(w) => self.turn(false, cx),
            _ if self.list.focus.is_focused(w) && self.list.navigate(&k.key) => {
                self.notice.clear();
                cx.notify()
            }
            _ if browser_key(k, w, cx) => {}
            _ => return,
        }
        w.prevent_default();
        cx.stop_propagation()
    }
}
impl Render for Characters {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        let v = cx.weak_entity();
        let copy = unavailable(
            button("Copy character", &self.copy, t, move |_, cx| {
                let _ = v.update(cx, |v, cx| v.copy(cx));
            }),
            self.selected().is_none(),
        );
        let v = cx.weak_entity();
        let list = self
            .list
            .render("Matching characters", t, w, move |i, double, w, cx| {
                let _ = v.update(cx, |v, cx| {
                    v.list.select(i);
                    v.list.focus.focus(w, cx);
                    v.notice.clear();
                    if double {
                        v.copy(cx)
                    }
                    cx.notify()
                });
            });
        let status = if !self.notice.is_empty() {
            self.notice.clone()
        } else if self.count() == 0 {
            "No matching characters".into()
        } else {
            format!(
                "{}–{} of {}",
                self.page * PAGE + 1,
                ((self.page + 1) * PAGE).min(self.count()),
                self.count()
            )
        };
        let mut bar = div().flex().gap_2().items_center();
        for (label, focus, forward, disabled) in [
            ("Previous", &self.prev, false, self.page == 0),
            (
                "Next",
                &self.next,
                true,
                (self.page + 1) * PAGE >= self.count(),
            ),
        ] {
            let v = cx.weak_entity();
            bar = bar.child(unavailable(
                button(label, focus, t, move |_, cx| {
                    if !disabled {
                        let _ = v.update(cx, |v, cx| v.turn(forward, cx));
                    }
                }),
                disabled,
            ))
        }
        let mut preview = div()
            .w(relative(0.4))
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .bg(rgb(t.panel));
        if let Some(e) = self.selected() {
            let glyph = e.glyph.to_string();
            let code = e.code();
            preview = preview
                .child(
                    text_label("glyph", glyph.clone())
                        .text_size(rems(5.))
                        .child(glyph),
                )
                .child(
                    text_label("name", e.name.clone())
                        .text_center()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(e.name.clone()),
                )
                .child(
                    text_label("code", code.clone())
                        .text_color(rgb(t.secondary))
                        .child(code),
                )
        }
        div()
            .size_full()
            .font_family("DejaVu Sans")
            .bg(rgb(t.surface))
            .text_color(rgb(t.text))
            .flex()
            .flex_col()
            .on_key_down(cx.listener(Self::keys))
            .child(
                self.header
                    .render("Characters", copy, &self.query.focus_handle(cx), t, w),
            )
            .child(div().p_3().child(self.query.clone()))
            .child(div().flex_1().min_h_0().flex().child(list).child(preview))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .bg(rgb(t.panel))
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(Role::Status.control("status", status.clone()).child(status))
                    .child(bar),
            )
    }
}
fn main() {
    browser_window(
        "Characters",
        "org.joelinux.mundane.Characters",
        Characters::new,
    )
}
#[cfg(test)]
mod tests {
    use super::Characters;
    use gpui::TestAppContext;
    #[gpui::test]
    fn copy_supplementary(cx: &mut TestAppContext) {
        let (v, ui) = cx.add_window_view(Characters::new);
        ui.run_until_parked();
        ui.simulate_keystrokes("1 d 1 1 e enter ctrl-c");
        ui.update(|_, cx| {
            assert_eq!(v.read(cx).selected().unwrap().glyph, '𝄞');
            assert_eq!(
                cx.read_from_clipboard().unwrap().text().as_deref(),
                Some("𝄞")
            )
        });
    }
}
