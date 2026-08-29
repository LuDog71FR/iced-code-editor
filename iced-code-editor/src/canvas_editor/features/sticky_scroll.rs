//! Sticky-scroll header computation.
//!
//! Sticky scroll pins the header lines of the blocks enclosing the topmost
//! visible line to the top of the viewport, so the structural context (which
//! `impl`, which function, which `match`) stays readable while scrolling deep
//! inside a long block. This module holds the logic — which lines to pin, and
//! how much room they take — so it can be unit-tested without a renderer; the
//! drawing itself lives in [`crate::canvas_editor::render`].
//!
//! Scope detection is shared with code folding: the enclosing blocks are the
//! [`FoldRegion`]s containing the line, which
//! [`compute_foldable_regions`](super::folding::compute_foldable_regions)
//! already produces nested and in ascending header order. Detection is
//! therefore indentation-based, with the same trade-off as folding: it is
//! language-agnostic, but badly indented code yields misleading headers.
//!
//! What is shared is the computation and its cache, not the toggle: the blocks
//! a line sits in are a property of the buffer, not of whether the user may
//! collapse them. This module therefore reads
//! [`CodeEditor::block_regions`], which ignores
//! [`CodeEditor::folding_enabled`], so turning code folding off removes the
//! fold chevrons and leaves the pinned headers alone.

use super::folding::FoldRegion;
use crate::canvas_editor::CodeEditor;

/// Maximum number of header lines pinned at once, mirroring VS Code's default.
///
/// Without a bound, a deeply nested block would pin enough headers to bury the
/// code the reader is actually looking at.
pub(crate) const DEFAULT_MAX_STICKY_LINES: usize = 5;

/// Returns the logical lines to pin above `top_line`, outermost block first.
///
/// A region qualifies when it *strictly* encloses `top_line`, that is
/// `start_line < top_line <= end_line`. The strict comparison on `start_line`
/// matters: when the header itself is the topmost visible line it is already on
/// screen, and pinning it would show the same line twice.
///
/// # Arguments
///
/// * `regions` - Pre-computed fold regions, in ascending `start_line` order
/// * `top_line` - Index of the topmost visible logical line
/// * `max_lines` - Upper bound on the number of pinned headers
///
/// # Returns
///
/// Header line indices, outermost first, at most `max_lines` of them. Empty
/// when `top_line` sits at the top level or when `max_lines` is `0`.
///
/// # Behaviour
///
/// Take a `fn` at line 0 enclosing an `if` at line 1, both closing at line 4.
/// Asked from line 3, deep inside both blocks, this returns `[0, 1]`: both
/// headers, outermost first. Asked from line 1 — the inner header itself,
/// already on screen — it returns `[0]` alone.
///
/// Both this function and [`FoldRegion::new`] are `pub(crate)`, so the two
/// cases above cannot be written as a doctest. They are pinned instead by
/// `test_nested_headers_are_outermost_first` and
/// `test_visible_header_is_not_pinned` below, which the compiler does read.
pub(crate) fn sticky_headers(
    regions: &[FoldRegion],
    top_line: usize,
    max_lines: usize,
) -> Vec<usize> {
    if max_lines == 0 {
        return Vec::new();
    }

    // `regions` is already sorted by ascending `start_line`, so filtering
    // preserves the outermost-first order the caller expects.
    regions
        .iter()
        .filter(|region| {
            region.start_line < top_line && top_line <= region.end_line
        })
        .map(|region| region.start_line)
        .take(max_lines)
        .collect()
}

