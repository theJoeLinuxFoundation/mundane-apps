use mundane_ui::ControlRole as _;
mod closing;
mod profiles;
use closing::Target;
mod grid;
mod keys;
mod links;
mod tabs;
mod windows;

use alacritty_terminal::{
    event::Event,
    grid::{Dimensions, Scroll},
    index::{Boundary, Direction, Side},
    selection::{Selection, SelectionType},
    term::{TermMode, search::RegexSearch},
    tty::Options,
};
use gpui::{prelude::*, *};
use grid::{Frame, Layout};
use mundane_services::terminal::Session;
use mundane_ui::{TextField, Theme, WindowHeader, button, focus_ring};
use std::ops::Range;

const APP_ID: &str = "org.joelinux.mundane.Terminal";
const BODY_FONT: f32 = 16.;

struct Find {
    field: Entity<TextField>,
    submit: FocusHandle,
    return_focus: FocusHandle,
    missed: bool,
}

struct Terminal {
    profiles: profiles::Profiles,
    form: Option<profiles::Form>,
    warning: Option<closing::Warning>,
    profile_focus: FocusHandle,
    tabs: Vec<tabs::ShellTab>,
    active: usize,
    next_id: u64,
    options: Options,
    tab_scroll: ScrollHandle,
    new_focus: FocusHandle,
    window_focus: FocusHandle,
    detach_focus: FocusHandle,
    close_focus: FocusHandle,
    notice: Option<String>,
    grid_focus: FocusHandle,
    find_button: FocusHandle,
    header: WindowHeader,
    content: mundane_ui::ApplicationContent,
    find: Option<Find>,
    layout: Layout,
    wheel_remainder: f32,
    composing: String,
}

impl Terminal {
    fn new(options: Options, window: &mut Window, cx: &mut Context<Self>) -> anyhow::Result<Self> {
        let (profiles, notice) = match profiles::Profiles::load() {
            Ok(p) => (p, None),
            Err(e) => (
                Default::default(),
                Some(format!("Could not load profiles. {e}")),
            ),
        };
        let mut layout = Layout {
            bounds: Bounds::new(Point::default(), window.viewport_size()),
            font_size: BODY_FONT,
        };
        layout.font_size = profiles.selected().size.into();
        let first = Self::start(1, &profiles, options.clone(), layout, window, cx)?;
        Ok(Self::with_tab(
            options, profiles, notice, layout, first, window, cx,
        ))
    }
    fn with_tab(
        options: Options,
        profiles: profiles::Profiles,
        notice: Option<String>,
        layout: Layout,
        first: tabs::ShellTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let v = cx.weak_entity();
        window.on_window_should_close(cx, move |w, cx| {
            v.update(cx, |s, cx| s.allow_close(Target::Window, w, cx))
                .unwrap_or(true)
        });
        let v = cx.weak_entity();
        let header = WindowHeader::new(cx).on_close(move |w, cx| {
            let _ = v.update(cx, |s, cx| s.request_close(Target::Window, w, cx));
        });

        let grid_focus = cx.focus_handle().tab_stop(true);
        grid_focus.focus(window, cx);
        Self {
            next_id: first.id + 1,
            tabs: vec![first],
            active: 0,
            window_focus: cx.focus_handle().tab_stop(true),
            detach_focus: cx.focus_handle().tab_stop(true),
            options,
            tab_scroll: ScrollHandle::new(),
            new_focus: cx.focus_handle().tab_stop(true),
            close_focus: cx.focus_handle().tab_stop(true),
            profiles,
            notice,
            form: None,
            warning: None,
            profile_focus: cx.focus_handle().tab_stop(true),
            grid_focus,
            find_button: cx.focus_handle().tab_stop(true),
            content: Default::default(),
            header,
            find: None,
            layout,
            wheel_remainder: 0.,
            composing: String::new(),
        }
    }

    fn input(&mut self, bytes: Vec<u8>) {
        let mut screen = self.tabs[self.active].session.screen.lock();
        screen.scroll_display(Scroll::Bottom);
        screen.selection = None;
        drop(screen);
        self.tabs[self.active].session.write(bytes);
    }

