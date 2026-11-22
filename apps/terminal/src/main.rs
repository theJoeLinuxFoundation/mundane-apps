mod grid;
mod keys;

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
use mundane_ui::{TextField, Theme, button, focus_ring, header};
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
    session: Session,
    grid_focus: FocusHandle,
    find_button: FocusHandle,
    find: Option<Find>,
    layout: Layout,
    theme: Theme,
    wheel_remainder: f32,
    composing: String,
    _events: Task<()>,
}

impl Terminal {
    fn new(options: Options, window: &mut Window, cx: &mut Context<Self>) -> anyhow::Result<Self> {
        let theme = if std::env::var_os("MUNDANE_HIGH_CONTRAST").as_deref() == Some("1".as_ref()) {
            Theme::HIGH_CONTRAST
        } else {
            Theme::DARK
        };
        let layout = Layout {
            bounds: Bounds::new(Point::default(), window.viewport_size()),
            font_size: BODY_FONT,
        };
        let (session, events) = Session::open(options, layout.size())?;
        let grid_focus = cx.focus_handle();
        grid_focus.focus(window, cx);
        let task = cx.spawn_in(window, async move |view, cx| {
            while let Ok(event) = events.recv().await {
                let result = view.update_in(cx, |terminal, window, cx| {
                    for event in std::iter::once(event)
                        .chain(std::iter::from_fn(|| events.try_recv().ok()).take(255))
                    {
                        match event {
                            Event::Exit => {
                                window.remove_window();
                                return;
                            }
                            Event::PtyWrite(s) => terminal.session.write(s.into_bytes()),
                            Event::TextAreaSizeRequest(reply) => terminal
                                .session
                                .write(reply(terminal.session.size()).into_bytes()),
                            _ => {}
                        }
                    }
                    cx.notify();
                });
                if result.is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            session,
            grid_focus,
            find_button: cx.focus_handle(),
            find: None,
            layout,
            theme,
            wheel_remainder: 0.,
            composing: String::new(),
            _events: task,
        })
    }

    fn input(&mut self, bytes: Vec<u8>) {
        let mut screen = self.session.screen.lock();
        screen.scroll_display(Scroll::Bottom);
        screen.selection = None;
        drop(screen);
        self.session.write(bytes);
    }

    fn paste(&mut self, text: String) {
        let bracketed = self
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
        let field = cx.new(|cx| TextField::new("Find text", self.theme, cx));
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
        let mut screen = self.session.screen.lock();
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
        match (key.key.as_str(), mods.control, mods.shift) {
            ("c", true, true) => {
                if let Some(text) = self.session.screen.lock().selection_to_string() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }
            ("v", true, true) => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.paste(text);
                }
            }
            ("f", true, true) => self.open_find(window, cx),
            ("pageup", false, true) => self.session.screen.lock().scroll_display(Scroll::PageUp),
            ("pagedown", false, true) => {
                self.session.screen.lock().scroll_display(Scroll::PageDown)
            }
            ("+" | "=", true, _) => self.layout.font_size = (self.layout.font_size + 2.).min(40.),
            ("-", true, _) => self.layout.font_size = (self.layout.font_size - 2.).max(10.),
            ("0", true, _) => self.layout.font_size = BODY_FONT,
            ("f6", false, _) => self.find_button.focus(window, cx),
            _ => {
                let app_cursor = self
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
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }

    fn terminal_grid(&self, cx: &Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let painter = view.clone();
        let surface = canvas(
            move |bounds, window, cx| {
                view.update(cx, |this, _| {
                    this.layout.bounds = bounds;
                    this.session.resize(this.layout.size());
                    Frame::read(
                        &this.session.screen.lock(),
                        this.layout,
                        this.theme,
                        this.grid_focus.is_focused(window),
                    )
                })
            },
            move |bounds, frame, window, cx| {
                frame.paint(window, cx);
                window.handle_input(
                    &painter.read(cx).grid_focus,
                    ElementInputHandler::new(bounds, painter.clone()),
                    cx,
                );
            },
        )
        .size_full();

        focus_ring(div().id("screen"), &self.grid_focus, self.theme)
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
                    let mut screen = this.session.screen.lock();
                    let (point, side) = this.layout.hit(ev.position, &screen);
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
                    let mut screen = this.session.screen.lock();
                    let (point, side) = this.layout.hit(ev.position, &screen);
                    if let Some(selection) = &mut screen.selection {
                        selection.update(point, side);
                    }
                    cx.notify();
                }
            }))
            .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _, cx| {
                let delta = match ev.delta {
                    ScrollDelta::Pixels(p) => f32::from(p.y) / this.layout.height(),
                    ScrollDelta::Lines(p) => p.y,
                };
                this.wheel_remainder += delta;
                let rows = this.wheel_remainder.trunc() as i32;
                this.wheel_remainder -= rows as f32;
                this.session
                    .screen
                    .lock()
                    .scroll_display(Scroll::Delta(rows));
                cx.notify();
            }))
            .child(surface)
    }

    fn dialog(&self, find: &Find, cx: &Context<Self>) -> impl IntoElement {
        let view = cx.weak_entity();
        let field = find.field.clone();
        let submit = find.submit.clone();
        div()
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
                    .border_color(rgb(self.theme.border))
                    .bg(rgb(self.theme.panel))
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child("Find in terminal")
                    .child(find.field.clone())
                    .when(find.missed, |el| el.child("No matching text"))
                    .child(button(
                        "Find match",
                        &find.submit,
                        self.theme,
                        move |window, cx| {
                            let _ = view.update(cx, |this, cx| this.find_text(window, cx));
                        },
                    )),
            )
    }
}

impl Render for Terminal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.weak_entity();
        let find = button("Find", &self.find_button, self.theme, move |window, cx| {
            let _ = view.update(cx, |this, cx| this.open_find(window, cx));
        })
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
            if matches!(event.keystroke.key.as_str(), "escape" | "f6" | "tab") {
                this.grid_focus.focus(window, cx);
                cx.stop_propagation();
            }
        }));
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .font_family("DejaVu Sans")
            .bg(rgb(self.theme.surface))
            .text_color(rgb(self.theme.text))
            .child(header("Terminal", self.theme).child(find))
            .child(self.terminal_grid(cx))
            .when(!self.composing.is_empty(), |el| {
                el.child(self.composing.clone())
            })
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
        let screen = self.session.screen.lock();
        let cursor = screen.grid().cursor.point;
        let cell = self.session.size();
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
    Application::with_platform(gpui_linux::current_platform(false)).run(|cx| {
        mundane_ui::bind_text_keys(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(820.), px(520.)), cx);
        let result = cx.open_window(
            WindowOptions {
                app_id: Some(APP_ID.into()),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_decorations: Some(WindowDecorations::Server),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Terminal");
                cx.new(|cx| {
                    Terminal::new(Default::default(), window, cx).unwrap_or_else(|err| {
                        eprintln!("Could not open a shell: {err}");
                        std::process::exit(1);
                    })
                })
            },
        );
        if let Err(err) = result {
            eprintln!("Could not open Terminal: {err}");
            std::process::exit(1);
        }
    });
}
