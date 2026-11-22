use super::*;
use mundane_ui::{BrowserList, BrowserRow};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct Navigation {
    pub places: BrowserList,
    pub visible: bool,
    pub bookmarks: std::collections::HashSet<PathBuf>,
    pub bookmark_focus: FocusHandle,
    revision: u64,
    pub query: Option<String>,
    cancel: Arc<AtomicBool>,
}
impl Navigation {
    pub fn new(cx: &App) -> Self {
        Self {
            places: BrowserList::new(cx),
            visible: true,
            bookmarks: Default::default(),
            bookmark_focus: cx.focus_handle().tab_stop(true),
            revision: 0,
            query: None,
            cancel: Default::default(),
        }
    }
    pub fn reset(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.query = None;
    }
}
impl Drop for Navigation {
    fn drop(&mut self) {
        self.reset()
    }
}
impl Files {
    pub(super) fn refresh_places(&mut self, cx: &mut Context<Self>) {
        self.nav.revision += 1;
        let revision = self.nav.revision;
        let task = cx.background_executor().spawn(async { files::places() });
        cx.spawn(async move |view, cx| {
            let (places, bookmarks) = task.await;
            let _ = view.update(cx, |v, cx| {
                if revision != v.nav.revision {
                    return;
                }
                v.nav.bookmarks = bookmarks;
                v.nav.places.replace(
                    places
                        .into_iter()
                        .map(|p| BrowserRow {
                            key: p.path,
                            label: p.name,
                            detail: p.group.into(),
                            value: String::new(),
                            depth: 0,
                            fraction: None,
                        })
                        .collect(),
                );
                v.select_place();
                cx.notify()
            });
        })
        .detach()
    }
    pub(super) fn select_place(&mut self) {
        self.nav.places.selected = self.nav.places.rows.iter().position(|r| r.key == self.path);
    }
    pub(super) fn bookmark(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.working {
            return;
        }
        let path = self.path.clone();
        self.working = true;
        let task = cx
            .background_executor()
            .spawn(async move { files::toggle_bookmark(&path) });
        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |v, cx| {
                v.working = false;
                v.message = Some(match result {
                    Ok(true) => "Folder bookmarked.".into(),
                    Ok(false) => "Bookmark removed.".into(),
                    Err(e) => e.to_string(),
                });
                v.refresh_places(cx);
                cx.notify()
            });
        })
        .detach();
    }
    pub(super) fn search(&mut self, query: String, cx: &mut Context<Self>) {
        self.marks = Default::default();
        self.search_with_selection(query, None, cx)
    }
    pub(super) fn search_with_selection(
        &mut self,
        query: String,
        prefer: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        self.requested = None;
        if query.trim().is_empty() {
            self.message = Some("Enter a file name to search.".into());
            return;
        }
        self.nav.reset();
        self.generation += 1;
        let generation = self.generation;
        self.nav.cancel = Default::default();
        let cancel = self.nav.cancel.clone();
        let path = self.path.clone();
        let hidden = self.show_hidden;
        self.nav.query = Some(query.clone());
        self.loading = true;
        self.selected = None;
        self.entries.clear();
        self.message = None;
        self.previews = Default::default();
        let task = cx.background_executor().spawn(async move {
            files::search(&path, &query, hidden, || cancel.load(Ordering::Relaxed))
        });
        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |v, cx| {
                if v.generation != generation {
                    return;
                }
                v.loading = false;
                match result {
                    Ok(r) => {
                        v.entries = r.entries;
                        v.marks.retain(&v.entries);
                        v.selected = prefer
                            .as_ref()
                            .and_then(|p| v.entries.iter().position(|e| &e.path == p))
                            .or_else(|| (!v.entries.is_empty()).then_some(0));
                        if let Some(i) = v.selected {
                            v.scroll.scroll_to_item(i, ScrollStrategy::Nearest);
                        }
                        let n = v.entries.len();
                        let mut status = if r.limited {
                            "Showing the first 1,000 results".into()
                        } else {
                            format!("{n} {}", if n == 1 { "result" } else { "results" })
                        };
                        if r.skipped > 0 {
                            status.push_str(&format!(" · Folders skipped: {}", r.skipped));
                        }
                        v.message = Some(status);
                    }
                    Err(e) => v.message = Some(format!("Search failed. {e}")),
                }
                cx.notify()
            });
        })
        .detach();
        cx.notify()
    }
    pub(super) fn open_place(&mut self, i: usize, w: &mut Window, cx: &mut Context<Self>) {
        if self.working {
            return;
        }
        if let Some(row) = self.nav.places.rows.get(i) {
            self.load(row.key.clone(), None, cx);
            self.list_focus.focus(w, cx);
        }
    }
    pub(super) fn navigation_key(
        &mut self,
        e: &KeyDownEvent,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let k = &e.keystroke;
        match (k.key.as_str(), k.modifiers.control) {
            ("f", true) if !self.working => self.prompt(Prompt::Search, w, cx),
            ("d", true) if !self.loading && !self.working => self.bookmark(cx),
            ("f9", false) => {
                self.nav.visible = !self.nav.visible;
                self.list_focus.focus(w, cx);
            }
            ("escape", false) if self.nav.query.is_some() => self.load(self.path.clone(), None, cx),
            ("enter", false) if self.nav.places.focus.is_focused(w) => {
                if let Some(i) = self.nav.places.selected {
                    self.open_place(i, w, cx)
                }
            }
            _ if self.nav.places.focus.is_focused(w) && self.nav.places.navigate(&k.key) => {}
            _ => return,
        }
        cx.notify();
        w.prevent_default();
        cx.stop_propagation()
    }
}