    fn paste(&mut self, text: String) {
        let bracketed = self.tabs[self.active]
            .session
            .screen
            .lock()
            .mode()
            .contains(TermMode::BRACKETED_PASTE);
        let text = text.replace('\x1b', "");
        let bytes = if bracketed {
            format!("\x1b[200~{text}\x1b[201~")
        } else {
            text.replace("\r\n", "\n").replace('\n', "\r")
        };
        self.input(bytes.into_bytes());
    }

    fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.is_some() {
            return;
        }
        let field = cx.new(|cx| TextField::new("Find text", Theme::current(cx), cx));
        let return_focus = window
            .focused(cx)
            .unwrap_or_else(|| self.grid_focus.clone());
        field.read(cx).focus_handle(cx).focus(window, cx);
        self.find = Some(Find {
            field,
            return_focus,
            submit: cx.focus_handle(),
            missed: false,
        });
        cx.notify();
    }

    fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(find) = self.find.take() {
            find.return_focus.focus(window, cx);
        }
        cx.notify();
    }

    fn find_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(find) = &mut self.find else { return };
        let needle = find.field.read(cx).text();
        if needle.is_empty() {
            return;
        }
        let Ok(mut query) = RegexSearch::new(&regex::escape(needle)) else {
            return;
        };
        let mut screen = self.tabs[self.active].session.screen.lock();
        let origin = screen
            .selection
            .as_ref()
            .and_then(|s| s.to_range(&screen))
            .map(|range| range.end.add(&*screen, Boundary::Grid, 1))
            .unwrap_or_else(|| {
                alacritty_terminal::index::Point::new(
                    screen.topmost_line(),
                    alacritty_terminal::index::Column(0),
                )
            });
        if let Some(found) =
            screen.search_next(&mut query, origin, Direction::Right, Side::Left, None)
        {
            let mut selection = Selection::new(SelectionType::Simple, *found.start(), Side::Left);
            selection.update(*found.end(), Side::Right);
            screen.scroll_to_point(*found.start());
            screen.selection = Some(selection);
            drop(screen);
            self.find = None;
            self.grid_focus.focus(window, cx);
        } else {
            find.missed = true;
        }
        cx.notify();
    }

    fn grid_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        let mods = key.modifiers;
        if key.key == "enter"
            && mods.control
            && !mods.shift
            && let Some(url) = self.tabs[self.active]
                .session
                .screen
                .lock()
                .selection_to_string()
                .as_deref()
                .and_then(links::web_url)
        {
            cx.open_url(&url);
            window.prevent_default();
            cx.stop_propagation();
            return;
        }
        match (key.key.as_str(), mods.control, mods.shift) {
            ("c", true, true) => {
                if let Some(text) = self.tabs[self.active]
                    .session
                    .screen
                    .lock()
                    .selection_to_string()
                {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }
            ("v", true, true) => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.paste(text);
                }
            }
            ("f", true, true) => self.open_find(window, cx),
            ("pageup", false, true) => self.tabs[self.active]
                .session
                .screen
                .lock()
                .scroll_display(Scroll::PageUp),
            ("pagedown", false, true) => self.tabs[self.active]
                .session
                .screen
                .lock()
                .scroll_display(Scroll::PageDown),
            ("+" | "=", true, _) => self.layout.font_size = (self.layout.font_size + 2.).min(40.),
            ("-", true, _) => self.layout.font_size = (self.layout.font_size - 2.).max(10.),
            ("0", true, _) => self.layout.font_size = self.tabs[self.active].profile.size.into(),
            ("f6", false, _) => self.find_button.focus(window, cx),
            _ => {
                let app_cursor = self.tabs[self.active]
                    .session
                    .screen
                    .lock()
                    .mode()
                    .contains(TermMode::APP_CURSOR);
                let Some(bytes) = keys::encode(key, app_cursor) else {
                    return;
                };
                self.input(bytes);
            }
        }
        self.tabs[self.active].zoom = self.layout.font_size;
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }

    fn terminal_grid(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let painter = view.clone();
        let family: SharedString = self.tabs[self.active].profile.family.clone().into();
        let palette = self.tabs[self.active].profile.theme(Theme::current(cx));
        let surface = canvas(
            move |bounds, window, cx| {
                view.update(cx, |this, _| {
                    this.layout.bounds = bounds;
                    this.tabs[this.active].session.resize(this.layout.size());
                    Frame::read(
                        &this.tabs[this.active].session.screen.lock(),
                        this.layout,
                        palette,
                        this.grid_focus.is_focused(window),
                    )
                })
            },
            move |bounds, frame, window, cx| {
                frame.paint(family.clone(), window, cx);
                window.handle_input(
                    &painter.read(cx).grid_focus,
                    ElementInputHandler::new(bounds, painter.clone()),
                    cx,
                );
            },
        )
        .size_full();

        focus_ring(
            mundane_ui::Role::Terminal.control("grid", "Shell"),
            &self.grid_focus,
            Theme::current(cx),
        )
        .when(window.is_a11y_active(), |grid| {
            let screen = self.tabs[self.active].session.screen.clone();
            let layout = self.layout;
            let scale = window.scale_factor();
            grid.a11y_synthetic_children(move |tree| {
                grid::accessible(tree, &screen.lock(), layout, scale)
            })
        })
        .bg(rgb(palette.surface))
        .flex_1()
        .min_h_0()
        .p(px(8.))
        .overflow_hidden()
        .cursor_text()
        .on_key_down(cx.listener(Self::grid_key))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                this.grid_focus.focus(window, cx);
                let mut screen = this.tabs[this.active].session.screen.lock();
                let (point, side) = this.layout.hit(ev.position, &screen);
                if ev.modifiers.control
                    && let Some(url) = links::at(&screen, point)
                {
                    cx.open_url(&url);
                    return;
                }
                let kind = match ev.click_count {
                    2 => SelectionType::Semantic,
                    3 => SelectionType::Lines,
                    _ => SelectionType::Simple,
                };
                screen.selection = Some(Selection::new(kind, point, side));
                cx.notify();
            }),
        )
        .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
            if ev.pressed_button == Some(MouseButton::Left) {
                let mut screen = this.tabs[this.active].session.screen.lock();
                let (point, side) = this.layout.hit(ev.position, &screen);
                if let Some(selection) = &mut screen.selection {
                    selection.update(point, side);
                }
                cx.notify();
            }
        }))
        .on_scroll(cx.listener(|this, ev: &ScrollEvent, _, cx| {
            let delta = match ev.delta {
                ScrollDelta::Pixels(p) => f32::from(p.y) / this.layout.height(),
                ScrollDelta::Lines(p) => p.y,
            };
            this.wheel_remainder += delta;
            let rows = this.wheel_remainder.trunc() as i32;
            this.wheel_remainder -= rows as f32;
            this.tabs[this.active]
                .session
                .screen
                .lock()
                .scroll_display(Scroll::Delta(rows));
            cx.notify();
        }))
        .child(surface)
    }

    fn dialog(&self, find: &Find, cx: &Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        let view = cx.weak_entity();
        let field = find.field.clone();
        let submit = find.submit.clone();
        mundane_ui::Role::Dialog
            .control("find-dialog", "Find in Terminal")
            .aria_description("Search the terminal scrollback")
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x00000070))
            .flex()
            .items_center()
            .justify_center()
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => this.close_find(window, cx),
                    "enter" => this.find_text(window, cx),
                    "tab" => {
                        if submit.is_focused(window) {
                            field.read(cx).focus_handle(cx).focus(window, cx);
                        } else {
                            submit.focus(window, cx);
                        }
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(
                div()
                    .w(px(320.))
                    .max_w_full()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(theme.border))
                    .bg(rgb(theme.panel))
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(mundane_ui::heading("find-heading", "Find in terminal"))
                    .child(find.field.clone())
                    .when(find.missed, |el| {
                        el.child(
                            mundane_ui::Role::Status
                                .control("find-status", "No matching text")
                                .child("No matching text"),
                        )
                    })
                    .child(mundane_ui::unavailable(
                        button("Find match", &find.submit, theme, move |window, cx| {
                            let _ = view.update(cx, |this, cx| this.find_text(window, cx));
                        }),
                        find.field.read(cx).text().is_empty(),
                    )),
            )
    }
}

