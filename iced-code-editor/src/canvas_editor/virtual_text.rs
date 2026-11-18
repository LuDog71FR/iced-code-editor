//! Text drawn inside the editor that is not part of the buffer, such as LSP
//! inlay hints (`let total: u32`, `add(left: 1)`).
//!
//! Virtual text pushes the characters after it to the right, but it is never
//! in the buffer: the cursor steps over it, it is never copied and it never
//! reaches [`CodeEditor::content`] or the language server.

use std::collections::{BTreeMap, HashMap};

use iced::Color;

use crate::canvas_editor::range_style::apply_range_styles;
use crate::canvas_editor::render::text::calculate_segment_geometry;
use crate::canvas_editor::render::wrapping::{VisualLine, WrappingCalculator};
use crate::canvas_editor::{CodeEditor, measure_text_width};

/// One piece of virtual text, see [`CodeEditor::set_virtual_text`].
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualText {
    /// Logical line (0-based).
    pub line: usize,
    /// Character column the text is drawn before; the line length (or more)
    /// draws it after the last character.
    pub column: usize,
    /// What is drawn (one line; the owner replaces line breaks).
    pub text: String,
    /// Text color. Nothing is drawn behind it.
    pub color: Color,
}

/// One run of a line as it is displayed, in drawing order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LineRun<'a> {
    /// The buffer characters of the columns `start..end`, in one color.
    Text {
        /// First character column of the run.
        start: usize,
        /// Character column after the run (exclusive).
        end: usize,
        /// Color of the characters.
        color: Color,
    },
    /// One piece of virtual text.
    Virtual(&'a VirtualText),
}

/// Places `virtual_text` (sorted by column) between the colored `pieces` of
/// the columns `cols` of a line.
///
/// Virtual text goes before the character at its column. The one at the end
/// of the line goes after the last piece, on the segment reaching that end.
fn merge_virtual_text<'a>(
    pieces: Vec<(usize, usize, Color)>,
    virtual_text: &'a [VirtualText],
    line_len: usize,
    cols: (usize, usize),
) -> Vec<LineRun<'a>> {
    let column = |item: &VirtualText| item.column.min(line_len);
    let mut next = virtual_text.partition_point(|item| column(item) < cols.0);
    let mut runs = Vec::with_capacity(pieces.len());
    let push_text = |runs: &mut Vec<LineRun<'a>>, start, end, color| {
        if start < end {
            runs.push(LineRun::Text { start, end, color });
        }
    };
    for (start, end, color) in pieces {
        let mut pos = start;
        while let Some(item) = virtual_text.get(next)
            && column(item) < end
        {
            let at = column(item).max(pos);
            push_text(&mut runs, pos, at, color);
            runs.push(LineRun::Virtual(item));
            pos = at;
            next += 1;
        }
        push_text(&mut runs, pos, end, color);
    }
    if cols.1 >= line_len {
        runs.extend(virtual_text[next..].iter().map(LineRun::Virtual));
    }
    runs
}

/// The virtual text of every layer, and the same items by line for drawing.
#[derive(Debug, Default)]
pub(crate) struct VirtualTextLayers {
    layers: BTreeMap<String, Vec<VirtualText>>,
    /// Every item, by line, sorted by column (a stable sort: on the same
    /// column, layers in name order and items in the order they were given).
    by_line: HashMap<usize, Vec<VirtualText>>,
    /// Bumped whenever the displayed text changes, for the layout caches.
    pub(crate) revision: u64,
}

impl VirtualTextLayers {
    fn rebuild(&mut self) {
        self.by_line.clear();
        for item in self.layers.values().flatten() {
            self.by_line.entry(item.line).or_default().push(item.clone());
        }
        for items in self.by_line.values_mut() {
            items.sort_by_key(|item| item.column);
        }
    }

    pub(crate) fn on_line(&self, line: usize) -> &[VirtualText] {
        self.by_line.get(&line).map_or(&[], Vec::as_slice)
    }
}

