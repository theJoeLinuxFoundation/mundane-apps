use alacritty_terminal::{
    event::{EventListener, WindowSize},
    grid::Dimensions,
    index::{Column, Line, Point as CellPoint, Side},
    term::{Term, cell::Flags, color::Colors},
    vte::ansi::{Color, CursorShape, NamedColor},
};
use gpui::{
    App, Bounds, FontStyle, FontWeight, Pixels, Point, StrikethroughStyle, TextAlign, TextRun,
    UnderlineStyle, Window, fill, font, point, px, rgb, size,
};
use mundane_ui::Theme;

#[derive(Clone, Copy)]
pub struct Layout {
    pub bounds: Bounds<Pixels>,
    pub font_size: f32,
}

impl Layout {
    fn width(self) -> f32 {
        (self.font_size * 0.605).ceil()
    }
    pub fn height(self) -> f32 {
        (self.font_size * 1.45).ceil()
    }
    pub fn size(self) -> WindowSize {
        WindowSize {
            num_cols: ((f32::from(self.bounds.size.width) / self.width()) as u16).max(2),
            num_lines: ((f32::from(self.bounds.size.height) / self.height()) as u16).max(1),
            cell_width: self.width() as u16,
            cell_height: self.height() as u16,
        }
    }
    pub fn hit<T>(self, position: Point<Pixels>, term: &Term<T>) -> (CellPoint, Side) {
        let col = (f32::from(position.x - self.bounds.left()) / self.width()).max(0.);
        let row = (f32::from(position.y - self.bounds.top()) / self.height()) as usize;
        let line = row.min(term.screen_lines() - 1) as i32 - term.grid().display_offset() as i32;
        (
            CellPoint::new(Line(line), Column((col as usize).min(term.columns() - 1))),
            if col.fract() < 0.5 {
                Side::Left
            } else {
                Side::Right
            },
        )
    }
}

const PALETTE: [u32; 16] = [
    0x26322e, 0xe78d87, 0x9ecd9d, 0xe5c083, 0x8bb7e3, 0xc6a2de, 0x86c8c6, 0xd6ded8, 0x899990,
    0xffa6a1, 0xb2edb0, 0xffdb97, 0xaacfff, 0xe9c2fb, 0xace5df, 0xffffff,
];

fn resolve(color: Color, theme: Theme, colors: &Colors) -> u32 {
    let index = match color {
        Color::Spec(rgb) => return u32::from_be_bytes([0, rgb.r, rgb.g, rgb.b]),
        Color::Named(n) => n as usize,
        Color::Indexed(n) => n as usize,
    };
    palette_color(index, theme, colors)
}

pub fn palette_color(index: usize, theme: Theme, colors: &Colors) -> u32 {
    if let Some(rgb) = colors[index] {
        return u32::from_be_bytes([0, rgb.r, rgb.g, rgb.b]);
    }
    match index {
        0..16 => PALETTE[index],
        16..232 => {
            let n = index - 16;
            let component = |v| if v == 0 { 0 } else { 55 + v * 40 };
            (component(n / 36) << 16 | component(n / 6 % 6) << 8 | component(n % 6)) as u32
        }
        232..256 => ((8 + (index - 232) * 10) * 0x010101) as u32,
        n if n == NamedColor::Background as usize => theme.surface,
        n if (NamedColor::DimBlack as usize..=NamedColor::DimWhite as usize).contains(&n) => {
            PALETTE[n - NamedColor::DimBlack as usize]
        }
        _ => theme.text,
    }
}

struct Glyph {
    text: String,
    flags: Flags,
    origin: Point<Pixels>,
    bg: u32,
    fg: u32,
    width: Pixels,
}

pub struct Frame {
    glyphs: Vec<Glyph>,
    layout: Layout,
    theme: Theme,
}

impl Frame {
    pub fn read<T: EventListener>(
        term: &Term<T>,
        layout: Layout,
        theme: Theme,
        focused: bool,
    ) -> Self {
        let content = term.renderable_content();
        let mut glyphs = Vec::new();
        for item in content.display_iter {
            let cell = item.cell;
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let selected = content.selection.is_some_and(|range| {
                range.contains_cell(&item, content.cursor.point, content.cursor.shape)
            });
            let cursor = focused
                && content.cursor.point == item.point
                && content.cursor.shape != CursorShape::Hidden;
            let mut bg = resolve(cell.bg, theme, content.colors);
            let mut fg = resolve(cell.fg, theme, content.colors);
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut bg, &mut fg);
            }
            if selected {
                bg = theme.selection;
                fg = theme.text;
            }
            if cursor {
                bg = theme.focus;
                fg = theme.surface;
            }
            if cell.c == ' '
                && bg == theme.surface
                && !cell
                    .flags
                    .intersects(Flags::ALL_UNDERLINES | Flags::STRIKEOUT)
            {
                continue;
            }
            let row = item.point.line.0 + content.display_offset as i32;
            let origin = layout.bounds.origin
                + point(
                    px(item.point.column.0 as f32 * layout.width()),
                    px(row as f32 * layout.height()),
                );
            let mut text = cell.c.to_string();
            if let Some(extra) = cell.zerowidth() {
                text.extend(extra);
            }
            let width = layout.width()
                * if cell.flags.contains(Flags::WIDE_CHAR) {
                    2.
                } else {
                    1.
                };
            glyphs.push(Glyph {
                text,
                flags: cell.flags,
                origin,
                fg,
                bg,
                width: px(width),
            });
        }
        Self {
            glyphs,
            layout,
            theme,
        }
    }

    pub fn paint(self, window: &mut Window, cx: &mut App) {
        let height = px(self.layout.height());
        for glyph in self.glyphs {
            if glyph.bg != self.theme.surface {
                window.paint_quad(fill(
                    Bounds::new(glyph.origin, size(glyph.width, height)),
                    rgb(glyph.bg),
                ));
            }
            if glyph.flags.contains(Flags::HIDDEN) {
                continue;
            }
            let mut face = font("DejaVu Sans Mono");
            if glyph.flags.contains(Flags::BOLD) {
                face.weight = FontWeight::BOLD;
            }
            if glyph.flags.contains(Flags::ITALIC) {
                face.style = FontStyle::Italic;
            }
            let run = TextRun {
                len: glyph.text.len(),
                font: face,
                color: rgb(glyph.fg).into(),
                background_color: None,
                underline: glyph
                    .flags
                    .intersects(Flags::ALL_UNDERLINES)
                    .then(UnderlineStyle::default),
                strikethrough: glyph
                    .flags
                    .contains(Flags::STRIKEOUT)
                    .then(StrikethroughStyle::default),
            };
            let line = window.text_system().shape_line(
                glyph.text.into(),
                px(self.layout.font_size),
                &[run],
                None,
            );
            let _ = line.paint(glyph.origin, height, TextAlign::Left, None, window, cx);
        }
    }
}