impl Render for Terminal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        let view = cx.weak_entity();
        let find = button("Find", &self.find_button, theme, move |window, cx| {
            let _ = view.update(cx, |this, cx| this.open_find(window, cx));
        });
        let v = cx.weak_entity();
        let profiles_view = cx.weak_entity();
        let window_view = cx.weak_entity();
        let tools = div()
            .flex()
            .gap_1()
            .child(button(
                "Profiles",
                &self.profile_focus,
                theme,
                move |w, cx| {
                    let _ = profiles_view.update(cx, |s, cx| s.edit_profiles(w, cx));
                },
            ))
            .child(button("New tab", &self.new_focus, theme, move |w, cx| {
                let _ = v.update(cx, |t, cx| t.add_tab(w, cx));
            }))
            .child(button(
                "New window",
                &self.window_focus,
                theme,
                move |w, cx| {
                    let _ = window_view.update(cx, |s, cx| s.separate(false, w, cx));
                },
            ))
            .child(find);
        let compact = f32::from(window.viewport_size().width) < 32. * f32::from(window.rem_size());
        let (header_tools, second_row) = if compact {
            (
                div(),
                Some(tools.flex_wrap().px_2().py_1().bg(rgb(theme.panel))),
            )
        } else {
            (tools, None)
        };
        let body = self
            .content
            .render(self.find.is_some() || self.form.is_some() || self.warning.is_some())
            .size_full()
            .flex()
            .flex_col()
            .font_family("DejaVu Sans")
            .bg(rgb(theme.surface))
            .text_color(rgb(theme.text))
            .child(
                self.header
                    .render("Terminal", header_tools, &self.grid_focus, theme, window),
            )
            .children(second_row)
            .when(self.tabs.len() > 1, |el| el.child(self.strip(cx)))
            .child(self.terminal_grid(window, cx))
            .children(self.notice.clone())
            .when(!self.composing.is_empty(), |el| {
                el.child(self.composing.clone())
            });
        div()
            .relative()
            .capture_key_down(cx.listener(Self::tab_key))
            .size_full()
            .font_family("DejaVu Sans")
            .text_color(rgb(theme.text))
            .child(body)
            .children(
                self.warning
                    .as_ref()
                    .map(|warn| self.warning_dialog(warn, cx)),
            )
            .children(self.form.as_ref().map(|form| self.profile_dialog(form, cx)))
            .children(self.find.as_ref().map(|find| self.dialog(find, cx)))
    }
}

