// Adapted from Zed's crates/gpui/examples/input.rs at deb48ceb under Apache-2.0.
// See LICENSES/Apache-2.0.txt. Local changes integrate theme and focus roles,
// Linux editing shortcuts, horizontal scrolling and composition range handling.
use super::{Role, Theme, focus_ring};
use crate::ControlRole as _;
mod accessible;
mod multiline;
use multiline::Multiline;
use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ShapedLine,
    SharedString, Style, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div, fill,
    point, prelude::*, px, relative, rgb, size,
};

use unicode_segmentation::*;

actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
    ]
);

pub struct TextField {
    multi: Option<Multiline>,
    password: bool,
    accessible_layout: Option<accessible::Layout>,
    theme: Theme,
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
}

impl TextField {
    pub fn new(label: &'static str, theme: Theme, cx: &App) -> Self {
        Self {
            multi: None,
            password: false,
            accessible_layout: None,
            theme,
            focus_handle: cx.focus_handle().tab_stop(true),
            content: "".into(),
            placeholder: label.into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
        }
    }

    pub fn password(label: &'static str, theme: Theme, cx: &App) -> Self {
        Self {
            password: true,
            ..Self::new(label, theme, cx)
        }
    }

    fn display_offset(&self, byte: usize) -> usize {
        if self.password {
            self.content[..byte].graphemes(true).count() * "•".len()
        } else {
            byte
        }
    }

    fn model_offset(&self, byte: usize) -> usize {
        if self.password {
            self.content
                .grapheme_indices(true)
                .nth(byte / "•".len())
                .map_or(self.content.len(), |(i, _)| i)
        } else {
            byte
        }
    }

    /// Prefill a rename or location field so typing replaces the current value.
    pub fn select_value(mut self, value: String) -> Self {
        self.selected_range = 0..value.len();
        self.content = value.into();
        self
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let prev = self.previous_boundary(self.cursor_offset());
            if self.cursor_offset() == prev {
                window.play_system_bell();
                return;
            }
            self.select_to(prev, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            if self.cursor_offset() == next {
                window.play_system_bell();
                return;
            }
            self.select_to(next, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window, cx);
        self.is_selecting = true;

        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.password && !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }
    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.password && !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.reveal_caret();
        self.selection_reversed = false;
        self.selected_range = offset..offset;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if let Some(m) = &self.multi {
            return m.hit(position).unwrap_or(0);
        }
        if self.content.is_empty() {
            return 0;
        }

        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        self.model_offset(line.closest_index_for_x(position.x - bounds.left()))
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.reveal_caret();
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
    }

    fn reveal_caret(&mut self) {
        if let Some(m) = &mut self.multi {
            m.reveal = true;
            m.column = None;
        }
    }

    fn insertion(&self, text: &str) -> String {
        match &self.multi {
            None => text.replace(['\r', '\n'], " "),
            Some(m) => {
                let lf = text.replace("\r\n", "\n");
                if m.crlf { lf.replace('\n', "\r\n") } else { lf }
            }
        }
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }

        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }
}

impl EntityInputHandler for TextField {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        if self.password {
            return None;
        }
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.save_undo();
        let new_text = self.insertion(new_text);
        let new_text = new_text.as_str();
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selection_reversed = false;
        if let Some(m) = &mut self.multi {
            m.reindex(&self.content);
        }
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.save_undo();
        let new_text = self.insertion(new_text);
        let new_text = new_text.as_str();
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selection_reversed = false;
        if let Some(m) = &mut self.multi {
            m.reindex(&self.content);
        }
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        let prefix = self.offset_to_utf16(range.start);
        self.selected_range = new_selected_range_utf16
            .map(|r| {
                let units = new_text.encode_utf16().count();
                self.range_from_utf16(&(prefix + r.start.min(units)..prefix + r.end.min(units)))
            })
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        if let Some(m) = &self.multi {
            return m.ime_bounds(self.offset_from_utf16(range_utf16.start));
        }
        let bounds = self.last_bounds.unwrap_or(bounds);
        let last_layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(self.display_offset(range.start)),
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(self.display_offset(range.end)),
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        if let Some(m) = &self.multi {
            return m.hit(point).map(|offset| self.offset_to_utf16(offset));
        }
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;

        if !self.password && last_layout.text != self.content {
            return None;
        }
        let utf8_index = last_layout.index_for_x(line_point.x)?;
        Some(self.offset_to_utf16(self.model_offset(utf8_index)))
    }
}

