//! Line-based coloring for the output pane.
//!
//! The output pane renders its log through a read-only `text_editor` so the
//! user can select and copy lines. `text_editor` draws plain text, so the
//! per-level colors the pane used to get from one `text` widget per message
//! are reproduced here by [`LogHighlighter`], a
//! [`Highlighter`](iced::advanced::text::Highlighter) that colors each whole
//! line according to the `[LEVEL]` tag it carries.

use iced::Color;
use iced::advanced::text::Highlighter;
use std::ops::Range;

/// Colors used by [`LogHighlighter`], one per log level.
///
/// Doubles as the highlighter settings: `text_editor` re-runs the
/// highlighter whenever these change, so switching the application theme
/// recolors the log.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogPalette {
    /// Color of lines tagged `[ERROR]`.
    pub error: Color,
    /// Color of lines tagged `[OUTPUT]`.
    pub output: Color,
    /// Color of every other line.
    pub default: Color,
}

impl LogPalette {
    /// Returns the color a whole log line must be drawn with.
    pub fn color_for(&self, line: &str) -> Color {
        if line.contains("[ERROR]") {
            self.error
        } else if line.contains("[OUTPUT]") {
            self.output
        } else {
            self.default
        }
    }
}

/// Colors each log line as a single span, based on its `[LEVEL]` tag.
#[derive(Debug)]
pub struct LogHighlighter {
    /// Colors to pick from.
    palette: LogPalette,
    /// Index of the next line to highlight.
    current_line: usize,
}

impl Highlighter for LogHighlighter {
    type Settings = LogPalette;
    type Highlight = Color;
    type Iterator<'a> = std::option::IntoIter<(Range<usize>, Color)>;

    fn new(settings: &Self::Settings) -> Self {
        Self { palette: *settings, current_line: 0 }
    }

    fn update(&mut self, new_settings: &Self::Settings) {
        self.palette = *new_settings;
        self.current_line = 0;
    }

    fn change_line(&mut self, line: usize) {
        self.current_line = line;
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        self.current_line += 1;

        Some((0..line.len(), self.palette.color_for(line))).into_iter()
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three easily distinguishable colors, so a wrong branch is obvious.
    const PALETTE: LogPalette = LogPalette {
        error: Color::from_rgb(1.0, 0.0, 0.0),
        output: Color::from_rgb(0.0, 1.0, 0.0),
        default: Color::from_rgb(0.0, 0.0, 1.0),
    };

    #[test]
    fn test_color_for_picks_the_level_color() {
        assert_eq!(PALETTE.color_for("[ERROR] boom"), PALETTE.error);
        assert_eq!(PALETTE.color_for("[OUTPUT] hello"), PALETTE.output);
        assert_eq!(PALETTE.color_for("[INFO] started"), PALETTE.default);
        assert_eq!(PALETTE.color_for(""), PALETTE.default);
    }

    #[test]
    fn test_highlight_line_spans_the_whole_line() {
        let mut highlighter = LogHighlighter::new(&PALETTE);

        let spans: Vec<_> =
            highlighter.highlight_line("[ERROR] boom").collect();

        assert_eq!(spans, vec![(0..12, PALETTE.error)]);
    }

    #[test]
    fn test_highlight_line_advances_the_current_line() {
        let mut highlighter = LogHighlighter::new(&PALETTE);
        assert_eq!(highlighter.current_line(), 0);

        let _ = highlighter.highlight_line("[INFO] a").count();
        let _ = highlighter.highlight_line("[INFO] b").count();

        assert_eq!(highlighter.current_line(), 2);
    }

    #[test]
    fn test_change_line_rewinds_to_the_changed_line() {
        let mut highlighter = LogHighlighter::new(&PALETTE);
        let _ = highlighter.highlight_line("[INFO] a").count();
        let _ = highlighter.highlight_line("[INFO] b").count();

        highlighter.change_line(1);

        assert_eq!(highlighter.current_line(), 1);
    }

    #[test]
    fn test_update_swaps_the_palette_and_restarts() {
        let mut highlighter = LogHighlighter::new(&PALETTE);
        let _ = highlighter.highlight_line("[INFO] a").count();

        let inverted = LogPalette {
            error: PALETTE.output,
            output: PALETTE.error,
            default: PALETTE.default,
        };
        highlighter.update(&inverted);

        assert_eq!(highlighter.current_line(), 0);
        let spans: Vec<_> = highlighter.highlight_line("[ERROR] x").collect();
        assert_eq!(spans, vec![(0..9, inverted.error)]);
    }
}
