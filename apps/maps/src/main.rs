mod atlas;
mod geo;
mod source;
mod vector;
use atlas::{Atlas, Scene, Viewport};
use geo::{Camera, Size};
use gpui::{prelude::*, *};
use mundane_ui::{
    BrowserList, BrowserRow, ControlRole, Role, TextField, Theme, WindowHeader, bind_text_keys,
    browser_key, browser_window, button, external_link, focus_ring, unavailable,
};
use source::{Area, Place, Source};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

struct Maps {
    source: Source,
    theme: Theme,
    header: WindowHeader,
    query: Entity<TextField>,
    results: BrowserList,
    places: Vec<Place>,
    show_results: bool,
    searching: bool,
    search_status: String,
    focus: FocusHandle,
    search: FocusHandle,
    zoom_in: FocusHandle,
    zoom_out: FocusHandle,
    save: FocusHandle,
    open: FocusHandle,
    credits: [FocusHandle; 4],
    camera: Camera,
    place: Option<Place>,
    size: Option<Size>,
    scene: Option<Scene>,
    ready: bool,
    status: String,
    range: (u8, u8),
    offline: Option<Arc<Area>>,
    has_area: bool,
    opening: bool,
    saving: bool,
    drag: Option<Point<Pixels>>,
    offset: Point<Pixels>,
    jobs: async_channel::Sender<(u64, Viewport, bool)>,
    generation: Arc<AtomicU64>,
}
impl Maps {
    fn new(source: Source, w: &mut Window, cx: &mut Context<Self>) -> Self {
        bind_text_keys(cx);
        let theme = Theme::current();
        let query = cx.new(|cx| TextField::new("Find a place", theme, cx));
        let focus = cx.focus_handle().tab_stop(true);
        focus.focus(w, cx);
        let (jobs, incoming) = async_channel::bounded::<(u64, Viewport, bool)>(1);
        let (replies, events) = async_channel::bounded(1);
        let generation = Arc::new(AtomicU64::new(0));
        let current = generation.clone();
        let provider = source.clone();
        cx.background_executor()
            .scheduler_executor()
            .spawn_dedicated(move |_| async move {
                let mut atlas = Atlas::default();
                while let Ok((id, view, retry)) = incoming.recv().await {
                    if retry {
                        atlas.clear()
                    }
                    let rendered = atlas.render(&provider, &view, || {
                        current.load(Ordering::Relaxed) != id || incoming.is_closed()
                    });
                    if replies.send((id, rendered)).await.is_err() {
                        break;
                    }
                }
            })
            .detach();
        cx.spawn_in(w, async move |v, cx| {
            while let Ok((id, rendered)) = events.recv().await {
                if v.update_in(cx, |v, w, cx| {
                    if v.generation.load(Ordering::Relaxed) != id {
                        return;
                    }
                    v.ready = true;
                    if let Some(old) = v.scene.take() {
                        let _ = w.drop_image(old.image);
                    }
                    match rendered {
                        Ok(scene) => {
                            v.camera = scene.camera;
                            v.range = scene.range;
                            v.status = if scene.missing > 0 {
                                format!(
                                    "{} unavailable tiles{}",
                                    scene.missing,
                                    scene
                                        .message
                                        .as_ref()
                                        .map(|s| format!(" · {s}"))
                                        .unwrap_or_default()
                                )
                            } else if v.offline.is_some() {
                                "Saved area · offline".into()
                            } else {
                                "Map ready".into()
                            };
                            let place = v.place.as_ref().map_or("Map", |p| p.name());
                            let mode = if v.offline.is_some() {
                                " · Offline"
                            } else {
                                ""
                            };
                            if !v.show_results {
                                w.set_window_title(&format!(
                                    "{place} · Zoom {}{mode} — Maps",
                                    v.camera.zoom
                                ));
                            }
                            v.scene = Some(scene)
                        }
                        Err(e) => v.status = format!("Map unavailable · {e}"),
                    }
                    cx.notify()
                })
                .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let mut v = Self {
            source,
            theme,
            header: WindowHeader::new(cx),
            query,
            results: BrowserList::new(cx),
            places: vec![],
            show_results: false,
            searching: false,
            search_status: String::new(),
            focus,
            search: cx.focus_handle().tab_stop(true),
            zoom_in: cx.focus_handle().tab_stop(true),
            zoom_out: cx.focus_handle().tab_stop(true),
            save: cx.focus_handle().tab_stop(true),
            open: cx.focus_handle().tab_stop(true),
            credits: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            camera: Camera {
                lat: 20.,
                lon: 0.,
                zoom: 2,
            },
            place: None,
            size: None,
            scene: None,
            ready: false,
            status: "Loading map…".into(),
            range: (0, 14),
            offline: None,
            has_area: false,
            opening: false,
            saving: false,
            drag: None,
            offset: point(px(0.), px(0.)),
            jobs,
            generation,
        };
        if v.source.has_area() {
            v.has_area = true;
            v.open_area(w, cx)
        }
        v
    }
    fn draw(&mut self, retry: bool, cx: &mut Context<Self>) {
        if self.opening {
            return;
        }
        let Some(size) = self.size else { return };
        self.ready = false;
        self.status = "Loading map…".into();
        let id = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let v = Viewport {
            size,
            camera: self.camera,
            place: self.place.clone(),
            offline: self.offline.clone(),
            contrast: self.theme.focus == Theme::HIGH_CONTRAST.focus,
        };
        if self.jobs.force_send((id, v, retry)).is_err() {
            self.status = "Map worker unavailable".into()
        }
        cx.notify()
    }
    fn pan(&mut self, x: f64, y: f64, cx: &mut Context<Self>) {
        self.camera.pan(x, y);
        self.draw(false, cx)
    }
    fn zoom(&mut self, delta: i8, cx: &mut Context<Self>) {
        let next = self
            .camera
            .zoom
            .saturating_add_signed(delta)
            .clamp(self.range.0, self.range.1);
        if next != self.camera.zoom {
            self.camera.zoom = next;
            self.draw(false, cx)
        }
    }
    fn submit(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.searching || self.opening {
            return;
        }
        let q = self.query.read(cx).text().trim().to_owned();
        if q.is_empty() {
            self.query.focus_handle(cx).focus(w, cx);
            return;
        }
        self.show_results = true;
        self.searching = true;
        self.search_status = "Searching places…".into();
        self.places.clear();
        self.results.replace(vec![]);
        cx.notify();
        let src = self.source.clone();
        let offline = self.offline.is_some();
        let job = cx
            .background_executor()
            .spawn(async move { src.places(&q, offline) });
        cx.spawn_in(w, async move |v, cx| {
            let result = job.await;
            let _ = v.update_in(cx, |v, w, cx| {
                v.searching = false;
                match result {
                    Ok(items) => {
                        v.search_status = match items.len() {
                            0 => "No matching places".into(),
                            1 => "1 search result".into(),
                            n => format!("{n} search results"),
                        };
                        v.results.replace(
                            items
                                .iter()
                                .enumerate()
                                .map(|(i, p)| BrowserRow {
                                    key: i.to_string().into(),
                                    label: p.name().into(),
                                    detail: p.display_name.clone(),
                                    value: String::new(),
                                    depth: 0,
                                    fraction: None,
                                })
                                .collect(),
                        );
                        v.places = items;
                        if v.show_results && !v.places.is_empty() {
                            w.set_window_title("Place results — Maps");
                            v.results.focus.focus(w, cx)
                        }
                    }
                    Err(e) => v.search_status = format!("Search unavailable · {e}"),
                }
                cx.notify()
            });
        })
        .detach()
    }
    fn select(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self
            .results
            .selected
            .and_then(|i| self.places.get(i))
            .cloned()
            && let Some((lat, lon)) = p.point()
        {
            self.camera = Camera {
                lat,
                lon,
                zoom: 12u8.clamp(self.range.0, self.range.1),
            };
            self.place = Some(p);
            self.show_results = false;
            self.focus.focus(w, cx);
            self.draw(false, cx)
        }
    }
    fn save_area(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if !self.ready || self.saving || self.show_results {
            return;
        }
        let Some(a) = self.scene.as_ref().and_then(|s| s.area.clone()) else {
            return;
        };
        self.saving = true;
        let src = self.source.clone();
        let job = cx.background_executor().spawn(async move { src.save(&a) });
        cx.spawn_in(w, async move |v, cx| {
            let r = job.await;
            let _ = v.update_in(cx, |v, _, cx| {
                v.saving = false;
                match r {
                    Ok(()) => {
                        v.has_area = true;
                        v.status = "Current view saved for offline use".into()
                    }
                    Err(e) => v.status = e.to_string(),
                }
                cx.notify()
            });
        })
        .detach()
    }
    fn open_area(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.opening {
            return;
        }
        self.show_results = false;
        self.focus.focus(w, cx);
        if self.offline.take().is_some() {
            self.range = (0, 14);
            self.draw(false, cx);
            return;
        }
        if !self.has_area {
            return;
        }
        self.opening = true;
        self.ready = false;
        self.generation.fetch_add(1, Ordering::Relaxed);
        let src = self.source.clone();
        let job = cx.background_executor().spawn(async move { src.open() });
        cx.spawn_in(w, async move |v, cx| {
            let r = job.await;
            let _ = v.update_in(cx, |v, _, cx| {
                v.opening = false;
                match r {
                    Ok(a) => {
                        v.camera = a.camera;
                        v.place = a.place.clone();
                        v.range = (a.camera.zoom, a.camera.zoom);
                        v.offline = Some(Arc::new(a));
                        v.draw(false, cx)
                    }
                    Err(e) => v.status = format!("Could not open saved area · {e}"),
                }
                cx.notify()
            });
        })
        .detach()
    }
    fn keys(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        let map = self.focus.is_focused(w) && !self.show_results;
        match (k.key.as_str(), k.modifiers.control) {
            ("f6", false) => if self.has_area {
                &self.open
            } else {
                &self.save
            }
            .focus(w, cx),
            ("f", true) => {
                self.query.focus_handle(cx).focus(w, cx);
                self.query
                    .update(cx, |f, cx| f.select_match(0..f.text().len(), cx))
            }
            ("enter", false) if self.query.focus_handle(cx).is_focused(w) => self.submit(w, cx),
            ("enter", false) if self.show_results && self.results.focus.is_focused(w) => {
                self.select(w, cx)
            }
            ("escape", false) => {
                self.show_results = false;
                self.focus.focus(w, cx);
                cx.notify()
            }
            ("s", true) => self.save_area(w, cx),
            ("o", true) => self.open_area(w, cx),
            ("f5", false) => self.draw(true, cx),
            ("left", false) if map => self.pan(-64., 0., cx),
            ("right", false) if map => self.pan(64., 0., cx),
            ("up", false) if map => self.pan(0., -64., cx),
            ("down", false) if map => self.pan(0., 64., cx),
            ("+" | "=", false) if map => self.zoom(1, cx),
            ("-", false) if map => self.zoom(-1, cx),
            _ if self.show_results
                && self.results.focus.is_focused(w)
                && self.results.navigate(&k.key) =>
            {
                cx.notify()
            }
            _ if browser_key(k, w, cx) => {}
            _ => return,
        }
        w.prevent_default();
        cx.stop_propagation()
    }
    fn release(&mut self, e: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(start) = self.drag.take() {
            self.offset = point(px(0.), px(0.));
            let d = start - e.position;
            if d.x != px(0.) || d.y != px(0.) {
                self.pan(f32::from(d.x) as f64, f32::from(d.y) as f64, cx)
            }
            cx.notify()
        }
    }
    fn map(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let desc = format!(
            "{:.4}°, {:.4}° · Zoom {}",
            self.camera.lat, self.camera.lon, self.camera.zoom
        );
        let desc = self
            .place
            .as_ref()
            .map_or_else(|| desc.clone(), |p| format!("{} · {desc}", p.display_name));
        let mut layer = div()
            .absolute()
            .left(self.offset.x)
            .top(self.offset.y)
            .size_full()
            .overflow_hidden();
        if let Some(scene) = &self.scene {
            layer = layer.child(
                img(scene.image.clone())
                    .absolute()
                    .size_full()
                    .object_fit(ObjectFit::Fill),
            );
            for label in &scene.labels {
                layer = layer.child(
                    div()
                        .absolute()
                        .left(px(label.x))
                        .top(px(label.y))
                        .bg(rgb(t.panel))
                        .text_color(rgb(t.text))
                        .px_1()
                        .rounded_sm()
                        .text_xs()
                        .child(label.text.clone()),
                )
            }
            if let Some((p, size)) = self.place.as_ref().zip(self.size)
                && let Some((lat, lon)) = p.point()
            {
                let (x, y) = scene.camera.position(lat, lon, size);
                layer = layer
                    .child(
                        div()
                            .absolute()
                            .left(px(x - 6.))
                            .top(px(y - 6.))
                            .size(px(12.))
                            .rounded_full()
                            .border_2()
                            .border_color(rgb(t.text))
                            .bg(rgb(t.focus)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(x + 9.))
                            .top(px(y - 12.))
                            .bg(rgb(t.selection))
                            .border_1()
                            .border_color(rgb(t.focus))
                            .rounded_md()
                            .px_2()
                            .text_sm()
                            .child(p.name().to_owned()),
                    )
            }
        }
        let old = self.size;
        let v = cx.weak_entity();
        let measure = canvas(
            move |bounds, w, cx| {
                let size = Size {
                    width: f32::from(bounds.size.width).round() as u32,
                    height: f32::from(bounds.size.height).round() as u32,
                    scale: (w.scale_factor() * 100.).round() as u16,
                };
                if size.width > 0 && size.height > 0 && old != Some(size) {
                    cx.defer(move |cx| {
                        let _ = v.update(cx, |v, cx| {
                            if v.size != Some(size) {
                                v.size = Some(size);
                                v.draw(false, cx)
                            }
                        });
                    })
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        focus_ring(
            Role::Group
                .control("map", "Map view")
                .aria_description(desc),
            &self.focus,
            t,
        )
        .relative()
        .flex_1()
        .min_h_0()
        .overflow_hidden()
        .child(layer)
        .child(measure)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|v, e: &MouseDownEvent, w, cx| {
                v.focus.focus(w, cx);
                v.drag = Some(e.position);
                cx.notify()
            }),
        )
        .on_mouse_move(cx.listener(|v, e: &MouseMoveEvent, _, cx| {
            if let Some(start) = v.drag {
                v.offset = e.position - start;
                cx.notify()
            }
        }))
        .on_mouse_up(MouseButton::Left, cx.listener(Self::release))
        .on_mouse_up_out(MouseButton::Left, cx.listener(Self::release))
        .on_scroll(cx.listener(|v, e: &ScrollEvent, _, cx| {
            let d = e.delta.pixel_delta(px(20.)).y;
            if d != px(0.) {
                v.zoom(if d > px(0.) { 1 } else { -1 }, cx)
            }
        }))
    }
}
impl Render for Maps {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let v = cx.weak_entity();
        let save = unavailable(
            button(
                if self.has_area {
                    "Replace saved area"
                } else {
                    "Save area"
                },
                &self.save,
                t,
                move |w, cx| {
                    let _ = v.update(cx, |v, cx| v.save_area(w, cx));
                },
            ),
            !self.ready
                || self.saving
                || self.show_results
                || self.scene.as_ref().is_none_or(|s| s.area.is_none()),
        );
        let v = cx.weak_entity();
        let open = unavailable(
            button(
                if self.offline.is_some() {
                    "Go online"
                } else {
                    "Open saved area"
                },
                &self.open,
                t,
                move |w, cx| {
                    let _ = v.update(cx, |v, cx| v.open_area(w, cx));
                },
            ),
            !self.has_area || self.opening,
        );
        let v = cx.weak_entity();
        let search = unavailable(
            button("Search", &self.search, t, move |w, cx| {
                let _ = v.update(cx, |v, cx| v.submit(w, cx));
            }),
            self.searching,
        );
        let mut tools = div()
            .flex()
            .gap_2()
            .items_center()
            .child(div().flex_1().min_w_0().child(self.query.clone()))
            .child(search);
        if !self.show_results {
            for (label, focus, step, disabled) in [
                (
                    "Zoom out",
                    &self.zoom_out,
                    -1,
                    self.camera.zoom <= self.range.0,
                ),
                (
                    "Zoom in",
                    &self.zoom_in,
                    1,
                    self.camera.zoom >= self.range.1,
                ),
            ] {
                let v = cx.weak_entity();
                tools = tools.child(unavailable(
                    button(label, focus, t, move |_, cx| {
                        let _ = v.update(cx, |v, cx| v.zoom(step, cx));
                    }),
                    disabled,
                ))
            }
        }
        let mut body = div()
            .size_full()
            .flex()
            .flex_col()
            .font_family("DejaVu Sans")
            .bg(rgb(t.surface))
            .text_color(rgb(t.text))
            .on_key_down(cx.listener(Self::keys))
            .child(self.header.render(
                "Maps",
                div().flex().gap_2().child(save).child(open),
                if self.show_results {
                    &self.results.focus
                } else {
                    &self.focus
                },
                t,
                w,
            ))
            .child(div().p_2().child(tools));
        if self.show_results {
            let v = cx.weak_entity();
            body = body.child(
                self.results
                    .render("Place results", t, w, move |i, _, w, cx| {
                        let _ = v.update(cx, |v, cx| {
                            v.results.select(i);
                            v.select(w, cx)
                        });
                    }),
            );
        } else {
            body = body.child(self.map(cx))
        }
        let status = if self.show_results {
            self.search_status.clone()
        } else {
            format!("Zoom {}   ·   {}", self.camera.zoom, self.status)
        };
        let mut credits = div().flex().flex_wrap().items_center();
        for (i, (name, url)) in [
            ("OpenFreeMap", "https://openfreemap.org"),
            ("© OpenMapTiles", "https://openmaptiles.org"),
            (
                "Data from OpenStreetMap",
                "https://www.openstreetmap.org/copyright",
            ),
            (
                "© OpenStreetMap contributors · ODbL",
                "https://opendatacommons.org/licenses/odbl/1-0/",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            credits = credits.child(external_link(name, url, &self.credits[i], t))
        }
        body.child(
            Role::Status
                .control("map-status", status.clone())
                .px_3()
                .py_1()
                .child(status),
        )
        .child(div().bg(rgb(t.panel)).px_2().child(credits))
    }
}
fn main() {
    let Ok(dir) = mundane_services::provider::cache_dir("maps") else {
        eprintln!("Maps needs a cache directory.");
        return;
    };
    let source = Source {
        dir,
        tiles: std::env::var("MUNDANE_MAPS_TILEJSON").unwrap_or_else(|_| source::TILES.into()),
        search: std::env::var("MUNDANE_MAPS_SEARCH").unwrap_or_else(|_| source::SEARCH.into()),
    };
    browser_window("Maps", "org.joelinux.mundane.Maps", move |w, cx| {
        Maps::new(source, w, cx)
    })
}
#[cfg(test)]
mod tests;
