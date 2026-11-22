use super::*;
use gpui::{KeyDownEvent, ScrollEvent, TextAlign, canvas};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxRole {
    Keyword,
    String,
    Comment,
    Literal,
    Name,
}
impl SyntaxRole {
    fn color(self, t: Theme) -> u32 {
        if t.high_contrast {
            return t.text;
        }
        match (self, t.light) {
            (Self::Keyword, true) => 0x733684,
            (Self::Keyword, false) => 0xe7a6ed,
            (Self::String, true) => 0x27612d,
            (Self::String, false) => 0xa5dfaa,
            (Self::Comment, _) => t.secondary,
            (Self::Literal, true) => 0x824514,
            (Self::Literal, false) => 0xf2be87,
            (Self::Name, true) => 0x1d547d,
            (Self::Name, false) => 0x9dceff,
        }
    }
}

struct Snapshot {
    text: SharedString,
    selection: Range<usize>,
    reversed: bool,
}

#[derive(Default)]
pub(super) struct Multiline {
    starts: Vec<usize>,
    syntax: Vec<(Range<usize>, SyntaxRole)>,
    numbered: bool,
    pub(super) zoom: i32,
    top: usize,
    left: Pixels,
    scroll_remainder: f32,
    pub(super) reveal: bool,
    pub(super) column: Option<usize>,
    bounds: Bounds<Pixels>,
    lines: Vec<ShapedLine>,
    pub(super) crlf: bool,
    undo: VecDeque<Snapshot>,
    redo: Vec<Snapshot>,
}

impl Multiline {
    pub(super) fn reindex(&mut self, text: &str) {
        self.lines.clear();
        self.syntax.clear();
        self.starts.clear();
        self.starts.push(0);
        self.starts
            .extend(text.match_indices('\n').map(|(i, _)| i + 1));
        self.reveal = true;
        self.column = None;
    }

    fn row(&self, offset: usize) -> usize {
        self.starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1)
    }

    fn range(&self, row: usize, text: &str) -> Range<usize> {
        let start = self.starts[row];
        let end = self.starts.get(row + 1).map_or(text.len(), |next| {
            let lf = next - 1;
            if text[start..lf].ends_with('\r') {
                lf - 1
            } else {
                lf
            }
        });
        start..end
    }

    fn row_height(&self) -> f32 {
        25. + self.zoom as f32 * 1.5
    }

    fn page(&self) -> usize {
        (f32::from(self.bounds.size.height) / self.row_height())
            .floor()
            .max(1.) as usize
    }

    pub(super) fn hit(&self, position: Point<Pixels>) -> Option<usize> {
        let index = ((f32::from(position.y - self.bounds.top()) / self.row_height())
            .floor()
            .max(0.) as usize)
            .min(self.lines.len().checked_sub(1)?);
        Some(
            self.starts[self.top + index]
                + self.lines[index]
                    .closest_index_for_x(position.x - self.bounds.left() + self.left),
        )
    }

    pub(super) fn ime_bounds(&self, offset: usize) -> Option<Bounds<Pixels>> {
        let row = self.row(offset);
        let visible = row.checked_sub(self.top)?;
        let line = self.lines.get(visible)?;
        Some(Bounds::new(
            self.bounds.origin
                + point(
                    line.x_for_index((offset - self.starts[row]).min(line.len())) - self.left,
                    px(self.row_height() * visible as f32),
                ),
            size(px(2.), px(self.row_height())),
        ))
    }
}

impl TextField {
    pub fn multiline(label: &'static str, theme: Theme, cx: &App) -> Self {
        let mut field = Self::new(label, theme, cx);
        let mut layout = Multiline::default();
        layout.reindex("");
        field.multi = Some(layout);
        field
    }

    /// Show one-based logical line numbers alongside a multiline document.
    pub fn with_line_numbers(mut self) -> Self {
        if let Some(layout) = &mut self.multi {
            layout.numbered = true;
        }
        self
    }

    pub fn line_count(&self) -> usize {
        self.multi.as_ref().map_or(1, |m| m.starts.len())
    }

    pub fn go_to_line(&mut self, line: usize, cx: &mut Context<Self>) -> bool {
        let Some(&offset) = line
            .checked_sub(1)
            .and_then(|i| self.multi.as_ref()?.starts.get(i))
        else {
            return false;
        };
        self.move_to(offset, cx);
        true
    }

