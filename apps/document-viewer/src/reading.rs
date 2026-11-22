use super::*;
use mundane_ui::{ControlRole, Role};
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    io::{Seek, Write},
    rc::Rc,
};

pub(super) struct Flow {
    pub list: ListState,
    cache: HashMap<i32, Result<Sheet, String>>,
    pending: HashSet<i32>,
    generation: Rc<()>,
}
pub(super) struct Selection {
    page: i32,
    start: usize,
    end: usize,
    chars: Arc<Vec<book::Glyph>>,
}
impl Selection {
    fn range(&self) -> std::ops::Range<usize> {
        self.start.min(self.end)..self.start.max(self.end)
    }
}
impl Viewer {
    pub(super) fn toggle_flow(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.state.is_none() {
            return;
        }
        if self.flow.is_some() {
            self.clear_flow(w);
            self.request(Request::Page(self.state.as_ref().unwrap().page), w, cx);
        } else {
            self.start_flow(w, cx);
        }
        self.content.focus(w, cx);
        cx.notify();
    }
    fn clear_flow(&mut self, w: &mut Window) {
        if let Some(old) = self.flow.take() {
            for sheet in old.cache.into_values().flatten() {
                let _ = w.drop_image(sheet.image);
            }
        }
    }
    pub(super) fn start_flow(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.clear_flow(w);
        let Some(s) = &self.state else { return };
        let list = ListState::new(s.book.pages as usize, ListAlignment::Top, px(32.));
        list.scroll_to(ListOffset {
            item_ix: (s.page - 1) as usize,
            offset_in_item: px(0.),
        });
        let view = cx.weak_entity();
        list.set_scroll_handler(move |event, w, cx| {
            let page = event.visible_range.start as i32 + 1;
            let _ = view.update(cx, |v, cx| v.reading_position(page, w, cx));
        });
        self.flow = Some(Flow {
            list,
            cache: Default::default(),
            pending: Default::default(),
            generation: Rc::new(()),
        });
    }
    pub(super) fn reading_position(&mut self, page: i32, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(s) = &mut self.state
            && page != s.page
            && (1..=s.book.pages).contains(&page)
        {
            s.page = page;
            if !self.page_field.focus_handle(cx).is_focused(w) {
                self.page_field.update(cx, |f, cx| {
                    f.select_match(0..f.text().len(), cx);
                    f.replace_text_in_range(None, &page.to_string(), w, cx);
                });
            }
            cx.notify();
        }
    }
    pub(super) fn flow_row(
        &mut self,
        i: usize,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let page = i as i32 + 1;
        let Some(s) = &self.state else {
            return div().into_any_element();
        };
        let Some(&(width, height)) = s.book.sizes.get(i) else {
            return div().into_any_element();
        };
        let zoom = ZOOMS[s.zoom];
        let size = (
            px((width * zoom).clamp(1., 80000.)),
            px((height * zoom).clamp(1., 80000.)),
        );
        let book = s.book.clone();
        let Some(flow) = &mut self.flow else {
            return div().into_any_element();
        };
        let sheet = if page == self.rendered_page {
            self.sheet.clone()
        } else {
            flow.cache.get(&page).and_then(|r| r.as_ref().ok()).cloned()
        };
        let text = sheet.as_ref().map(|sheet| sheet.text.clone());
        let paper = if let Some(sheet) = sheet {
            self.paper(page, &sheet, cx).into_any_element()
        } else if let Some(Err(error)) = flow.cache.get(&page) {
            div()
                .child(format!("Page {page}: {error}"))
                .into_any_element()
        } else {
            if flow.pending.len() < 2 && flow.pending.insert(page) {
                let generation = flow.generation.clone();
                let scale = w.scale_factor();
                let work = cx
                    .background_executor()
                    .spawn(async move { book.render(page, zoom, scale, None) });
                cx.spawn_in(w, async move |view, cx| {
                    let result = work.await;
                    let _ = view.update_in(cx, |v, w, cx| {
                        if let Some(f) = &mut v.flow
                            && Rc::ptr_eq(&generation, &f.generation)
                        {
                            f.pending.remove(&page);
                            let top = f.list.logical_scroll_top().item_ix as i32 + 1;
                            let bottom = top
                                + ((f32::from(w.viewport_size().height) / 80.).ceil() as i32)
                                + 2;
                            f.cache.retain(|n, r| {
                                let keep = (top - 2..=bottom).contains(n);
                                if !keep && let Ok(sheet) = r {
                                    let _ = w.drop_image(sheet.image.clone());
                                }
                                keep
                            });
                            f.cache
                                .insert(page, result.map(Into::into).map_err(|e| e.to_string()));
                            cx.notify()
                        }
                    });
                })
                .detach();
            }
            div()
                .bg(rgb(0xffffff))
                .w(size.0)
                .h(size.1)
                .into_any_element()
        };
        Role::Document
            .control(("pdf-page", i), format!("Page {page}"))
            .when_some(text, super::view::page_text)
            .w_full()
            .flex()
            .justify_center()
            .min_h(px(80.))
            .p_3()
            .child(paper)
            .into_any_element()
    }
    pub(super) fn select_keys(&mut self, key: &Keystroke, cx: &mut Context<Self>) -> bool {
        if key.modifiers.control && key.key == "c" {
            if let Some(s) = &self.selection
                && !s.range().is_empty()
            {
                cx.write_to_clipboard(ClipboardItem::new_string(
                    s.chars[s.range()].iter().map(|c| c.ch).collect(),
                ));
            }
            return true;
        }
        let all = key.modifiers.control && key.key == "a";
        if !all && !(key.modifiers.shift && matches!(key.key.as_str(), "left" | "right")) {
            return false;
        }
        let page = self.state.as_ref().map_or(0, |s| s.page);
        if self.selection.is_none() || all {
            let sheet = if page == self.rendered_page {
                self.sheet.as_ref()
            } else {
                self.flow
                    .as_ref()
                    .and_then(|f| f.cache.get(&page)?.as_ref().ok())
            };
            let Some(sheet) = sheet else { return false };
            self.selection = Some(Selection {
                page,
                start: 0,
                end: 0,
                chars: sheet.glyphs.clone(),
            });
        }
        let s = self.selection.as_mut().unwrap();
        s.end = if all {
            s.chars.len()
        } else if key.key == "left" {
            s.end.saturating_sub(1)
        } else {
            (s.end + 1).min(s.chars.len())
        };
        cx.notify();
        true
    }
    pub(super) fn paper(&self, page: i32, sheet: &Sheet, cx: &Context<Self>) -> Stateful<Div> {
        let image = sheet.image.clone();
        let glyphs = sheet.glyphs.clone();
        let selection = self
            .selection
            .as_ref()
            .filter(|s| s.page == page)
            .map(|s| s.range())
            .unwrap_or_default();
        let marks = sheet.highlights.clone();
        let selection_color = rgba((Theme::current(cx).focus << 8) | 0x70);
        let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
        let measured = bounds.clone();
        let hit = move |pos: Point<Pixels>| {
            let b = bounds.get();
            if b.size.width <= px(0.) || b.size.height <= px(0.) {
                return None;
            }
            let x = f32::from(pos.x - b.left()) / f32::from(b.size.width);
            let y = f32::from(pos.y - b.top()) / f32::from(b.size.height);
            glyphs
                .iter()
                .enumerate()
                .filter_map(|(i, g)| {
                    let [l, t, r, b] = g.rect?;
                    let distance = (x - x.clamp(l, r)).powi(2) + (y - y.clamp(t, b)).powi(2) * 4.;
                    Some((distance, if x > (l + r) * 0.5 { i + 1 } else { i }))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, i)| i)
        };
        let begin = hit.clone();
        let glyphs = sheet.glyphs.clone();
        let paint = glyphs.clone();
        let errors = cx.weak_entity();
        div()
            .id(("paper", page as usize))
            .w(px(sheet.logical.0))
            .h(px(sheet.logical.1))
            .flex_none()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |v, e: &MouseDownEvent, w, cx| {
                    if let Some(i) = begin(e.position) {
                        v.content.focus(w, cx);
                        let start = v
                            .selection
                            .as_ref()
                            .filter(|s| s.page == page && e.modifiers.shift)
                            .map_or(i, |s| s.start);
                        v.selection = Some(Selection {
                            page,
                            start,
                            end: i,
                            chars: glyphs.clone(),
                        });
                        cx.notify();
                    }
                }),
            )
            .on_mouse_move(cx.listener(move |v, e: &MouseMoveEvent, _, cx| {
                if e.pressed_button == Some(MouseButton::Left)
                    && let Some(s) = &mut v.selection
                    && s.page == page
                    && let Some(i) = hit(e.position)
                    && s.end != i
                {
                    s.end = i;
                    cx.notify();
                }
            }))
            .child(
                canvas(
                    move |b, _, _| measured.set(b),
                    move |b, (), w, cx| {
                        if let Err(e) = w.paint_image(b, b, Default::default(), image, 0, false) {
                            w.defer(cx, move |_, cx| {
                                let _ = errors.update(cx, |v, cx| {
                                    v.error = Some(format!("Could not display page. {e}"));
                                    cx.notify()
                                });
                            });
                            return;
                        }
                        let rect = |[l, t, r, d]: [f32; 4]| {
                            Bounds::new(
                                b.origin + point(b.size.width * l, b.size.height * t),
                                size(b.size.width * (r - l), b.size.height * (d - t)),
                            )
                        };
                        for mark in marks {
                            w.paint_quad(quad(
                                rect(mark),
                                px(0.),
                                gpui::transparent_black(),
                                px(1.),
                                rgb(0x795600),
                                BorderStyle::Solid,
                            ));
                        }
                        for g in &paint[selection] {
                            if let Some(area) = g.rect {
                                w.paint_quad(fill(rect(area), selection_color));
                            }
                        }
                    },
                )
                .size_full(),
            )
    }
    pub(super) fn print(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(book) = self.state.as_ref().map(|s| s.book.clone()) else {
            return;
        };
        self.busy = true;
        self.error = None;
        cx.notify();
        let work = cx.background_executor().spawn(async move {
            use ashpd::desktop::print::{PrintOptions, PrintProxy};
            let mut file = tempfile::tempfile()?;
            file.write_all(&book.original)?;
            file.rewind()?;
            let portal = PrintProxy::new().await?;
            let result = portal
                .print(None, &book.name, &file, PrintOptions::default())
                .await
                .and_then(|request| request.response());
            match result {
                Ok(()) => Ok::<_, anyhow::Error>(()),
                Err(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)) => Ok(()),
                Err(e) => Err(e.into()),
            }
        });
        cx.spawn(async move |view, cx| {
            let result = work.await;
            let _ = view.update(cx, |v, cx| {
                v.busy = false;
                v.error = result.err().map(|e| format!("Could not print. {e}"));
                cx.notify()
            });
        })
        .detach();
    }
}
