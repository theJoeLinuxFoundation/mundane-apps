mod book;
mod reading;
#[cfg(test)]
mod tests;
mod view;

use book::{Book, Hit, Raster};
use gpui::{prelude::*, *};
use mundane_ui::{TextField, Theme, WindowHeader};
use std::{path::PathBuf, sync::Arc};

const APP_ID: &str = "org.joelinux.mundane.DocumentViewer";
const ZOOMS: [f32; 8] = [0.5, 0.75, 1., 1.25, 1.5, 2., 3., 4.];
const NORMAL: usize = 2;

#[derive(Clone, Copy)]
enum Action {
    Open,
    Find,
    Previous,
    Next,
    Smaller,
    Larger,
    PreviousHit,
    NextHit,
    Done,
}

#[derive(Clone)]
struct Sheet {
    glyphs: Arc<Vec<book::Glyph>>,
    image: Arc<RenderImage>,
    logical: (f32, f32),
    highlights: Vec<[f32; 4]>,
    text: SharedString,
}
impl From<Raster> for Sheet {
    fn from(r: Raster) -> Self {
        Self {
            glyphs: Arc::new(r.glyphs),
            image: Arc::new(RenderImage::new([image::Frame::new(r.pixels)])),
            logical: r.logical,
            highlights: r.highlights,
            text: r.text.into(),
        }
    }
}

#[derive(Default, Clone)]
struct Matches {
    query: String,
    hits: Arc<Vec<Hit>>,
    index: Option<usize>,
}

#[derive(Clone)]
struct Location {
    book: Arc<Book>,
    page: i32,
    zoom: usize,
    matches: Matches,
}
impl Location {
    fn hit(&self) -> Option<&Hit> {
        self.matches
            .index
            .and_then(|i| self.matches.hits.get(i))
            .filter(|h| h.page == self.page)
    }
}

enum Request {
    Open(PathBuf),
    Page(i32),
    Zoom(usize),
    Search(String, bool),
    Dismiss,
}
impl Request {
    fn load(self, old: Option<Location>, scale: f32) -> anyhow::Result<(Location, Raster)> {
        let state = match self {
            Self::Open(path) => Location {
                book: Arc::new(Book::open(&path)?),
                page: 1,
                zoom: NORMAL,
                matches: Default::default(),
            },
            cmd => {
                let mut state = old.expect("document command requires an open book");
                match cmd {
                    Self::Page(n) => {
                        state.page = n;
                        state.matches.index = None;
                    }
                    Self::Zoom(z) => state.zoom = z,
                    Self::Dismiss => state.matches = Default::default(),
                    Self::Search(query, back) => {
                        if state.matches.query != query || query.is_empty() {
                            let hits = state.book.search(&query)?;
                            state.matches = Matches {
                                query,
                                hits: Arc::new(hits),
                                index: None,
                            };
                        }
                        let hits = &state.matches.hits;
                        if !hits.is_empty() {
                            let i = match state.matches.index {
                                Some(i) if back => (i + hits.len() - 1) % hits.len(),
                                Some(i) => (i + 1) % hits.len(),
                                None if back => hits
                                    .iter()
                                    .rposition(|h| h.page <= state.page)
                                    .unwrap_or(hits.len() - 1),
                                None => hits.iter().position(|h| h.page >= state.page).unwrap_or(0),
                            };
                            state.page = hits[i].page;
                            state.matches.index = Some(i);
                        }
                    }
                    Self::Open(_) => unreachable!(),
                }
                state
            }
        };
        let raster = state
            .book
            .render(state.page, ZOOMS[state.zoom], scale, state.hit())?;
        Ok((state, raster))
    }
}

