//! Scrollable, keyboard-navigable rows for the filesystem and storage browsers.
use super::*;
use std::{path::PathBuf, rc::Rc, sync::Arc};

pub struct BrowserRow {
    pub key: PathBuf,
    pub label: String,
    pub detail: String,
    pub value: String,
    pub depth: usize,
    pub fraction: Option<f32>,
}
pub struct BrowserList {
    pub focus: FocusHandle,
    pub selected: Option<usize>,
    pub rows: Rc<[BrowserRow]>,
    scroll: UniformListScrollHandle,
}
impl BrowserList {
    pub fn new(cx: &App) -> Self {
        Self {
            focus: cx.focus_handle().tab_stop(true),
            selected: None,
            rows: Rc::default(),
            scroll: UniformListScrollHandle::new(),
        }
    }
    pub fn replace(&mut self, rows: Vec<BrowserRow>) {
        let old = self.selected.and_then(|i| self.rows.get(i)).map(|r| &r.key);
        let next = old
            .and_then(|key| rows.iter().position(|r| &r.key == key))
            .unwrap_or(0);
        self.rows = rows.into();
        self.select(next);
    }
    pub fn select(&mut self, i: usize) {
        self.selected = self.rows.get(i).map(|_| i);
        if self.selected.is_some() {
            self.scroll.scroll_to_item(i, ScrollStrategy::Nearest)
        }
    }
    pub fn navigate(&mut self, key: &str) -> bool {
        let last = self.rows.len().saturating_sub(1);
        let current = self.selected.unwrap_or(0);
        let i = match key {
            "home" => 0,
            "end" => last,
            "up" => current.saturating_sub(1),
            "down" => (current + 1).min(last),
            _ => return false,
        };
        self.select(i);
        true
    }
    pub fn render(
        &self,
        name: &'static str,
        theme: Theme,
        w: &Window,
        click: impl Fn(usize, bool, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let rows = self.rows.clone();
        let selected = self.selected;
        let focused = self.focus.is_focused(w);
        let click = Rc::new(click);
        let list = uniform_list("browser-rows", rows.len(), move |range, _, _| {
            range
                .map(|i| {
                    let row = &rows[i];
                    let active = Some(i) == selected;
                    let click = click.clone();
                    option(
                        ElementId::Path(Arc::from(row.key.as_path())),
                        row.label.clone(),
                        i,
                        active,
                        focused,
                    )
                    .aria_description(format!("{} {}", row.detail, row.value).trim().to_owned())
                    .h(rems(3.))
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .pr_3()
                    .pl(rems(0.75 + row.depth.min(12) as f32))
                    .border_l_2()
                    .border_color(rgb(if active { theme.focus } else { theme.surface }))
                    .when(active, |s| s.bg(rgb(theme.selection)))
                    .when(!active, |s| s.hover(move |s| s.bg(rgb(theme.panel))))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .child(div().truncate().child(row.label.clone()))
                            .when(!row.detail.is_empty(), |e| {
                                e.child(
                                    div()
                                        .truncate()
                                        .text_sm()
                                        .text_color(rgb(theme.secondary))
                                        .child(row.detail.clone()),
                                )
                            }),
                    )
                    .when_some(row.fraction, |el, f| {
                        el.child(
                            div()
                                .w(relative(0.20))
                                .h(px(5.))
                                .bg(rgb(theme.border))
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative(f.clamp(0., 1.)))
                                        .bg(rgb(theme.focus)),
                                ),
                        )
                    })
                    .child(
                        div()
                            .flex_none()
                            .text_sm()
                            .when(row.fraction.is_some(), |el| el.w(rems(6.)).text_right())
                            .child(row.value.clone()),
                    )
                    .on_click(move |e, w, cx| click(i, e.click_count() == 2, w, cx))
                })
                .collect::<Vec<_>>()
        })
        .track_scroll(&self.scroll)
        .size_full();
        focus_ring(
            Role::ListBox
                .control("items", name)
                .aria_size_of_set(self.rows.len()),
            &self.focus,
            theme,
        )
        .flex_1()
        .min_h_0()
        .overflow_hidden()
        .child(list)
    }
}

pub fn byte_size(n: u64) -> String {
    for (unit, shift) in [("TiB", 40), ("GiB", 30), ("MiB", 20), ("KiB", 10)] {
        if n >= 1 << shift {
            return format!("{:.1} {unit}", n as f64 / (1u64 << shift) as f64);
        }
    }
    format!("{n} B")
}

pub fn browser_window<T: Render>(
    title: &'static str,
    id: &'static str,
    build: impl FnOnce(&mut Window, &mut Context<T>) -> T + 'static,
) {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("{title} requires a Wayland session.");
        std::process::exit(1)
    }
    Application::with_platform(gpui_linux::current_platform(false)).run(move |cx| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit()
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(780.), px(520.)), cx);
        if let Err(e) = cx.open_window(
            WindowOptions {
                app_id: Some(id.into()),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            |w, cx| {
                w.set_window_title(title);
                cx.new(|cx| build(w, cx))
            },
        ) {
            eprintln!("Could not open {title}. {e}");
            std::process::exit(1)
        }
    });
}
pub fn browser_key(key: &Keystroke, w: &mut Window, cx: &mut App) -> bool {
    match (key.key.as_str(), key.modifiers.control) {
        ("w", true) => w.remove_window(),
        ("tab", false) if key.modifiers.shift => w.focus_prev(cx),
        ("tab", false) => w.focus_next(cx),
        ("+" | "=" | "-" | "0", true) => {
            let size = match key.key.as_str() {
                "-" => w.rem_size() - px(2.),
                "0" => px(16.),
                _ => w.rem_size() + px(2.),
            };
            w.set_rem_size(size.clamp(px(12.), px(24.)));
            w.refresh()
        }
        _ => return false,
    }
    true
}