impl CodeEditor {
    /// Returns how many header rows will still be pinned once `line` is the
    /// topmost visible line.
    ///
    /// Scrolling a line to row 0 of the viewport does not make it visible: the
    /// sticky layer is drawn *over* the top rows, so a line that is itself
    /// enclosed by a block arrives underneath that block's pinned header. This
    /// is the number of rows a scroll must leave free above `line` for it to
    /// actually be readable — the `rows_above` argument of
    /// [`CodeEditor::scroll_to_line`].
    ///
    /// `0` when sticky scroll is off or when nothing encloses `line`. Code
    /// folding being disabled does not affect it: the blocks come from
    /// [`CodeEditor::block_regions`], which does not consult that toggle.
    ///
    /// # Arguments
    ///
    /// * `line` - Index of the logical line that is about to become the topmost
    ///   visible one
    ///
    /// # Returns
    ///
    /// The number of headers that will remain pinned above it, at most
    /// [`DEFAULT_MAX_STICKY_LINES`]
    pub(crate) fn sticky_headroom(&self, line: usize) -> usize {
        if !self.sticky_scroll_enabled {
            return 0;
        }

        sticky_headers(&self.block_regions(), line, DEFAULT_MAX_STICKY_LINES)
            .len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `fn` at line 0 wrapping `if` at line 1, both ending at line 4.
    fn nested_regions() -> Vec<FoldRegion> {
        vec![FoldRegion::new(0, 4), FoldRegion::new(1, 4)]
    }

    /// `fn` at line 0, `if` at line 1, three statements, both blocks closing.
    const NESTED_SOURCE: &str =
        "fn main() {\n    if a {\n        b();\n        c();\n    }\n}";

    #[test]
    fn test_no_headers_without_regions() {
        assert!(sticky_headers(&[], 10, DEFAULT_MAX_STICKY_LINES).is_empty());
    }

    #[test]
    fn test_nested_headers_are_outermost_first() {
        let regions = nested_regions();
        assert_eq!(
            sticky_headers(&regions, 3, DEFAULT_MAX_STICKY_LINES),
            vec![0, 1]
        );
    }

    #[test]
    fn test_visible_header_is_not_pinned() {
        let regions = nested_regions();
        // Line 1 is the inner header and is on screen: only the outer one is
        // pinned, so the line is never rendered twice.
        assert_eq!(
            sticky_headers(&regions, 1, DEFAULT_MAX_STICKY_LINES),
            vec![0]
        );
        // Line 0 is the outermost header: nothing encloses it.
        assert!(
            sticky_headers(&regions, 0, DEFAULT_MAX_STICKY_LINES).is_empty()
        );
    }

    #[test]
    fn test_line_past_region_end_is_not_enclosed() {
        let regions = nested_regions();
        assert!(
            sticky_headers(&regions, 5, DEFAULT_MAX_STICKY_LINES).is_empty()
        );
    }

    #[test]
    fn test_last_line_of_region_is_enclosed() {
        let regions = nested_regions();
        assert_eq!(
            sticky_headers(&regions, 4, DEFAULT_MAX_STICKY_LINES),
            vec![0, 1]
        );
    }

    #[test]
    fn test_truncation_keeps_outermost_scopes() {
        let regions = vec![
            FoldRegion::new(0, 9),
            FoldRegion::new(1, 9),
            FoldRegion::new(2, 9),
            FoldRegion::new(3, 9),
        ];
        assert_eq!(sticky_headers(&regions, 8, 2), vec![0, 1]);
    }

    #[test]
    fn test_zero_max_lines_pins_nothing() {
        let regions = nested_regions();
        assert!(sticky_headers(&regions, 3, 0).is_empty());
    }

    #[test]
    fn test_headroom_is_zero_for_an_outermost_header() {
        // Nothing encloses line 0, so scrolling it to row 0 leaves it visible.
        let editor = CodeEditor::new(NESTED_SOURCE, "rs");

        assert_eq!(editor.sticky_headroom(0), 0);
    }

    #[test]
    fn test_headroom_reserves_a_row_for_a_nested_header() {
        // Line 1 is the `if` header, still enclosed by `fn`. Scrolled to row 0
        // it would land underneath the `fn` header the layer keeps pinned --
        // the jump has to leave that one row free.
        let editor = CodeEditor::new(NESTED_SOURCE, "rs");

        assert_eq!(editor.sticky_headroom(1), 1);
    }

    #[test]
    fn test_headroom_counts_every_enclosing_block() {
        // Deep inside both blocks: two headers stay pinned.
        let editor = CodeEditor::new(NESTED_SOURCE, "rs");

        assert_eq!(editor.sticky_headroom(3), 2);
    }

    #[test]
    fn test_headroom_survives_code_folding_being_disabled() {
        // The two features share the block computation, not the toggle. A user
        // who turns folding off wants no fold chevrons; nothing about that
        // says the structural context should stop being pinned.
        let mut editor = CodeEditor::new(NESTED_SOURCE, "rs");
        assert_eq!(editor.sticky_headroom(3), 2);

        editor.set_folding_enabled(false);

        assert_eq!(
            editor.sticky_headroom(3),
            2,
            "sticky scroll must not follow the folding toggle"
        );
    }

    #[test]
    fn test_headroom_is_zero_when_sticky_scroll_is_off() {
        // No layer, nothing covering row 0, nothing to reserve.
        let mut editor = CodeEditor::new(NESTED_SOURCE, "rs");
        assert_eq!(editor.sticky_headroom(3), 2);

        editor.set_sticky_scroll_enabled(false);

        assert_eq!(editor.sticky_headroom(3), 0);
    }

    #[test]
    fn test_sibling_region_is_not_pinned() {
        // Two consecutive top-level blocks: standing in the second one must not
        // pin the first one's header.
        let regions = vec![FoldRegion::new(0, 2), FoldRegion::new(3, 5)];
        assert_eq!(
            sticky_headers(&regions, 4, DEFAULT_MAX_STICKY_LINES),
            vec![3]
        );
    }
}