struct Viewer {
    state: Option<Location>,
    sheet: Option<Sheet>,
    busy: bool,
    finding: bool,
    error: Option<String>,
    page_field: Entity<TextField>,
    query: Entity<TextField>,
    content: FocusHandle,
    tools: [FocusHandle; 9],
    header: WindowHeader,
    scroll: ScrollHandle,
    raster_scale: f32,
    flow: Option<reading::Flow>,
    outline: Entity<mundane_ui::ChoiceMenu>,
    flow_button: FocusHandle,
    print_button: FocusHandle,
    selection: Option<reading::Selection>,
    rendered_page: i32,
}

impl Viewer {
    fn new(w: &mut Window, cx: &mut Context<Self>) -> Self {
        let theme = Theme::current(cx);
        let content = cx.focus_handle().tab_stop(true);
        content.focus(w, cx);
        w.set_window_title("Document Viewer");
        let outline =
            cx.new(|cx| mundane_ui::ChoiceMenu::new("Document outline", Vec::new(), true, cx));
        cx.observe(&outline, |_, _, cx| cx.notify()).detach();
        cx.subscribe_in(
            &outline,
            w,
            |this: &mut Self, _, event: &mundane_ui::MenuEvent, w, cx| {
                if let mundane_ui::MenuEvent::Selected(key) = event
                    && let Some(n) = key.split(':').next().and_then(|s| s.parse::<i32>().ok())
                {
                    this.request(Request::Page(n), w, cx);
                }
            },
        )
        .detach();
        Self {
            flow: None,
            outline,
            flow_button: cx.focus_handle().tab_stop(true),
            print_button: cx.focus_handle().tab_stop(true),
            selection: None,
            rendered_page: 0,
            state: None,
            sheet: None,
            busy: false,
            finding: false,
            error: None,
            page_field: cx.new(|cx| TextField::new("Page number", theme, cx)),
            query: cx.new(|cx| TextField::new("Find text", theme, cx)),
            content,
            tools: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            header: WindowHeader::new(cx),
            scroll: ScrollHandle::new(),
            raster_scale: w.scale_factor(),
        }
    }