struct TextElement {
    input: Entity<TextField>,
}

struct PrepaintState {
    text_bounds: Bounds<Pixels>,
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.display_offset(input.selected_range.start)
            ..input.display_offset(input.selected_range.end);
        let cursor = input.display_offset(input.cursor_offset());
        let style = window.text_style();

        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), rgb(input.theme.secondary).into())
        } else {
            (
                if input.password {
                    "•".repeat(content.graphemes(true).count()).into()
                } else {
                    content
                },
                style.color,
            )
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(mark) = input.marked_range.as_ref() {
            let marked_range = input.display_offset(mark.start)..input.display_offset(mark.end);
            vec![
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked_range.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        let cursor_pos = line.x_for_index(cursor);
        let shift = (cursor_pos + px(2.) - bounds.size.width).max(px(0.));
        let bounds = Bounds::new(point(bounds.left() - shift, bounds.top()), bounds.size);
        let (selection, cursor) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_pos, bounds.top()),
                        size(px(2.), bounds.bottom() - bounds.top()),
                    ),
                    rgb(input.theme.focus),
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(selected_range.start),
                            bounds.top(),
                        ),
                        point(
                            bounds.left() + line.x_for_index(selected_range.end),
                            bounds.bottom(),
                        ),
                    ),
                    rgb(input.theme.selection),
                )),
                None,
            )
        };
        if let Some(layout) = &input.accessible_layout {
            let mut geometry = layout.borrow_mut();
            geometry.lines.clear();
            geometry.lines.push((0, line.clone(), bounds));
        }
        PrepaintState {
            text_bounds: bounds,
            line: Some(line),
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }
        let line = prepaint.line.take().unwrap();
        line.paint(
            prepaint.text_bounds.origin,
            window.line_height(),
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        )
        .unwrap();

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(prepaint.text_bounds);
        });
    }
}

impl Render for TextField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let multi = self.multi.is_some();
        let role = if multi {
            Role::MultilineTextInput
        } else if self.password {
            Role::PasswordInput
        } else {
            Role::TextInput
        };
        let control = focus_ring(
            role.control(&self.focus_handle, self.placeholder.clone())
                .aria_placeholder(self.placeholder.clone()),
            &self.focus_handle,
            self.theme,
        );
        let control = if window.is_a11y_active() {
            self.accessible(control, window.scale_factor(), cx)
        } else {
            self.accessible_layout = None;
            control
        };
        control
            .w_full()
            .overflow_hidden()
            .rounded_md()
            .flex()
            .key_context(if multi {
                "TextField DocumentText"
            } else {
                "TextField SingleLine"
            })
            .text_color(rgb(self.theme.text))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .bg(rgb(self.theme.surface))
            .line_height(px(24.))
            .text_size(px(16. + self.multi.as_ref().map_or(0, |m| m.zoom) as f32))
            .when(multi, |el| {
                el.size_full()
                    .min_h_0()
                    .font_family("DejaVu Sans Mono")
                    .p_3()
                    .on_key_down(cx.listener(Self::document_keys))
                    .on_scroll(cx.listener(Self::scroll_document))
                    .child(self.paper(cx))
            })
            .when(!multi, |el| {
                el.child(
                    div()
                        .h(px(32.))
                        .w_full()
                        .p(px(4.))
                        .overflow_hidden()
                        .child(TextElement { input: cx.entity() }),
                )
            })
    }
}

impl Focusable for TextField {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

pub fn bind_text_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("TextField")),
        KeyBinding::new("delete", Delete, Some("TextField")),
        KeyBinding::new("left", Left, Some("TextField")),
        KeyBinding::new("right", Right, Some("TextField")),
        KeyBinding::new("shift-left", SelectLeft, Some("TextField")),
        KeyBinding::new("shift-right", SelectRight, Some("TextField")),
        KeyBinding::new("ctrl-a", SelectAll, Some("TextField")),
        KeyBinding::new("ctrl-v", Paste, Some("TextField")),
        KeyBinding::new("ctrl-c", Copy, Some("TextField")),
        KeyBinding::new("ctrl-x", Cut, Some("TextField")),
        KeyBinding::new("home", Home, Some("SingleLine")),
        KeyBinding::new("end", End, Some("SingleLine")),
    ]);
}