impl CodeEditor {
    /// Replaces the virtual text of one layer (for example `"inlay-hints"`),
    /// leaving the other layers alone. An empty list removes the layer.
    ///
    /// Positions are not moved when the buffer changes: the owner of a layer
    /// replaces it after an edit. Until then, a column past the end of its
    /// line draws at the end, and a line past the end of the buffer is not
    /// drawn.
    pub fn set_virtual_text(
        &mut self,
        layer: impl Into<String>,
        items: Vec<VirtualText>,
    ) {
        let layer = layer.into();
        let current = self.virtual_text.layers.get(&layer);
        if current.map_or(items.is_empty(), |current| *current == items) {
            return;
        }
        if items.is_empty() {
            self.virtual_text.layers.remove(&layer);
        } else {
            self.virtual_text.layers.insert(layer, items);
        }
        self.virtual_text.rebuild();
        self.invalidate_virtual_text_layout();
    }

    /// Drops everything computed from the displayed virtual text.
    pub(crate) fn invalidate_virtual_text_layout(&mut self) {
        self.virtual_text.revision = self.virtual_text.revision.wrapping_add(1);
        *self.max_content_width_cache.borrow_mut() = None;
        self.content_cache.clear();
        self.overlay_cache.clear();
    }

    /// The virtual text displayed on `line`: none while it is hidden
    /// ([`CodeEditor::set_virtual_text_enabled`]).
    fn displayed_virtual_text(&self, line: usize) -> &[VirtualText] {
        if self.virtual_text_enabled {
            self.virtual_text.on_line(line)
        } else {
            &[]
        }
    }

    /// Removes the virtual text of one layer.
    pub fn clear_virtual_text(&mut self, layer: &str) {
        self.set_virtual_text(layer, Vec::new());
    }

    /// The virtual text of one layer, as last given.
    pub fn virtual_text(&self, layer: &str) -> &[VirtualText] {
        self.virtual_text.layers.get(layer).map_or(&[], Vec::as_slice)
    }

    /// The runs displayed for the columns `cols` of `line`: the syntax `spans`
    /// of the whole line (the plain text color without them), recolored by
    /// the range styles, with the virtual text between them.
    ///
    /// The canvas and the sticky-scroll headers both draw these runs, so a
    /// pinned header looks like the line it mirrors.
    pub(crate) fn line_runs(
        &self,
        line: usize,
        spans: Option<&[(Color, String)]>,
        cols: (usize, usize),
    ) -> Vec<LineRun<'_>> {
        let mut pieces = Vec::new();
        if let Some(spans) = spans {
            let mut char_pos = 0;
            for (color, text) in spans {
                let text_end = char_pos + text.chars().count();
                let start = char_pos.max(cols.0);
                let end = text_end.min(cols.1);
                if start < end {
                    pieces.push((start, end, *color));
                }
                char_pos = text_end;
            }
        } else if cols.0 < cols.1 {
            pieces.push((cols.0, cols.1, self.style.text_color));
        }
        merge_virtual_text(
            apply_range_styles(pieces, self.range_styles.on_line(line)),
            self.displayed_virtual_text(line),
            self.buffer.line_len(line),
            cols,
        )
    }

    /// `(column, width)` of the virtual text on `line`, sorted by column, with
    /// columns clamped to the line length.
    pub(crate) fn virtual_text_widths(&self, line: usize) -> Vec<(usize, f32)> {
        let items = self.displayed_virtual_text(line);
        if items.is_empty() {
            return Vec::new();
        }
        let len = self.buffer.line_len(line);
        items
            .iter()
            .map(|item| {
                (
                    item.column.min(len),
                    measure_text_width(
                        &item.text,
                        self.full_char_width,
                        self.char_width,
                    ),
                )
            })
            .collect()
    }

    /// The wrapping calculator for the current settings and virtual text.
    pub(crate) fn wrapping_calculator(&self) -> WrappingCalculator {
        let virtual_text = self
            .virtual_text
            .by_line
            .keys()
            .map(|&line| (line, self.virtual_text_widths(line)))
            .collect();
        WrappingCalculator::new(
            self.wrap_enabled,
            self.wrap_column,
            self.full_char_width,
            self.char_width,
        )
        .with_virtual_text(virtual_text)
    }

    /// X of the cursor at `col` of `visual_line`, in content space (with the
    /// gutter, without the horizontal scroll). The cursor stays before the
    /// virtual text at its own column.
    pub(crate) fn caret_content_x(
        &self,
        visual_line: &VisualLine,
        col: usize,
    ) -> f32 {
        calculate_segment_geometry(
            self.buffer.line(visual_line.logical_line),
            visual_line.start_col,
            col,
            col,
            self.gutter_width() + 5.0,
            self.full_char_width,
            self.char_width,
            &self.virtual_text_widths(visual_line.logical_line),
        )
        .0
    }

    /// Width of the text of `line` plus its virtual text.
    pub(crate) fn line_display_width(&self, line: usize) -> f32 {
        measure_text_width(
            self.buffer.line(line),
            self.full_char_width,
            self.char_width,
        ) + self
            .displayed_virtual_text(line)
            .iter()
            .map(|item| {
                measure_text_width(
                    &item.text,
                    self.full_char_width,
                    self.char_width,
                )
            })
            .sum::<f32>()
    }
}