// The shell owns its editable line. Only expose the local IME composition to the input method.
impl EntityInputHandler for Terminal {
    fn text_for_range(
        &mut self,
        r: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let units: Vec<_> = self.composing.encode_utf16().collect();
        let range = r.start.min(units.len())..r.end.min(units.len());
        *adjusted = Some(range.clone());
        Some(String::from_utf16_lossy(&units[range]))
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let end = self.composing.encode_utf16().count();
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        (!self.composing.is_empty()).then(|| 0..self.composing.encode_utf16().count())
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.composing.clear();
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.composing.clear();
        self.input(text.as_bytes().to_vec());
        cx.notify();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.composing = text.to_owned();
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let screen = self.tabs[self.active].session.screen.lock();
        let cursor = screen.grid().cursor.point;
        let cell = self.tabs[self.active].session.size();
        Some(Bounds::new(
            bounds.origin
                + point(
                    px(cursor.column.0 as f32 * cell.cell_width as f32),
                    px(cursor.line.0 as f32 * cell.cell_height as f32),
                ),
            size(px(cell.cell_width as f32), px(cell.cell_height as f32)),
        ))
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

fn main() {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("Terminal requires a Wayland session.");
        std::process::exit(1);
    }
    mundane_ui::run(|cx| {
        mundane_ui::bind_text_keys(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let result = cx.open_window(windows::options(cx), |window, cx| {
            window.set_window_title("Terminal");
            cx.new(|cx| {
                Terminal::new(Default::default(), window, cx).unwrap_or_else(|err| {
                    eprintln!("Could not open a shell: {err}");
                    std::process::exit(1);
                })
            })
        });
        if let Err(err) = result {
            eprintln!("Could not open Terminal: {err}");
            std::process::exit(1);
        }
    });
}

#[cfg(test)]
mod tests;
