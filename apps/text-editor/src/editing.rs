use super::*;
use std::{sync::Arc, time::Duration};

impl Editor {
    pub(super) fn restore(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let result = (|| -> std::io::Result<()> {
            for (slot, draft) in state::recover(&state::directory()?)? {
                let (doc, text) = draft.into_document()?;
                if self.page().dirty(cx) {
                    self.new_tab(w, cx);
                }
                let page = &mut self.pages[self.active];
                page.document = doc;
                page.slot = Some(Arc::new(slot));
                page.recovered = true;
                page.body
                    .update(cx, |field, cx| field.set_document(text.into(), cx));
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.error = Some(format!("Could not read recovery copies. {e}"));
        }
    }
    pub(super) fn file_ready(&mut self, cx: &mut Context<Self>) {
        let p = &mut self.pages[self.active];
        p.recovered = false;
        let path = p.document.path.clone().unwrap();
        if let Err(e) = p.clear_draft() {
            self.error = Some(format!("Could not clear the recovery copy. {e}"));
        }
        if let Err(e) = state::directory().and_then(|d| state::remember(&d, &path)) {
            self.error = Some(format!("Could not update recent files. {e}"));
        }
        self.changed(self.page().body.clone(), cx)
    }
    pub(super) fn changed(&mut self, field: Entity<TextField>, cx: &mut Context<Self>) {
        cx.notify();
        let Some(p) = self.pages.iter_mut().find(|p| p.body == field) else {
            return;
        };
        let text = field.read(cx).snapshot();
        let language = syntax::language(p.document.path.as_deref());
        if p.seen
            .as_ref()
            .is_some_and(|(s, l)| s == &text && *l == language)
        {
            return;
        }
        p.seen = Some((text.clone(), language));
        if !p.pending {
            p.pending = true;
            let field = field.clone();
            let timer = cx.background_executor().timer(Duration::from_millis(600));
            cx.spawn(async move |view, cx| {
                timer.await;
                let _ = view.update(cx, |v, cx| v.autosave(&field, cx));
            })
            .detach();
        }
        if let Some(lang) = language {
            let snapshot = text.clone();
            let work = cx
                .background_executor()
                .spawn(async move { syntax::spans(lang, &snapshot) });
            cx.spawn(async move |view, cx| {
                let spans = work.await;
                let _ = view.update(cx, |v, cx| {
                    if v.pages.iter().any(|p| {
                        p.body == field
                            && p.seen
                                .as_ref()
                                .is_some_and(|(s, l)| s == &text && *l == language)
                    }) {
                        field.update(cx, |f, cx| f.set_highlights(spans, cx));
                    }
                });
            })
            .detach();
        } else {
            field.update(cx, |f, cx| f.set_highlights(vec![], cx));
        }
    }
    fn autosave(&mut self, field: &Entity<TextField>, cx: &mut Context<Self>) {
        let Some(p) = self.pages.iter_mut().find(|p| &p.body == field) else {
            return;
        };
        p.pending = false;
        let result = (|| -> std::io::Result<()> {
            if !p.dirty(cx) {
                return p.clear_draft();
            }
            let slot = match &p.slot {
                Some(slot) => slot.clone(),
                None => {
                    let slot = Arc::new(state::Slot::create(&state::directory()?)?);
                    p.slot = Some(slot.clone());
                    slot
                }
            };
            let revision = slot.next();
            let doc = p.document.clone();
            let text = field.read(cx).snapshot();
            let work = cx
                .background_executor()
                .spawn(async move { slot.write(revision, &state::Draft::new(&doc, &text)?) });
            cx.spawn(async move |view, cx| {
                if let Err(e) = work.await {
                    let _ = view.update(cx, |v, cx| {
                        v.error = Some(format!("Could not autosave the draft. {e}"));
                        cx.notify();
                    });
                }
            })
            .detach();
            Ok(())
        })();
        if let Err(e) = result {
            self.error = Some(format!("Could not autosave the draft. {e}"));
            cx.notify();
        }
    }
    pub(super) fn show_recent(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        match state::directory().and_then(|d| state::recent(&d)) {
            Ok(paths) => {
                self.recent.replace(
                    paths
                        .into_iter()
                        .map(|p| mundane_ui::BrowserRow {
                            label: p.file_name().unwrap_or_default().to_string_lossy().into(),
                            detail: {
                                let parent = p.parent().unwrap_or(&p);
                                dirs::home_dir()
                                    .and_then(|home| {
                                        parent.strip_prefix(home).ok().map(|relative| {
                                            Path::new("~").join(relative).display().to_string()
                                        })
                                    })
                                    .unwrap_or_else(|| parent.display().to_string())
                            },
                            key: p,
                            value: String::new(),
                            depth: 0,
                            fraction: None,
                        })
                        .collect(),
                );
                self.showing_recent = true;
                self.searching = false;
                self.going = false;
                self.recent.focus.focus(w, cx);
            }
            Err(e) => self.error = Some(format!("Could not read recent files. {e}")),
        }
    }
    pub(super) fn open_recent(&mut self, i: usize, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(row) = self.recent.rows.get(i) {
            let path = row.key.clone();
            self.showing_recent = false;
            self.open(path, w, cx);
        }
    }
    pub(super) fn replace(&mut self, all: bool, w: &mut Window, cx: &mut Context<Self>) {
        let q = self.query.read(cx).snapshot();
        if q.is_empty() {
            self.result = Some("Enter text to find.".into());
            return;
        }
        let replacement = self.replacement.read(cx).snapshot();
        let field = self.page().body.clone();
        let n = if all {
            field.update(cx, |f, cx| f.replace_all(&q, &replacement, cx))
        } else {
            if field.read(cx).text()[field.read(cx).selection()] != *q {
                self.find_match(false, cx);
            }
            if field.read(cx).text()[field.read(cx).selection()] == *q {
                field.update(cx, |f, cx| {
                    f.replace_text_in_range(None, &replacement, w, cx)
                });
                self.find_match(true, cx);
                1
            } else {
                0
            }
        };
        self.result = Some(format!(
            "Replaced {n} {}.",
            if n == 1 { "match" } else { "matches" }
        ));
    }
}
