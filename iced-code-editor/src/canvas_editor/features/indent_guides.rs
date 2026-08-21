//! Indentation-guide level computation.
//!
//! Indentation guides are the faint vertical lines an editor draws at every
//! indentation level, making the nesting of a block visible without reading its
//! braces. This module holds the pure logic — how many guides a given line
//! deserves — so it can be unit-tested without a renderer; the drawing itself
//! lives in [`crate::canvas_editor::render`].
//!
//! Blank lines have no indentation of their own, so their level is inferred
//! from the nearest non-blank line above and below (see [`guide_levels`]).

use crate::buffer::TextBuffer;
use crate::canvas_editor::indent_width;

/// Maximum number of consecutive blank lines scanned when inferring the level
/// of a blank line.
///
/// Without a bound, a buffer made mostly of blank lines would turn every
/// visible line into a full-buffer scan. Past this many blank lines the run is
/// treated as a gap between blocks and no guide is drawn, which is what a
/// reader would expect from such a large hole anyway.
const MAX_BLANK_RUN_SCAN: usize = 200;

/// Computes how many indentation guides `line` should display.
///
/// A line indented by `n` display columns gets `n / unit` guides, drawn at
/// columns `0, unit, 2 * unit, …`. Blank lines take the *smaller* of the levels
/// of the nearest non-blank line above and below them, so a blank line inside a
/// block keeps the block's guides while a blank line between two blocks does
/// not sprout guides that lead nowhere.
///
/// # Arguments
///
/// * `buffer` - The buffer the line belongs to
/// * `line` - Index of the logical line to measure
/// * `unit` - Width of one indentation level, in display columns
///
/// # Returns
///
/// The number of guides to draw. Zero when `unit` is `0`, when `line` is out of
/// bounds, or when the line sits at the top level.
pub(crate) fn guide_levels(
    buffer: &TextBuffer,
    line: usize,
    unit: usize,
) -> usize {
    if unit == 0 || line >= buffer.line_count() {
        return 0;
    }

    let width = match indent_width(buffer.line(line)) {
        Some(width) => width,
        None => blank_line_width(buffer, line),
    };

    width / unit
}

/// Infers the indentation width of a blank line from its non-blank neighbours.
///
/// Returns the smaller of the widths found above and below, or `0` when either
/// side runs into the edge of the buffer or into more than
/// [`MAX_BLANK_RUN_SCAN`] blank lines.
///
/// # Arguments
///
/// * `buffer` - The buffer the line belongs to
/// * `line` - Index of the blank logical line
fn blank_line_width(buffer: &TextBuffer, line: usize) -> usize {
    let above = (0..line)
        .rev()
        .take(MAX_BLANK_RUN_SCAN)
        .find_map(|index| indent_width(buffer.line(index)));
    let below = ((line + 1)..buffer.line_count())
        .take(MAX_BLANK_RUN_SCAN)
        .find_map(|index| indent_width(buffer.line(index)));

    match (above, below) {
        (Some(above), Some(below)) => above.min(below),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_editor::TAB_WIDTH;

    #[test]
    fn test_guide_levels_space_indentation() {
        let buffer = TextBuffer::new("fn f() {\n    a();\n        b();\n}");
        assert_eq!(guide_levels(&buffer, 0, 4), 0);
        assert_eq!(guide_levels(&buffer, 1, 4), 1);
        assert_eq!(guide_levels(&buffer, 2, 4), 2);
        assert_eq!(guide_levels(&buffer, 3, 4), 0);
    }

    #[test]
    fn test_guide_levels_tab_indentation() {
        let buffer = TextBuffer::new("fn f() {\n\ta();\n\t\tb();\n}");
        assert_eq!(guide_levels(&buffer, 1, TAB_WIDTH), 1);
        assert_eq!(guide_levels(&buffer, 2, TAB_WIDTH), 2);
    }

    #[test]
    fn test_guide_levels_zero_unit_draws_nothing() {
        let buffer = TextBuffer::new("        deep();");
        assert_eq!(guide_levels(&buffer, 0, 0), 0);
    }

    #[test]
    fn test_guide_levels_out_of_bounds_line() {
        let buffer = TextBuffer::new("    a();");
        assert_eq!(guide_levels(&buffer, 42, 4), 0);
    }

    #[test]
    fn test_guide_levels_blank_line_inside_block() {
        // The blank line is surrounded by equally indented code, so it keeps
        // the block's guide.
        let buffer = TextBuffer::new("fn f() {\n    a();\n\n    b();\n}");
        assert_eq!(guide_levels(&buffer, 2, 4), 1);
    }

    #[test]
    fn test_guide_levels_blank_line_at_end_of_block_uses_min() {
        // Below the blank line the block is already closed, so the smaller of
        // the two neighbouring levels wins and no guide is drawn.
        let buffer = TextBuffer::new("fn f() {\n    a();\n\n}");
        assert_eq!(guide_levels(&buffer, 2, 4), 0);
    }

    #[test]
    fn test_guide_levels_blank_line_between_blocks_uses_min() {
        let buffer =
            TextBuffer::new("fn f() {\n        a();\n\n    b();\n    }");
        assert_eq!(guide_levels(&buffer, 2, 4), 1);
    }

    #[test]
    fn test_guide_levels_blank_line_at_buffer_edges() {
        let buffer = TextBuffer::new("\n    a();\n\n");
        // No non-blank line above the first line.
        assert_eq!(guide_levels(&buffer, 0, 4), 0);
        // No non-blank line below the last line.
        assert_eq!(guide_levels(&buffer, 2, 4), 0);
    }

    #[test]
    fn test_guide_levels_indent_not_multiple_of_unit() {
        // Six columns of indentation with a unit of four: one full level, the
        // remainder does not earn a guide of its own.
        let buffer = TextBuffer::new("      odd();");
        assert_eq!(guide_levels(&buffer, 0, 4), 1);
    }

    #[test]
    fn test_guide_levels_long_blank_run_draws_nothing() {
        // The run must be long enough that its middle is more than
        // MAX_BLANK_RUN_SCAN lines away from *both* neighbours.
        let blank_lines = MAX_BLANK_RUN_SCAN * 2 + 50;
        let mut text = String::from("    a();\n");
        text.push_str(&"\n".repeat(blank_lines));
        text.push_str("    b();");
        let buffer = TextBuffer::new(&text);

        // Every line of the run is out of reach of at least one neighbour, so
        // the whole gap stays free of guides.
        assert_eq!(guide_levels(&buffer, 1, 4), 0);
        assert_eq!(guide_levels(&buffer, 1 + blank_lines / 2, 4), 0);
        assert_eq!(guide_levels(&buffer, blank_lines, 4), 0);
    }
}