/// Publish the same visible cells and scroll origin used by the grid painter.
pub fn accessible<T: EventListener>(
    tree: &mut gpui::A11ySubtreeBuilder,
    term: &Term<T>,
    layout: Layout,
    scale: f32,
) {
    use accesskit::{Node, Role, TextPosition, TextSelection};
    let offset = term.grid().display_offset() as i32;
    let cursor = term.grid().cursor.point;
    for row in 0..term.screen_lines() {
        let line = Line(row as i32 - offset);
        let id = tree.synthetic_node_id(line.0);
        let mut node = Node::new(Role::TextRun);
        let mut text = String::new();
        let mut lengths = Vec::new();
        let mut positions = Vec::new();
        let mut widths = Vec::new();
        let mut caret = 0;
        for col in 0..term.columns() {
            let cell = &term.grid()[line][Column(col)];
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            if col <= cursor.column.0 {
                caret = lengths.len();
            }
            let start = text.len();
            text.push(cell.c);
            if let Some(marks) = cell.zerowidth() {
                text.extend(marks);
            }
            // The AT-SPI offsets are Unicode scalar offsets. Keep oversized combining
            // sequences representable and give marks the same cell origin.
            for (index, c) in text[start..].chars().enumerate() {
                lengths.push(c.len_utf8() as u8);
                positions.push(col as f32 * layout.width() * scale);
                widths.push(if index == 0 {
                    layout.width()
                        * scale
                        * if cell.flags.contains(Flags::WIDE_CHAR) {
                            2.
                        } else {
                            1.
                        }
                } else {
                    0.
                });
            }
        }
        if row + 1 < term.screen_lines() {
            text.push('\n');
            lengths.push(1);
            positions.push(term.columns() as f32 * layout.width() * scale);
            widths.push(0.);
        }
        let x = (f32::from(layout.bounds.left()) * scale) as f64;
        let y = ((f32::from(layout.bounds.top()) + row as f32 * layout.height()) * scale) as f64;
        node.set_bounds(accesskit::Rect {
            x0: x,
            y0: y,
            x1: x + (term.columns() as f32 * layout.width() * scale) as f64,
            y1: y + (layout.height() * scale) as f64,
        });
        node.set_value(text);
        node.set_character_lengths(lengths);
        node.set_character_positions(positions);
        node.set_character_widths(widths);
        node.set_text_direction(accesskit::TextDirection::LeftToRight);
        tree.push_child(id, node);
        if line == cursor.line {
            let pos = TextPosition {
                node: id,
                character_index: caret,
            };
            tree.parent_node().set_text_selection(TextSelection {
                anchor: pos,
                focus: pos,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::{event::VoidListener, vte::ansi::Processor};
    use mundane_services::terminal::GridSize;

    #[test]
    fn split_utf8_sgr_and_cursor_motion_preserve_cell_positions() {
        let mut term = Term::new(
            Default::default(),
            &GridSize { cols: 12, rows: 2 },
            VoidListener,
        );
        let mut parser = Processor::<alacritty_terminal::vte::ansi::StdSyncHandler>::new();
        for &byte in "ab\x1b[31m界\x1b[0m!\rZ".as_bytes() {
            parser.advance(&mut term, &[byte]);
        }
        let row = &term.grid()[Line(0)];
        assert_eq!(row[Column(0)].c, 'Z');
        assert_eq!(row[Column(2)].c, '界');
        assert_eq!(row[Column(2)].fg, Color::Named(NamedColor::Red));
        assert!(row[Column(3)].flags.contains(Flags::WIDE_CHAR_SPACER));
        assert_eq!(row[Column(4)].c, '!');
        parser.advance(&mut term, b"\r\nsecond\r\nthird");
        term.scroll_display(alacritty_terminal::grid::Scroll::Top);
        assert_eq!(term.grid().display_offset(), 1);
        assert_eq!(term.grid()[Line(-1)][Column(2)].c, '界');
    }
}