    pub fn set_document(&mut self, text: SharedString, cx: &mut Context<Self>) {
        if self.password {
            self.content = self.insertion(&text).into();
        } else {
            let mut layout = Multiline {
                crlf: text.find('\n').is_some_and(|i| text[..i].ends_with('\r')),
                numbered: self.multi.as_ref().is_some_and(|m| m.numbered),
                zoom: self.multi.as_ref().map_or(0, |m| m.zoom),
                ..Default::default()
            };
            layout.reindex(&text);
            self.multi = Some(layout);
            self.content = text;
        }
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
        cx.notify();
    }

    pub fn set_highlights(
        &mut self,
        spans: Vec<(Range<usize>, SyntaxRole)>,
        cx: &mut Context<Self>,
    ) {
        if let Some(layout) = &mut self.multi {
            layout.syntax = spans;
            cx.notify();
        }
    }

    /// Apply a literal replace-all as one undo step, retaining untouched line endings.
    pub fn replace_all(&mut self, query: &str, replacement: &str, cx: &mut Context<Self>) -> usize {
        if query.is_empty() {
            return 0;
        }
        let count = self.content.matches(query).count();
        if count > 0 {
            self.save_undo();
            let replacement = self.insertion(replacement);
            let end = self.content.find(query).unwrap() + replacement.len();
            self.content = self.content.replace(query, &replacement).into();
            self.marked_range = None;
            if let Some(m) = &mut self.multi {
                m.reindex(&self.content);
            }
            self.move_to(end, cx);
        }
        count
    }

    pub fn snapshot(&self) -> SharedString {
        self.content.clone()
    }

    pub fn selection(&self) -> Range<usize> {
        self.selected_range.clone()
    }

    pub fn select_match(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        self.move_to(range.start, cx);
        self.select_to(range.end, cx);
    }

    pub fn line_column(&self) -> (usize, usize) {
        let offset = self.cursor_offset();
        let (row, start) = self.multi.as_ref().map_or((0, 0), |m| {
            let row = m.row(offset);
            (row, m.starts[row])
        });
        (
            row + 1,
            self.content[start..offset].graphemes(true).count() + 1,
        )
    }

