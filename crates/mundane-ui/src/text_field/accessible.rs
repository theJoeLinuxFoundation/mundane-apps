//! Translate model byte offsets and shaped glyph coordinates into AccessKit text runs.
use super::*;
use accesskit::{
    Action, ActionData, Node, NodeId, Rect, TextDirection, TextPosition, TextSelection,
};
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
pub(super) struct Geometry {
    pub lines: Vec<(usize, ShapedLine, Bounds<Pixels>)>,
}
pub(super) type Layout = Rc<RefCell<Geometry>>;

// AccessKit stores byte lengths in u8. Split unusually long graphemes only on
// scalar boundaries. Selection requests inside one snap back to its leading edge.
fn boundaries(text: &str) -> Vec<usize> {
    let mut result = Vec::new();
    for (start, cluster) in text.grapheme_indices(true) {
        result.push(start);
        let mut chunk = 0;
        for (i, c) in cluster.char_indices() {
            if i + c.len_utf8() - chunk > u8::MAX as usize {
                result.push(start + i);
                chunk = i;
            }
        }
    }
    result.push(text.len());
    result
}

struct Run {
    start: usize,
    offsets: Vec<usize>,
    node: NodeId,
}
impl Run {
    fn position(&self, byte: usize) -> TextPosition {
        TextPosition {
            node: self.node,
            character_index: self
                .offsets
                .partition_point(|i| self.start + i <= byte)
                .saturating_sub(1),
        }
    }
}