    fn request(&mut self, req: Request, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.error = None;
        let scale = w.scale_factor();
        let opened = matches!(req, Request::Open(_));
        let reset_scroll = matches!(
            req,
            Request::Open(_) | Request::Page(_) | Request::Search(..)
        );
        let old = self.state.clone();
        let work = cx
            .background_executor()
            .spawn(async move { req.load(old, scale) });
        cx.spawn_in(w, async move |view, cx| {
            let result = work.await;
            let _ = view.update_in(cx, |this, w, cx| {
                this.busy = false;
                match result {
                    Ok((state, raster)) => {
                        this.selection = None;
                        this.rendered_page = state.page;
                        if opened {
                            let choices = state
                                .book
                                .outline
                                .iter()
                                .enumerate()
                                .map(|(i, (title, page))| {
                                    mundane_ui::MenuChoice::new(
                                        format!("{page}:{i}"),
                                        format!("{title} · {page}"),
                                    )
                                })
                                .collect();
                            this.outline.update(cx, |m, cx| m.replace(choices, w, cx));
                            w.set_window_title(&format!("{} — Document Viewer", state.book.name));
                            this.finding = false;
                            this.content.focus(w, cx);
                        }
                        this.page_field.update(cx, |f, cx| {
                            f.select_match(0..f.text().len(), cx);
                            f.replace_text_in_range(None, &state.page.to_string(), w, cx);
                        });
                        if let Some(prev) = this.sheet.replace(raster.into()) {
                            let _ = w.drop_image(prev.image);
                        }
                        this.raster_scale = scale;
                        let in_flow = this.flow.is_some();
                        this.state = Some(state);
                        if in_flow {
                            this.start_flow(w, cx);
                        }
                        if reset_scroll {
                            this.scroll.set_offset(Default::default());
                        }
                        if let Some(sheet) = &this.sheet
                            && let Some(rect) = sheet.highlights.first()
                        {
                            let scroll = this.scroll.clone();
                            let view = cx.weak_entity();
                            let center = point(
                                px((rect[0] + rect[2]) * sheet.logical.0 / 2. + 12.),
                                px((rect[1] + rect[3]) * sheet.logical.1 / 2. + 12.),
                            );
                            w.on_next_frame(move |_, cx| {
                                let size = scroll.bounds().size;
                                let max = scroll.max_offset();
                                scroll.set_offset(point(
                                    (size.width / 2. - center.x).clamp(-max.x, px(0.)),
                                    (size.height / 2. - center.y).clamp(-max.y, px(0.)),
                                ));
                                let _ = view.update(cx, |_, cx| cx.notify());
                            });
                        }
                    }
                    Err(e) => this.error = Some(format!("{e:#}")),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn choose(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.busy = true;
        self.error = None;
        cx.notify();
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(w, async move |view, cx| {
            let choice = choice.await;
            let _ = view.update_in(cx, |this, w, cx| {
                this.busy = false;
                match choice {
                    Ok(Ok(paths)) => {
                        if let Some(path) = paths.and_then(|v| v.into_iter().next()) {
                            this.request(Request::Open(path), w, cx)
                        }
                    }
                    Ok(Err(e)) => this.error = Some(format!("File chooser unavailable. {e}")),
                    Err(e) => this.error = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn enabled(&self, a: Action) -> bool {
        if self.busy {
            return false;
        }
        let Some(s) = &self.state else {
            return matches!(a, Action::Open);
        };
        match a {
            Action::Previous => s.page > 1,
            Action::Next => s.page < s.book.pages,
            Action::Smaller => s.zoom > 0,
            Action::Larger => s.zoom + 1 < ZOOMS.len(),
            _ => true,
        }
    }

    fn act(&mut self, a: Action, w: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled(a) {
            return;
        }
        if matches!(a, Action::Open) {
            self.choose(w, cx);
            return;
        }
        let Some(s) = &self.state else { return };
        let req = match a {
            Action::Find => {
                self.finding = true;
                self.query.update(cx, |f, cx| {
                    f.select_match(0..f.text().len(), cx);
                    f.focus_handle(cx).focus(w, cx);
                });
                cx.notify();
                return;
            }
            Action::Done => {
                self.finding = false;
                self.content.focus(w, cx);
                Request::Dismiss
            }
            Action::Previous => Request::Page(s.page - 1),
            Action::Next => Request::Page(s.page + 1),
            Action::Smaller => Request::Zoom(s.zoom - 1),
            Action::Larger => Request::Zoom(s.zoom + 1),
            Action::NextHit | Action::PreviousHit => Request::Search(
                self.query.read(cx).text().to_string(),
                matches!(a, Action::PreviousHit),
            ),
            Action::Open => unreachable!(),
        };
        self.request(req, w, cx);
    }

    fn jump(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = &self.state else { return };
        let page = self.page_field.read(cx).text().trim().parse::<i32>().ok();
        if let Some(n) = page.filter(|n| (1..=s.book.pages).contains(n)) {
            self.content.focus(w, cx);
            self.request(Request::Page(n), w, cx);
        } else {
            self.error = Some(format!("Enter a page from 1 to {}.", s.book.pages));
            cx.notify();
        }
    }
}

fn main() {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("Document Viewer requires a Wayland session.");
        std::process::exit(1);
    }
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    mundane_ui::run(move |cx| {
        mundane_ui::bind_text_keys(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit()
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(820.), px(620.)), cx);
        if let Err(e) = cx.open_window(
            WindowOptions {
                app_id: Some(APP_ID.into()),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            |w, cx| {
                cx.new(|cx| {
                    let mut viewer = Viewer::new(w, cx);
                    if let Some(path) = path {
                        viewer.request(Request::Open(path), w, cx);
                    }
                    viewer
                })
            },
        ) {
            eprintln!("Could not open Document Viewer. {e}");
            std::process::exit(1);
        }
    });
}
