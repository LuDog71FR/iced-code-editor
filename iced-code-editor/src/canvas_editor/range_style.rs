//! Colors given to ranges of the text, such as LSP semantic tokens
//! (`parameter`, `mutable variable`) drawn over the syntax highlighting.
//!
//! A range style only changes the color of characters that are already
//! there: it never changes a width, so the cursor, selection, hit testing and
//! wrapping are untouched.

use std::collections::{BTreeMap, HashMap};

use iced::Color;

use crate::canvas_editor::CodeEditor;

/// The color of one range of a line, see [`CodeEditor::set_range_styles`].
#[derive(Debug, Clone, PartialEq)]
pub struct RangeStyle {
    /// Logical line (0-based).
    pub line: usize,
    /// First character column of the range.
    pub start: usize,
    /// Character column after the range (exclusive).
    pub end: usize,
    /// Text color, instead of the syntax highlighting's.
    pub color: Color,
}

/// The range styles of every layer, and the same styles by line for drawing.
#[derive(Debug, Default)]
pub(crate) struct RangeStyleLayers {
    layers: BTreeMap<String, Vec<RangeStyle>>,
    /// Every style, by line, in the order they are painted: layers in name
    /// order, and styles in the order they were given. Later ones win.
    by_line: HashMap<usize, Vec<RangeStyle>>,
}

impl RangeStyleLayers {
    fn rebuild(&mut self) {
        self.by_line.clear();
        for style in self.layers.values().flatten() {
            if style.start < style.end {
                self.by_line.entry(style.line).or_default().push(style.clone());
            }
        }
    }

    pub(crate) fn on_line(&self, line: usize) -> &[RangeStyle] {
        self.by_line.get(&line).map_or(&[], Vec::as_slice)
    }
}

/// Recolors the `(start, end, color)` pieces of a line with `styles`, in
/// order, splitting a piece where a style starts or ends inside it. The
/// pieces keep their columns; only colors change.
pub(crate) fn apply_range_styles(
    mut pieces: Vec<(usize, usize, Color)>,
    styles: &[RangeStyle],
) -> Vec<(usize, usize, Color)> {
    for style in styles {
        let mut next = Vec::with_capacity(pieces.len() + 2);
        for (start, end, color) in pieces {
            let from = style.start.max(start);
            let to = style.end.min(end);
            if from >= to {
                next.push((start, end, color));
                continue;
            }
            if start < from {
                next.push((start, from, color));
            }
            next.push((from, to, style.color));
            if to < end {
                next.push((to, end, color));
            }
        }
        pieces = next;
    }
    pieces
}

impl CodeEditor {
    /// Replaces the range styles of one layer (for example
    /// `"semantic-tokens"`), leaving the other layers alone. An empty list
    /// removes the layer.
    ///
    /// Where styles overlap, layers later in name order win, and within a
    /// layer the later style wins. Positions are not moved when the buffer
    /// changes: the owner of a layer replaces it after an edit. Until then,
    /// a range past the end of its line is cut there.
    pub fn set_range_styles(
        &mut self,
        layer: impl Into<String>,
        styles: Vec<RangeStyle>,
    ) {
        let layer = layer.into();
        let current = self.range_styles.layers.get(&layer);
        if current.map_or(styles.is_empty(), |current| *current == styles) {
            return;
        }
        if styles.is_empty() {
            self.range_styles.layers.remove(&layer);
        } else {
            self.range_styles.layers.insert(layer, styles);
        }
        self.range_styles.rebuild();
        self.content_cache.clear();
    }

    /// Removes the range styles of one layer.
    pub fn clear_range_styles(&mut self, layer: &str) {
        self.set_range_styles(layer, Vec::new());
    }

    /// The range styles of one layer, as last given.
    pub fn range_styles(&self, layer: &str) -> &[RangeStyle] {
        self.range_styles.layers.get(layer).map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color::from_rgb(1.0, 0.0, 0.0);
    const GREEN: Color = Color::from_rgb(0.0, 1.0, 0.0);
    const BLUE: Color = Color::from_rgb(0.0, 0.0, 1.0);

    fn style(start: usize, end: usize, color: Color) -> RangeStyle {
        RangeStyle { line: 0, start, end, color }
    }

    #[test]
    fn test_style_inside_a_piece_splits_it() {
        let pieces = vec![(0, 10, RED)];
        assert_eq!(
            apply_range_styles(pieces, &[style(3, 6, GREEN)]),
            vec![(0, 3, RED), (3, 6, GREEN), (6, 10, RED)]
        );
    }

    #[test]
    fn test_style_across_pieces_and_past_the_end() {
        let pieces = vec![(0, 4, RED), (4, 8, BLUE)];
        assert_eq!(
            apply_range_styles(pieces, &[style(2, 20, GREEN)]),
            vec![(0, 2, RED), (2, 4, GREEN), (4, 8, GREEN)]
        );
    }

    #[test]
    fn test_later_style_wins() {
        let pieces = vec![(0, 6, RED)];
        assert_eq!(
            apply_range_styles(
                pieces,
                &[style(0, 6, GREEN), style(2, 4, BLUE)]
            ),
            vec![(0, 2, GREEN), (2, 4, BLUE), (4, 6, GREEN)]
        );
    }

    #[test]
    fn test_layers_by_line_in_name_order() {
        let mut editor = CodeEditor::new("let x = 1;\nlet y = 2;", "rs");
        editor.set_range_styles("b", vec![style(4, 5, BLUE)]);
        editor.set_range_styles(
            "a",
            vec![
                style(4, 5, GREEN),
                RangeStyle { line: 1, ..style(4, 5, RED) },
            ],
        );
        let colors: Vec<_> =
            editor.range_styles.on_line(0).iter().map(|s| s.color).collect();
        assert_eq!(colors, vec![GREEN, BLUE], "layer \"b\" is painted last");
        assert_eq!(editor.range_styles.on_line(1).len(), 1);
        assert_eq!(editor.range_styles("a").len(), 2);

        editor.clear_range_styles("a");
        assert_eq!(editor.range_styles.on_line(0).len(), 1);
        assert!(editor.range_styles.on_line(1).is_empty());
        assert!(editor.range_styles("a").is_empty());
    }

    #[test]
    fn test_scope_color_follows_the_theme_and_token_colors() {
        let mut editor = CodeEditor::new("fn main() {}", "rs");
        let function = editor.scope_color("entity.name.function");
        assert!(function.is_some());
        assert_ne!(function, editor.scope_color("keyword"));
        assert_eq!(editor.scope_color("not a scope!"), None);

        let mut style = editor.style;
        style.token_colors.string = Some(RED);
        editor.set_theme(style);
        assert_eq!(editor.scope_color("string.quoted.double"), Some(RED));
    }

    #[test]
    fn test_empty_ranges_are_ignored() {
        let mut editor = CodeEditor::new("abc", "txt");
        editor.set_range_styles("a", vec![style(2, 2, RED), style(3, 1, RED)]);
        assert!(editor.range_styles.on_line(0).is_empty());
    }
}