/// Total width of the virtual text in `widths` whose column is in
/// `from..to` (`to` inclusive when `inclusive`).
pub(crate) fn width_between(
    widths: &[(usize, f32)],
    from: usize,
    to: usize,
    inclusive: bool,
) -> f32 {
    widths
        .iter()
        .filter(|(col, _)| {
            *col >= from && (*col < to || (inclusive && *col == to))
        })
        .map(|(_, w)| w)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_editor::render::text::calculate_segment_geometry;
    use iced::Point;

    const GRAY: Color = Color::from_rgb(0.5, 0.5, 0.5);

    fn hint(line: usize, column: usize, text: &str) -> VirtualText {
        VirtualText { line, column, text: text.to_string(), color: GRAY }
    }

    /// `let total = 1;` with `: u32` after `total` (column 9).
    fn editor_with_type_hint() -> CodeEditor {
        let mut editor = CodeEditor::new("let total = 1;\nx", "rs");
        editor.set_virtual_text("hints", vec![hint(0, 9, ": u32")]);
        editor
    }

    #[test]
    fn layers_are_replaced_and_cleared_independently() {
        let mut editor = CodeEditor::new("a\nb", "rs");
        editor.set_virtual_text("types", vec![hint(0, 1, ": T")]);
        editor.set_virtual_text("debug", vec![hint(0, 1, " = 3")]);
        assert_eq!(editor.virtual_text.on_line(0).len(), 2);
        // Same column: layers in name order.
        assert_eq!(editor.virtual_text.on_line(0)[0].text, " = 3");

        let revision = editor.virtual_text.revision;
        editor.set_virtual_text("types", vec![hint(0, 1, ": T")]);
        assert_eq!(editor.virtual_text.revision, revision, "no change");

        editor.clear_virtual_text("debug");
        assert_eq!(editor.virtual_text("types"), &[hint(0, 1, ": T")]);
        assert!(editor.virtual_text("debug").is_empty());
        editor.set_virtual_text("types", Vec::new());
        assert!(editor.virtual_text.on_line(0).is_empty());
        assert_eq!(editor.content(), "a\nb", "never part of the buffer");
    }

    #[test]
    fn segments_and_the_cursor_around_virtual_text() {
        // `(x, width)`, the width only when given.
        let close = |(x, w): (f32, f32), (ex, ew): (f32, Option<f32>)| {
            (x - ex).abs() < 0.01 && ew.is_none_or(|ew| (w - ew).abs() < 0.01)
        };
        let hints = [(2, 30.0)];
        let geometry = |start, end| {
            calculate_segment_geometry(
                "abcd", 0, start, end, 0.0, 20.0, 10.0, &hints,
            )
        };
        // The cursor at the hint's column stays before it, one column later
        // it is after both.
        assert!(close(geometry(2, 2), (20.0, None)));
        assert!(close(geometry(3, 3), (60.0, None)));
        // Text starting at the hint's column starts after it.
        assert!(close(geometry(2, 3), (50.0, Some(10.0))));
        // Text ending at the hint's column does not cover it; text around it
        // does.
        assert!(close(geometry(0, 2), (0.0, Some(20.0))));
        assert!(close(geometry(1, 3), (10.0, Some(50.0))));
        // Without virtual text, nothing changes.
        assert!(close(
            calculate_segment_geometry("abcd", 0, 1, 3, 0.0, 20.0, 10.0, &[]),
            (10.0, Some(20.0))
        ));
    }

    #[test]
    fn the_cursor_x_and_clicks_skip_the_hint() {
        let editor = editor_with_type_hint();
        let visual_lines = editor.visual_lines_cached(800.0);
        let row = &visual_lines[0];
        let cw = editor.char_width;
        let hint_w = measure_text_width(": u32", editor.full_char_width, cw);
        let base = editor.gutter_width() + 5.0;
        assert!(
            (editor.caret_content_x(row, 9) - (base + 9.0 * cw)).abs() < 0.01
        );
        assert!(
            (editor.caret_content_x(row, 10) - (base + 10.0 * cw + hint_w))
                .abs()
                < 0.01
        );

        let click = |x: f32| {
            editor.calculate_cursor_from_point(Point::new(base + x, 5.0))
        };
        // On either half of the hint: the column before it.
        assert_eq!(click(9.0 * cw + 1.0), Some((0, 9)));
        assert_eq!(click(9.0 * cw + hint_w - 1.0), Some((0, 9)));
        // On the right half of the space after the hint: one column later.
        assert_eq!(click(9.0 * cw + hint_w + cw * 0.75), Some((0, 10)));
        // Past the end of the line.
        assert_eq!(click(40.0 * cw), Some((0, 14)));
    }

    #[test]
    fn hints_widen_the_line_and_its_wrapping() {
        let mut editor = CodeEditor::new("abcdefgh", "rs");
        let plain = editor.max_content_width();
        editor.set_virtual_text("hints", vec![hint(0, 8, " // end")]);
        let hint_w = measure_text_width(
            " // end",
            editor.full_char_width,
            editor.char_width,
        );
        assert!((editor.max_content_width() - plain - hint_w).abs() < 0.01);

        // Wrap at 6 columns: "abc" + a 4-column hint before `d` makes `d`
        // start the second row, with the hint in front of it.
        editor.set_virtual_text("hints", vec![hint(0, 3, "1234")]);
        editor.set_wrap_enabled(true);
        editor.set_wrap_column(Some(6));
        let rows = editor.visual_lines_cached(800.0);
        let cols: Vec<_> =
            rows.iter().map(|r| (r.start_col, r.end_col)).collect();
        assert_eq!(cols, vec![(0, 3), (3, 5), (5, 8)]);
    }

    #[test]
    fn a_hint_at_the_end_of_the_line_wraps_with_the_last_character() {
        let mut editor = CodeEditor::new("abcde", "rs");
        editor.set_wrap_enabled(true);
        editor.set_wrap_column(Some(6));

        // 5 columns of text + 4 of hint do not fit in 6: `e` moves to the
        // second row and takes the hint with it.
        editor.set_virtual_text("hints", vec![hint(0, 5, "1234")]);
        let rows = editor.visual_lines_cached(800.0);
        let cols: Vec<_> =
            rows.iter().map(|r| (r.start_col, r.end_col)).collect();
        assert_eq!(cols, vec![(0, 4), (4, 5)]);
        // The cursor at the end of the line stays on the hint's row.
        assert_eq!(WrappingCalculator::logical_to_visual(&rows, 0, 5), Some(1));

        // A hint that fits changes nothing.
        editor.set_virtual_text("hints", vec![hint(0, 5, "1")]);
        let rows = editor.visual_lines_cached(800.0);
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn runs_place_virtual_text_between_the_colored_pieces() {
        const RED: Color = Color::from_rgb(1.0, 0.0, 0.0);
        let text = |start, end| LineRun::Text { start, end, color: RED };
        let middle = hint(0, 2, "m");
        let end = hint(0, 4, "e");
        let items = [middle.clone(), end.clone()];
        let pieces = vec![(0, 4, RED)];

        assert_eq!(
            merge_virtual_text(pieces, &items, 4, (0, 4)),
            vec![
                text(0, 2),
                LineRun::Virtual(&middle),
                text(2, 4),
                LineRun::Virtual(&end),
            ]
        );
        // A segment that stops before the end of the line leaves the last
        // hint to the segment reaching it.
        assert_eq!(
            merge_virtual_text(vec![(0, 2, RED)], &items, 4, (0, 2)),
            vec![text(0, 2)]
        );
        assert_eq!(
            merge_virtual_text(vec![(2, 4, RED)], &items, 4, (2, 4)),
            vec![LineRun::Virtual(&middle), text(2, 4), LineRun::Virtual(&end),]
        );
        // An empty line only has its hints.
        assert_eq!(
            merge_virtual_text(Vec::new(), &items[1..], 0, (0, 0)),
            vec![LineRun::Virtual(&end)]
        );
    }

    #[test]
    fn line_runs_show_range_styles_and_virtual_text() {
        const RED: Color = Color::from_rgb(1.0, 0.0, 0.0);
        const BLUE: Color = Color::from_rgb(0.0, 0.0, 1.0);
        let mut editor = CodeEditor::new("abcd", "rs");
        let end = hint(0, 4, "!");
        editor.set_virtual_text("hints", vec![end.clone()]);
        editor.set_range_styles(
            "semantic",
            vec![crate::RangeStyle { line: 0, start: 1, end: 3, color: BLUE }],
        );

        let spans = vec![(RED, "abcd".to_string())];
        assert_eq!(
            editor.line_runs(0, Some(&spans), (0, 4)),
            vec![
                LineRun::Text { start: 0, end: 1, color: RED },
                LineRun::Text { start: 1, end: 3, color: BLUE },
                LineRun::Text { start: 3, end: 4, color: RED },
                LineRun::Virtual(&end),
            ]
        );

        // Without syntax spans, the plain text color is the base.
        let plain = editor.style.text_color;
        assert_eq!(
            editor.line_runs(0, None, (0, 1)),
            vec![LineRun::Text { start: 0, end: 1, color: plain }]
        );
    }

    #[test]
    fn hidden_virtual_text_takes_no_space_and_is_kept() {
        let mut editor = CodeEditor::new("abcde", "rs");
        editor.set_wrap_enabled(true);
        editor.set_wrap_column(Some(6));
        let plain_width = editor.max_content_width();
        let shown = hint(0, 5, "1234");
        editor.set_virtual_text("hints", vec![shown.clone()]);
        assert_eq!(editor.visual_lines_cached(800.0).len(), 2);

        editor.set_virtual_text_enabled(false);
        assert_eq!(editor.visual_lines_cached(800.0).len(), 1, "no wrap");
        assert!(editor.virtual_text_widths(0).is_empty());
        assert!((editor.max_content_width() - plain_width).abs() < 0.01);
        assert_eq!(
            editor.line_runs(0, None, (0, 5)).len(),
            1,
            "only the buffer text is drawn"
        );
        assert_eq!(editor.virtual_text("hints"), std::slice::from_ref(&shown));

        // A layer replaced while hidden is the one shown afterwards.
        let replaced = hint(0, 5, "12345");
        editor.set_virtual_text("hints", vec![replaced.clone()]);
        editor.set_virtual_text_enabled(true);
        assert_eq!(editor.visual_lines_cached(800.0).len(), 2);
        assert!(
            editor
                .line_runs(0, None, (4, 5))
                .contains(&LineRun::Virtual(&replaced))
        );
    }

    #[test]
    fn columns_past_the_line_are_drawn_at_its_end() {
        let mut editor = CodeEditor::new("ab", "rs");
        editor.set_virtual_text("hints", vec![hint(0, 99, "!")]);
        assert_eq!(editor.virtual_text_widths(0)[0].0, 2);
        assert!(editor.virtual_text_widths(5).is_empty());
    }
}
