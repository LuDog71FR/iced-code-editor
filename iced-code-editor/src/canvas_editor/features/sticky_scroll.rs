//! Sticky-scroll header computation.
//!
//! Sticky scroll pins the header lines of the blocks enclosing the topmost
//! visible line to the top of the viewport, so the structural context (which
//! `impl`, which function, which `match`) stays readable while scrolling deep
//! inside a long block. This module holds the pure logic — which lines to pin —
//! so it can be unit-tested without a renderer; the drawing itself lives in
//! [`crate::canvas_editor::render`].
//!
//! Scope detection is shared with code folding: the enclosing blocks are the
//! [`FoldRegion`]s containing the line, which
//! [`compute_foldable_regions`](super::folding::compute_foldable_regions)
//! already produces nested and in ascending header order. Detection is
//! therefore indentation-based, with the same trade-off as folding: it is
//! language-agnostic, but badly indented code yields misleading headers.

use super::folding::FoldRegion;

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
/// # Example
///
/// ```ignore
/// // `fn` at line 0 encloses lines 1..=3, `if` at line 1 encloses lines 2..=3.
/// let regions = vec![FoldRegion::new(0, 3), FoldRegion::new(1, 3)];
///
/// // Deep inside both blocks, both headers are pinned, outermost first.
/// assert_eq!(sticky_headers(&regions, 3, 5), vec![0, 1]);
///
/// // On the inner header itself, only the outer one is pinned.
/// assert_eq!(sticky_headers(&regions, 1, 5), vec![0]);
/// ```
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `fn` at line 0 wrapping `if` at line 1, both ending at line 4.
    fn nested_regions() -> Vec<FoldRegion> {
        vec![FoldRegion::new(0, 4), FoldRegion::new(1, 4)]
    }

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