impl TextField {
    pub(super) fn accessible(
        &mut self,
        control: gpui::Stateful<gpui::Div>,
        scale: f32,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let layout = self
            .accessible_layout
            .get_or_insert_with(Default::default)
            .clone();
        let text = self.content.clone();
        let password = self.password;
        let selection = self.selected_range.clone();
        let reversed = self.selection_reversed;
        let positions = Rc::new(RefCell::new(Vec::<Run>::new()));
        let action_positions = positions.clone();
        let entity = cx.weak_entity();
        let current = text.clone();
        let value = if password {
            "•".repeat(text.graphemes(true).count()).into()
        } else {
            text.clone()
        };
        control
            .aria_value(value)
            .on_a11y_action(Action::SetTextSelection, move |data, _, cx| {
                let Some(ActionData::SetTextSelection(s)) = data else {
                    return;
                };
                let runs = action_positions.borrow();
                let offset = |p: TextPosition| {
                    let run = runs.iter().find(|r| r.node == p.node)?;
                    Some(run.start + run.offsets.get(p.character_index)?)
                };
                if let (Some(anchor), Some(focus)) = (offset(s.anchor), offset(s.focus)) {
                    let _ = entity.update(cx, |field, cx| {
                        if field.content != current {
                            return;
                        }
                        let snap = |i| {
                            if i == current.len() {
                                i
                            } else {
                                current
                                    .grapheme_indices(true)
                                    .take_while(|(start, _)| *start <= i)
                                    .last()
                                    .map_or(0, |(start, _)| start)
                            }
                        };
                        field.move_to(snap(anchor), cx);
                        field.select_to(snap(focus), cx);
                    });
                }
            })
            .a11y_synthetic_children(move |tree| {
                tree.parent_node().set_clips_children();
                let mut runs = positions.borrow_mut();
                runs.clear();
                let layout = layout.borrow();
                let mut start = 0;
                for (row, line) in text.split('\n').enumerate() {
                    let end = (start + line.len() + 1).min(text.len());
                    let source = &text[start..end];
                    let offsets = if password {
                        source
                            .grapheme_indices(true)
                            .map(|(i, _)| i)
                            .chain(Some(source.len()))
                            .collect()
                    } else {
                        boundaries(source)
                    };
                    // Word starts, like character lengths, are u8 in AccessKit.
                    // Limit each run to 255 selectable positions without breaking a cluster.
                    let words: Vec<_> = if password {
                        Vec::new()
                    } else {
                        source
                            .unicode_word_indices()
                            .map(|(byte, _)| byte)
                            .collect()
                    };
                    let last_chunk = offsets.len().saturating_sub(2) / 255;
                    for chunk in 0..=last_chunk {
                        let first = chunk * 255;
                        let slice = &offsets[first..(first + 256).min(offsets.len())];
                        let byte = slice[0];
                        let end_byte = *slice.last().unwrap();
                        let value = if password {
                            "•".repeat(slice.len() - 1)
                        } else {
                            source[byte..end_byte].to_owned()
                        };
                        let mut node = Node::new(Role::TextRun);
                        node.set_character_lengths(
                            slice
                                .windows(2)
                                .map(|pair| {
                                    if password {
                                        3
                                    } else {
                                        (pair[1] - pair[0]) as u8
                                    }
                                })
                                .collect::<Vec<_>>(),
                        );
                        let word_range = words.partition_point(|&w| w < byte)
                            ..words.partition_point(|&w| w < end_byte);
                        node.set_word_starts(
                            words[word_range]
                                .iter()
                                .filter_map(|w| slice.binary_search(w).ok().map(|i| i as u8))
                                .collect::<Vec<_>>(),
                        );
                        if let Ok(i) = layout.lines.binary_search_by_key(&row, |l| l.0) {
                            let (_, shaped, bounds) = &layout.lines[i];
                            let xs: Vec<f32> = slice
                                .iter()
                                .enumerate()
                                .map(|(index, byte)| {
                                    f32::from(
                                        shaped.x_for_index(
                                            (if password { (first + index) * 3 } else { *byte })
                                                .min(shaped.len()),
                                        ),
                                    ) * scale
                                })
                                .collect();
                            let origin = xs[0];
                            node.set_character_widths(
                                xs.windows(2)
                                    .map(|p| (p[1] - p[0]).abs())
                                    .collect::<Vec<_>>(),
                            );
                            node.set_character_positions(
                                xs[..xs.len() - 1]
                                    .iter()
                                    .map(|x| x - origin)
                                    .collect::<Vec<_>>(),
                            );
                            node.set_text_direction(TextDirection::LeftToRight);
                            node.set_bounds(Rect {
                                x0: (f32::from(bounds.left()) * scale + origin) as f64,
                                y0: (f32::from(bounds.top()) * scale) as f64,
                                x1: (f32::from(bounds.left()) * scale + xs.last().unwrap()) as f64,
                                y1: (f32::from(bounds.bottom()) * scale) as f64,
                            });
                        }
                        if chunk > 0 {
                            node.set_previous_on_line(tree.synthetic_node_id((row, chunk - 1)));
                        }
                        if chunk < last_chunk {
                            node.set_next_on_line(tree.synthetic_node_id((row, chunk + 1)));
                        }
                        node.set_value(value);
                        let id = tree.synthetic_node_id((row, chunk));
                        tree.push_child(id, node);
                        runs.push(Run {
                            start: start + byte,
                            offsets: slice.iter().map(|i| i - byte).collect(),
                            node: id,
                        });
                    }
                    start = end;
                }
                let position =
                    |i| runs[runs.partition_point(|r| r.start <= i).saturating_sub(1)].position(i);
                let (a, b) = (position(selection.start), position(selection.end));
                tree.parent_node().set_text_selection(TextSelection {
                    anchor: if reversed { b } else { a },
                    focus: if reversed { a } else { b },
                });
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grapheme_offsets_cover_empty_unicode_crlf_and_long_clusters() {
        assert_eq!(boundaries(""), [0]);
        assert_eq!(boundaries("e\u{301}😀\r\n"), [0, 3, 7, 9]);
        let text = format!("e{}", "\u{301}".repeat(180));
        let offsets = boundaries(&text);
        assert_eq!(offsets.last(), Some(&text.len()));
        assert!(
            offsets
                .windows(2)
                .all(|p| p[1] - p[0] <= 255 && text.is_char_boundary(p[1]))
        );
    }
}