    pub fn line_ending(&self) -> &'static str {
        if self.multi.as_ref().is_some_and(|m| m.crlf) {
            "CRLF"
        } else {
            "LF"
        }
    }

    pub(super) fn save_undo(&mut self) {
        if self.marked_range.is_some() {
            return;
        }
        if let Some(m) = &mut self.multi {
            m.redo.clear();
            m.undo.push_back(Snapshot {
                text: self.content.clone(),
                selection: self.selected_range.clone(),
                reversed: self.selection_reversed,
            });
            let mut bytes: usize = m.undo.iter().map(|s| s.text.len()).sum();
            while m.undo.len() > 1 && (m.undo.len() > 100 || bytes > 16 * 1024 * 1024) {
                bytes -= m.undo.pop_front().unwrap().text.len();
            }
        }
    }

    fn history(&mut self, redo: bool, cx: &mut Context<Self>) {
        let m = self.multi.as_mut().unwrap();
        let old = if redo {
            m.redo.pop()
        } else {
            m.undo.pop_back()
        };
        if let Some(old) = old {
            let current = Snapshot {
                text: std::mem::replace(&mut self.content, old.text),
                selection: std::mem::replace(&mut self.selected_range, old.selection),
                reversed: std::mem::replace(&mut self.selection_reversed, old.reversed),
            };
            if redo {
                m.undo.push_back(current);
            } else {
                m.redo.push(current);
            }
            self.marked_range = None;
            m.reindex(&self.content);
            cx.notify();
        }
    }

    pub(super) fn document_keys(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = &event.keystroke;
        if key.modifiers.control && matches!(key.key.as_str(), "=" | "+" | "-" | "0") {
            let m = self.multi.as_mut().unwrap();
            m.zoom = match key.key.as_str() {
                "-" => (m.zoom - 2).max(-4),
                "0" => 0,
                _ => (m.zoom + 2).min(16),
            };
            m.reveal = true;
            window.prevent_default();
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let offset = self.cursor_offset();
        let m = self.multi.as_ref().unwrap();
        let row = m.row(offset);
        let range = m.range(row, &self.content);
        let ctrl = key.modifiers.control;
        let shift = key.modifiers.shift;
        let mut preferred = None;
        let next = match key.key.as_str() {
            "z" if ctrl => {
                self.history(shift, cx);
                None
            }
            "y" if ctrl => {
                self.history(true, cx);
                None
            }
            "tab" if !ctrl && !shift => {
                self.replace_text_in_range(None, "\t", window, cx);
                None
            }
            "enter" if !ctrl => {
                self.replace_text_in_range(None, "\n", window, cx);
                None
            }
            "home" => Some(if ctrl { 0 } else { range.start }),
            "end" => Some(if ctrl { self.content.len() } else { range.end }),
            direction @ ("up" | "down" | "pageup" | "pagedown") => {
                let count = if direction.starts_with("page") {
                    m.page()
                } else {
                    1
                };
                let dest = if direction.ends_with("up") {
                    row.saturating_sub(count)
                } else {
                    (row + count).min(m.starts.len() - 1)
                };
                let column = m
                    .column
                    .unwrap_or_else(|| self.content[range.start..offset].graphemes(true).count());
                let target = m.range(dest, &self.content);
                preferred = Some(column);
                Some(
                    self.content[target.clone()]
                        .grapheme_indices(true)
                        .nth(column)
                        .map_or(target.end, |(i, _)| target.start + i),
                )
            }
            _ => return,
        };
        if let Some(next) = next {
            if shift {
                self.select_to(next, cx);
            } else {
                self.move_to(next, cx);
            }
            self.multi.as_mut().unwrap().column = preferred;
        }
        window.prevent_default();
        cx.stop_propagation();
    }

    pub(super) fn scroll_document(
        &mut self,
        event: &ScrollEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let m = self.multi.as_mut().unwrap();
        let delta = event.delta.pixel_delta(px(m.row_height()));
        m.scroll_remainder -= f32::from(delta.y) / m.row_height();
        let rows = m.scroll_remainder.trunc();
        m.scroll_remainder -= rows;
        m.top = m
            .top
            .saturating_add_signed(rows as isize)
            .min(m.starts.len().saturating_sub(m.page()));
        m.left = (m.left - delta.x).max(px(0.));
        m.reveal = false;
        cx.notify();
    }

    pub(super) fn paper(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let entity = cx.entity();
        let prepare = entity.clone();
        canvas(
            move |bounds, window, cx| {
                prepare.update(cx, |input, _| input.layout_document(bounds, window))
            },
            move |bounds, (), window, cx| {
                let input = entity.read(cx);
                window.handle_input(
                    &input.focus_handle,
                    ElementInputHandler::new(bounds, entity.clone()),
                    cx,
                );
                entity.update(cx, |input, cx| {
                    let m = input.multi.as_ref().unwrap();
                    if m.numbered {
                        input.paint_numbers(bounds, window, cx);
                    }
                    window.with_content_mask(
                        Some(gpui::ContentMask { bounds: m.bounds }),
                        |window| input.paint_document(m.bounds, window, cx),
                    );
                });
            },
        )
        .size_full()
    }

    fn layout_document(&mut self, mut bounds: Bounds<Pixels>, window: &Window) {
        let cursor = self.cursor_offset();
        let m = self.multi.as_mut().unwrap();
        if m.numbered {
            let digits = m.starts.len().ilog10() + 1;
            let width = px(14. + digits as f32 * (m.row_height() * 0.55)).min(bounds.size.width);
            bounds.origin.x += width;
            bounds.size.width -= width;
        }
        m.bounds = bounds;
        let caret_row = m.row(cursor);
        let page = m.page();
        m.top = m.top.min(m.starts.len().saturating_sub(page));
        if m.reveal {
            m.top = m.top.clamp(caret_row.saturating_sub(page - 1), caret_row);
        }
        m.lines.clear();
        let style = window.text_style();
        for row in m.top..(m.top + page + 1).min(m.starts.len()) {
            let range = m.range(row, &self.content);
            let text: SharedString = self.content[range.clone()].to_owned().into();
            let run = TextRun {
                len: text.len(),
                font: style.font(),
                color: rgb(self.theme.text).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let mut runs = Vec::new();
            let mut cursor = range.start;
            let first = m
                .syntax
                .partition_point(|(span, _)| span.end <= range.start);
            for (span, role) in m.syntax[first..]
                .iter()
                .take_while(|(span, _)| span.start < range.end)
            {
                let start = span.start.max(range.start);
                let end = span.end.min(range.end);
                if cursor < start {
                    runs.push(TextRun {
                        len: start - cursor,
                        ..run.clone()
                    });
                }
                runs.push(TextRun {
                    len: end - start,
                    color: rgb(role.color(self.theme)).into(),
                    ..run.clone()
                });
                cursor = end;
            }
            if cursor < range.end || runs.is_empty() {
                runs.push(TextRun {
                    len: range.end - cursor,
                    ..run
                });
            }
            let line = window.text_system().shape_line(
                text,
                style.font_size.to_pixels(window.rem_size()),
                &runs,
                None,
            );
            if row == caret_row && m.reveal {
                let x = line.x_for_index((cursor - range.start).min(line.len()));
                m.left = m
                    .left
                    .clamp((x - bounds.size.width + px(2.)).max(px(0.)), x.max(px(2.)));
            }
            m.lines.push(line);
        }
        m.reveal = false;
        if let Some(layout) = &self.accessible_layout {
            let mut geometry = layout.borrow_mut();
            geometry.lines.clear();
            geometry
                .lines
                .extend(m.lines.iter().enumerate().map(|(i, line)| {
                    let origin = bounds.origin + point(-m.left, px(i as f32 * m.row_height()));
                    (
                        m.top + i,
                        line.clone(),
                        Bounds::new(origin, size(line.width, px(m.row_height()))),
                    )
                }));
        }
    }

    fn paint_numbers(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let m = self.multi.as_ref().unwrap();
        let style = window.text_style();
        let size = style.font_size.to_pixels(window.rem_size());
        for i in 0..m.lines.len() {
            let number: SharedString = (m.top + i + 1).to_string().into();
            let run = TextRun {
                len: number.len(),
                font: style.font(),
                color: rgb(self.theme.secondary).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let line = window.text_system().shape_line(number, size, &[run], None);
            let origin = point(
                m.bounds.left() - px(10.) - line.width,
                bounds.top() + px(i as f32 * m.row_height()),
            );
            let _ = line.paint(
                origin,
                px(m.row_height()),
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
    }

    fn paint_document(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let cursor = self.cursor_offset();
        let m = self.multi.as_ref().unwrap();
        let caret_row = m.row(cursor);
        for (i, line) in m.lines.iter().enumerate() {
            let row = m.top + i;
            let start = m.starts[row];
            let origin = bounds.origin + point(-m.left, px(i as f32 * m.row_height()));
            let x = |offset: usize| {
                origin.x + line.x_for_index(offset.saturating_sub(start).min(line.len()))
            };
            let range = &self.selected_range;
            if range.start < m.starts.get(row + 1).copied().unwrap_or(self.content.len())
                && range.end > start
                && !range.is_empty()
            {
                let right = if range.end > start + line.len() {
                    (x(range.end) + px(6.)).min(bounds.right())
                } else {
                    x(range.end)
                };
                window.paint_quad(fill(
                    Bounds::from_corners(
                        point(x(range.start), origin.y),
                        point(right, origin.y + px(m.row_height())),
                    ),
                    rgb(self.theme.selection),
                ));
            }
            let _ = line.paint(
                origin,
                px(m.row_height()),
                TextAlign::Left,
                None,
                window,
                cx,
            );
            if let Some(mark) = &self.marked_range
                && mark.start < start + line.len()
                && mark.end > start
            {
                window.paint_quad(fill(
                    Bounds::from_corners(
                        point(x(mark.start), origin.y + px(m.row_height() - 2.)),
                        point(x(mark.end), origin.y + px(m.row_height() - 1.)),
                    ),
                    rgb(self.theme.text),
                ));
            }
            if row == caret_row && range.is_empty() && self.focus_handle.is_focused(window) {
                window.paint_quad(fill(
                    Bounds::new(point(x(cursor), origin.y), size(px(2.), px(m.row_height()))),
                    rgb(self.theme.focus),
                ));
            }
        }
    }
}
