//! Rendering caches: memoized wrapped visual lines, per-line syntax
//! highlighting, per-line bracket-nesting depth, and the maximum
//! content-width index used to size the horizontal scrollbar.

use iced::Color;
use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;
use syntect::highlighting::{HighlightState, Theme};
use syntect::parsing::{ParseState, SyntaxReference};

use crate::buffer::TextBuffer;
use crate::canvas_editor::features::bracket_match;
use crate::canvas_editor::render::wrapping;
use crate::canvas_editor::{CodeEditor, measure_text_width};

/// Memoized outcome of resolving an editor's syntax identifier and theme to a
/// syntect grammar and token palette.
///
/// Resolution is not free: `SyntaxSet::find_syntax_by_extension` scans every
/// bundled grammar's extension list linearly, and the `two-face` feature grows
/// that set several-fold. The canvas re-resolves on every frame, so the result
/// is memoized here, keyed on the two inputs that can change it.
pub(crate) struct ResolvedSyntax {
    /// Syntax identifier this entry was resolved for.
    syntax: String,
    /// Whether it was resolved against a dark editor background, which selects
    /// the dark token palette.
    dark_background: bool,
    /// The resolved grammar, `None` when syntect ships none at all.
    reference: Option<&'static SyntaxReference>,
    /// The resolved token palette, `None` when the theme set is empty.
    theme: Option<&'static Theme>,
}

impl ResolvedSyntax {
    /// Stores a resolution so later calls with the same inputs can reuse it.
    ///
    /// # Arguments
    ///
    /// * `syntax` - Syntax identifier that was resolved.
    /// * `dark_background` - Whether the editor background is dark.
    /// * `reference` - The grammar the identifier resolved to.
    /// * `theme` - The token palette the background selected.
    pub(crate) fn new(
        syntax: &str,
        dark_background: bool,
        reference: Option<&'static SyntaxReference>,
        theme: Option<&'static Theme>,
    ) -> Self {
        Self { syntax: syntax.to_string(), dark_background, reference, theme }
    }

    /// Returns the memoized resolution when it still matches both inputs.
    ///
    /// # Arguments
    ///
    /// * `syntax` - Syntax identifier being resolved now.
    /// * `dark_background` - Whether the editor background is dark now.
    ///
    /// # Returns
    ///
    /// The grammar and palette, or `None` when either input has changed.
    pub(crate) fn get(
        &self,
        syntax: &str,
        dark_background: bool,
    ) -> Option<(Option<&'static SyntaxReference>, Option<&'static Theme>)>
    {
        (self.syntax == syntax && self.dark_background == dark_background)
            .then_some((self.reference, self.theme))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct VisualLinesKey {
    pub(crate) buffer_revision: u64,
    /// `f32::to_bits()` is used so the cache key is stable and exact:
    /// - no epsilon comparisons are required
    /// - NaN payloads (if any) do not collapse unexpectedly
    viewport_width_bits: u32,
    gutter_width_bits: u32,
    wrap_enabled: bool,
    wrap_column: Option<usize>,
    folding_enabled: bool,
    fold_revision: u64,
    full_char_width_bits: u32,
    char_width_bits: u32,
}

pub(crate) struct VisualLinesCache {
    pub(crate) key: VisualLinesKey,
    visual_lines: Rc<Vec<wrapping::VisualLine>>,
    buffer_line_count: usize,
}

/// Per-line widths plus a counted ordered index for O(log n) max updates.
pub(crate) struct MaxContentWidthCache {
    revision: u64,
    line_widths: Vec<f32>,
    width_counts: BTreeMap<u32, usize>,
}

impl MaxContentWidthCache {
    fn add_width(&mut self, width: f32) {
        *self.width_counts.entry(width.to_bits()).or_insert(0) += 1;
    }

    fn remove_width(&mut self, width: f32) {
        let bits = width.to_bits();
        let remove_entry = if let Some(count) = self.width_counts.get_mut(&bits)
        {
            *count = count.saturating_sub(1);
            *count == 0
        } else {
            false
        };
        if remove_entry {
            self.width_counts.remove(&bits);
        }
    }

    fn max_width(&self) -> f32 {
        self.width_counts
            .last_key_value()
            .map_or(0.0, |(bits, _)| f32::from_bits(*bits))
    }
}

/// One highlighted logical line together with the syntect parser state *after*
/// it, so highlighting can resume sequentially from any cached line.
///
/// Storing the post-line state is what makes multi-line constructs (block
/// comments, multi-line strings) highlight correctly: line `N` is highlighted
/// starting from the state left by line `N - 1`.
struct CachedHighlightLine {
    /// Colored token spans covering the full logical line.
    spans: Rc<Vec<(Color, String)>>,
    /// Syntect parse state after this line (start state for the next line).
    parse_state: ParseState,
    /// Syntect highlight state after this line (start state for the next line).
    highlight_state: HighlightState,
}

/// Sequential per-line syntax-highlight cache.
///
/// `lines` holds a dense, valid prefix: `lines[i]` is the highlight of logical
/// line `i`. The prefix is extended lazily as deeper lines become visible and
/// truncated from the first edited line on each edit (see
/// [`CodeEditor::invalidate_highlight_from`]), so an edit never forces a full
/// re-parse from the top of the file.
pub(crate) struct HighlightCache {
    /// Active syntax/language identifier these lines were highlighted with.
    syntax: String,
    /// Dense valid prefix of highlighted lines (vector index = logical line).
    lines: Vec<CachedHighlightLine>,
}

impl HighlightCache {
    /// Creates an empty cache for the given syntax identifier.
    ///
    /// # Arguments
    ///
    /// * `syntax` - Active syntax/language identifier the cache is built for.
    pub(crate) fn new(syntax: String) -> Self {
        Self { syntax, lines: Vec::new() }
    }

    /// Returns the syntax identifier these lines were highlighted with.
    pub(crate) fn syntax(&self) -> &str {
        &self.syntax
    }

    /// Returns the number of highlighted logical lines (valid prefix length).
    pub(crate) fn valid_len(&self) -> usize {
        self.lines.len()
    }

    /// Returns the cached spans for `logical_line`, if within the valid prefix.
    ///
    /// # Arguments
    ///
    /// * `logical_line` - Index of the logical line to look up.
    pub(crate) fn spans(
        &self,
        logical_line: usize,
    ) -> Option<Rc<Vec<(Color, String)>>> {
        self.lines.get(logical_line).map(|line| Rc::clone(&line.spans))
    }

    /// Returns the syntect state to resume highlighting the next line from.
    ///
    /// This is the state left after the last cached line, or `None` when the
    /// cache is empty (highlighting then starts from the syntax's initial
    /// state).
    pub(crate) fn resume_state(&self) -> Option<(ParseState, HighlightState)> {
        self.lines.last().map(|line| {
            (line.parse_state.clone(), line.highlight_state.clone())
        })
    }

    /// Appends one highlighted line and its post-line state to the prefix.
    ///
    /// # Arguments
    ///
    /// * `spans` - The colored token spans of the line.
    /// * `parse_state` - Syntect parse state after the line.
    /// * `highlight_state` - Syntect highlight state after the line.
    pub(crate) fn push_line(
        &mut self,
        spans: Rc<Vec<(Color, String)>>,
        parse_state: ParseState,
        highlight_state: HighlightState,
    ) {
        self.lines.push(CachedHighlightLine {
            spans,
            parse_state,
            highlight_state,
        });
    }

    /// Truncates the valid prefix to `line`, discarding lines at index `line`
    /// and beyond so they are re-highlighted on next access.
    ///
    /// # Arguments
    ///
    /// * `line` - First logical line to invalidate.
    pub(crate) fn truncate(&mut self, line: usize) {
        self.lines.truncate(line);
    }
}

/// Sequential per-line bracket-nesting-depth cache used by bracket-pair
/// colorization.
///
/// `depths[i]` is the bracket nesting depth entering logical line `i`;
/// `depths[0]` is always `0`. Like [`HighlightCache`], the prefix is
/// extended lazily as deeper lines are drawn and truncated after an edit, so
/// an edit never forces a full rescan of the file. Unlike [`HighlightCache`],
/// no syntect state is involved: depth only depends on bracket characters, so
/// extending the prefix is a cheap plain-text scan.
pub(crate) struct BracketDepthCache {
    /// Dense valid prefix of "depth entering line `i`" (vector index = logical line).
    depths: Vec<usize>,
}

impl BracketDepthCache {
    /// Creates a cache seeded with `depths[0] == 0` (the file starts unnested).
    pub(crate) fn new() -> Self {
        Self { depths: vec![0] }
    }

    /// Returns the bracket nesting depth entering `line`, extending the
    /// cached prefix as needed.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The text buffer to scan for missing lines.
    /// * `line` - Logical line whose entering depth is requested.
    pub(crate) fn depth_at_line_start(
        &mut self,
        buffer: &TextBuffer,
        line: usize,
    ) -> usize {
        let target = line.min(buffer.line_count().saturating_sub(1));
        while self.depths.len() <= target {
            let idx = self.depths.len() - 1;
            let start = self.depths[idx];
            let end = bracket_match::bracket_depth_after_line(
                buffer.line(idx),
                start,
            );
            self.depths.push(end);
        }
        self.depths[target]
    }

    /// Truncates the cached prefix so depths entering `line` and beyond are
    /// recomputed on next access.
    ///
    /// Depths before `line` are unaffected since they only depend on earlier,
    /// unedited lines. Has no effect when the cache is already shorter.
    ///
    /// # Arguments
    ///
    /// * `line` - First logical line whose entering depth may have changed.
    pub(crate) fn truncate_from(&mut self, line: usize) {
        let keep = line.saturating_add(1).max(1);
        if self.depths.len() > keep {
            self.depths.truncate(keep);
        }
    }
}

impl CodeEditor {
    /// Returns the maximum content width across all lines, in pixels.
    ///
    /// Used to size the horizontal scrollbar when `wrap_enabled = false`.
    /// The result is cached keyed by `buffer_revision` so repeated calls are cheap.
    ///
    /// # Returns
    ///
    /// Total width in pixels including gutter, padding and a right margin.
    pub(crate) fn max_content_width(&self) -> f32 {
        let mut cache = self.max_content_width_cache.borrow_mut();
        if cache
            .as_ref()
            .is_none_or(|existing| existing.revision != self.buffer_revision)
        {
            let line_widths: Vec<f32> = (0..self.buffer.line_count())
                .map(|line| {
                    measure_text_width(
                        self.buffer.line(line),
                        self.full_char_width,
                        self.char_width,
                    )
                })
                .collect();
            let mut width_counts = BTreeMap::new();
            for width in &line_widths {
                *width_counts.entry(width.to_bits()).or_insert(0) += 1;
            }
            *cache = Some(MaxContentWidthCache {
                revision: self.buffer_revision,
                line_widths,
                width_counts,
            });
        }

        let gutter = self.gutter_width();
        let max_line_width =
            cache.as_ref().map_or(0.0, MaxContentWidthCache::max_width);

        // gutter + left padding + text + right margin
        gutter + 5.0 + max_line_width + 20.0
    }

    /// Returns wrapped "visual lines" for the current buffer and layout, with memoization.
    ///
    /// The editor frequently needs the wrapped view of the buffer:
    /// - hit-testing (mouse selection, cursor placement)
    /// - mapping logical ↔ visual positions
    /// - rendering (text, line numbers, highlights)
    ///
    /// Computing visual lines is relatively expensive for large files, so we
    /// cache the result keyed by:
    /// - `buffer_revision` (buffer content changes)
    /// - viewport width / gutter width (layout changes)
    /// - wrapping settings (wrap enabled / wrap column)
    /// - measured character widths (font / size changes)
    ///
    /// The returned `Rc<Vec<VisualLine>>` is cheap to clone and allows multiple
    /// rendering passes (content + overlay layers) to share the same computed
    /// layout without extra allocation.
    pub(crate) fn visual_lines_cached(
        &self,
        viewport_width: f32,
    ) -> Rc<Vec<wrapping::VisualLine>> {
        let key = VisualLinesKey {
            buffer_revision: self.buffer_revision,
            viewport_width_bits: viewport_width.to_bits(),
            gutter_width_bits: self.gutter_width().to_bits(),
            wrap_enabled: self.wrap_enabled,
            wrap_column: self.wrap_column,
            folding_enabled: self.folding_enabled,
            fold_revision: self.fold_revision,
            full_char_width_bits: self.full_char_width.to_bits(),
            char_width_bits: self.char_width.to_bits(),
        };

        let mut cache = self.visual_lines_cache.borrow_mut();
        if let Some(existing) = cache.as_ref()
            && existing.key == key
        {
            return existing.visual_lines.clone();
        }

        let hidden = self.hidden_lines_set();
        let wrapping_calc = wrapping::WrappingCalculator::new(
            self.wrap_enabled,
            self.wrap_column,
            self.full_char_width,
            self.char_width,
        );
        let visual_lines = wrapping_calc.calculate_visual_lines(
            &self.buffer,
            viewport_width,
            self.gutter_width(),
            &hidden,
        );
        let visual_lines = Rc::new(visual_lines);

        *cache = Some(VisualLinesCache {
            key,
            visual_lines: visual_lines.clone(),
            buffer_line_count: self.buffer.line_count(),
        });
        visual_lines
    }

    /// Rebuilds only the logical-line slice affected by the latest edit.
    ///
    /// The common typing path changes one line. Reusing the unchanged prefix
    /// and suffix prevents wrapping work from scaling with total file size.
    /// Collapsed folds intentionally fall back to a full rebuild because an
    /// indentation edit can change which distant lines are hidden.
    pub(crate) fn refresh_visual_lines_after_edit(
        &self,
        previous_revision: u64,
    ) {
        if !self.collapsed_folds.is_empty() {
            *self.visual_lines_cache.borrow_mut() = None;
            return;
        }

        let mut cache_guard = self.visual_lines_cache.borrow_mut();
        let Some(cache) = cache_guard.as_mut() else { return };
        if cache.key.buffer_revision != previous_revision {
            *cache_guard = None;
            return;
        }

        let same_layout = cache.key.gutter_width_bits
            == self.gutter_width().to_bits()
            && cache.key.wrap_enabled == self.wrap_enabled
            && cache.key.wrap_column == self.wrap_column
            && cache.key.folding_enabled == self.folding_enabled
            && cache.key.fold_revision == self.fold_revision
            && cache.key.full_char_width_bits == self.full_char_width.to_bits()
            && cache.key.char_width_bits == self.char_width.to_bits();
        if !same_layout {
            *cache_guard = None;
            return;
        }

        let old_line_count = cache.buffer_line_count;
        let new_line_count = self.buffer.line_count();
        let start_line =
            self.pre_edit_line.saturating_sub(1).min(old_line_count);
        let old_end_line =
            self.pre_edit_last_line.saturating_add(2).min(old_line_count);
        let new_end_line = if new_line_count >= old_line_count {
            old_end_line
                .saturating_add(new_line_count - old_line_count)
                .min(new_line_count)
        } else {
            old_end_line
                .saturating_sub(old_line_count - new_line_count)
                .max(start_line)
                .min(new_line_count)
        };

        let prefix_end = cache
            .visual_lines
            .partition_point(|visual| visual.logical_line < start_line);
        let suffix_start = cache
            .visual_lines
            .partition_point(|visual| visual.logical_line < old_end_line);

        let wrapping_calc = wrapping::WrappingCalculator::new(
            self.wrap_enabled,
            self.wrap_column,
            self.full_char_width,
            self.char_width,
        );
        let changed_visual_lines = wrapping_calc.calculate_visual_lines_range(
            &self.buffer,
            f32::from_bits(cache.key.viewport_width_bits),
            f32::from_bits(cache.key.gutter_width_bits),
            &HashSet::new(),
            start_line..new_end_line,
        );

        let old_segment_count = suffix_start.saturating_sub(prefix_end);
        let new_segment_count = changed_visual_lines.len();
        let visual_lines = Rc::make_mut(&mut cache.visual_lines);

        // The overwhelmingly common typing case keeps both the logical-line
        // count and the number of wrapped segments stable. Update that tiny
        // slice in place, without allocating or moving the rest of the file.
        if new_line_count == old_line_count
            && old_segment_count == new_segment_count
        {
            visual_lines[prefix_end..suffix_start]
                .clone_from_slice(&changed_visual_lines);
        } else {
            visual_lines.splice(prefix_end..suffix_start, changed_visual_lines);

            let shifted_suffix_start = prefix_end + new_segment_count;
            for visual in &mut visual_lines[shifted_suffix_start..] {
                visual.logical_line = if new_line_count >= old_line_count {
                    visual
                        .logical_line
                        .saturating_add(new_line_count - old_line_count)
                } else {
                    visual
                        .logical_line
                        .saturating_sub(old_line_count - new_line_count)
                };
            }
        }

        cache.key.buffer_revision = self.buffer_revision;
        cache.buffer_line_count = new_line_count;
    }

    /// Updates the horizontal-width index for the lines affected by an edit.
    ///
    /// This removes the final whole-file pass that used to happen after every
    /// keystroke when wrapping was disabled.
    pub(crate) fn refresh_max_content_width_after_edit(
        &self,
        previous_revision: u64,
    ) {
        let mut cache_guard = self.max_content_width_cache.borrow_mut();
        let Some(cache) = cache_guard.as_mut() else { return };
        if cache.revision != previous_revision {
            *cache_guard = None;
            return;
        }

        let old_line_count = cache.line_widths.len();
        let new_line_count = self.buffer.line_count();
        let start_line =
            self.pre_edit_line.saturating_sub(1).min(old_line_count);
        let old_end_line =
            self.pre_edit_last_line.saturating_add(2).min(old_line_count);
        if start_line == 0 && old_end_line == old_line_count {
            *cache_guard = None;
            return;
        }

        let new_end_line = if new_line_count >= old_line_count {
            old_end_line
                .saturating_add(new_line_count - old_line_count)
                .min(new_line_count)
        } else {
            old_end_line
                .saturating_sub(old_line_count - new_line_count)
                .max(start_line)
                .min(new_line_count)
        };
        let old_widths = cache.line_widths[start_line..old_end_line].to_vec();
        let new_widths: Vec<f32> = (start_line..new_end_line)
            .map(|line| {
                measure_text_width(
                    self.buffer.line(line),
                    self.full_char_width,
                    self.char_width,
                )
            })
            .collect();

        for width in old_widths {
            cache.remove_width(width);
        }
        for width in &new_widths {
            cache.add_width(*width);
        }
        cache.line_widths.splice(start_line..old_end_line, new_widths);
        cache.revision = self.buffer_revision;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_editor::Message;

    #[test]
    fn test_resolved_syntax_hits_only_on_both_keys() {
        let editor = CodeEditor::new("fn main() {}", "rs");
        let (_, reference, theme) = editor.resolve_syntax();
        let resolved = ResolvedSyntax::new("rs", true, reference, theme);

        assert!(
            resolved.get("rs", true).is_some(),
            "the same identifier and background must hit"
        );
        assert!(
            resolved.get("lua", true).is_none(),
            "a different syntax identifier must miss"
        );
        assert!(
            resolved.get("rs", false).is_none(),
            "a background lightness flip must miss: it selects another palette"
        );
    }

    #[test]
    fn test_resolve_syntax_populates_and_rekeys_the_memo() {
        let mut editor = CodeEditor::new("fn main() {}", "rs");
        assert!(
            editor.resolved_syntax.borrow().is_none(),
            "a fresh editor has resolved nothing yet"
        );

        let _ = editor.resolve_syntax();
        assert!(
            editor
                .resolved_syntax
                .borrow()
                .as_ref()
                .is_some_and(|resolved| resolved.get("rs", true).is_some()),
            "the first resolution must be memoized under its own key"
        );

        // The memo is self-keyed, so a language change must miss rather than
        // serve the previous grammar.
        editor.set_syntax("lua");
        assert!(
            editor
                .resolved_syntax
                .borrow()
                .as_ref()
                .is_some_and(|resolved| resolved.get("lua", true).is_none()),
            "the stale entry must not answer for the new identifier"
        );
        assert_eq!(editor.syntax_name(), "Lua");
    }

    #[test]
    fn test_bracket_depth_cache_extends_and_truncates() {
        let buffer = TextBuffer::new("fn main() {\n    let x = (1);\n}");
        let mut cache = BracketDepthCache::new();

        assert_eq!(cache.depth_at_line_start(&buffer, 0), 0);
        assert_eq!(cache.depth_at_line_start(&buffer, 1), 1);
        assert_eq!(cache.depth_at_line_start(&buffer, 2), 1);

        // Truncating from line 1 keeps depth entering line 1 but forces line 2
        // (and beyond) to be recomputed on next access.
        cache.truncate_from(1);
        assert_eq!(cache.depth_at_line_start(&buffer, 1), 1);
        assert_eq!(cache.depth_at_line_start(&buffer, 2), 1);
    }

    #[test]
    fn test_visual_lines_cached_reuses_cache_for_same_key() {
        let editor = CodeEditor::new("a\nb\nc", "rs");

        let first = editor.visual_lines_cached(800.0);
        let second = editor.visual_lines_cached(800.0);

        assert!(
            Rc::ptr_eq(&first, &second),
            "visual_lines_cached should reuse the cached Rc for identical keys"
        );
    }

    #[test]
    fn test_visual_lines_cached_changes_on_viewport_width_change() {
        let editor = CodeEditor::new("a\nb\nc", "rs");

        let first = editor.visual_lines_cached(800.0);
        let second = editor.visual_lines_cached(801.0);

        assert!(
            !Rc::ptr_eq(&first, &second),
            "visual_lines_cached should recompute when viewport width changes"
        );
    }

    #[test]
    fn test_visual_lines_cached_changes_on_buffer_revision_change() {
        let mut editor = CodeEditor::new("a\nb\nc", "rs");

        let first = editor.visual_lines_cached(800.0);
        editor.buffer_revision = editor.buffer_revision.wrapping_add(1);
        let second = editor.visual_lines_cached(800.0);

        assert!(
            !Rc::ptr_eq(&first, &second),
            "visual_lines_cached should recompute when buffer_revision changes"
        );
    }

    #[test]
    fn test_max_content_width_increases_with_longer_lines() {
        let short = CodeEditor::new("ab", "rs");
        let long =
            CodeEditor::new("abcdefghijklmnopqrstuvwxyz0123456789", "rs");

        assert!(
            long.max_content_width() > short.max_content_width(),
            "Longer lines should produce a greater max_content_width"
        );
    }

    #[test]
    fn test_max_content_width_ignores_blank_lines() {
        // Regression test for issue #26: a blank line measures to `-0.0`
        // (see `measure_text_width`'s doc note), and `MaxContentWidthCache`
        // orders widths by `f32::to_bits`. Before the fix, that sign bit made
        // any blank line sort above every real line's width, so a file with
        // both a blank line and a long line reported the blank line's width
        // (0) as the max instead of the long line's — hiding the horizontal
        // scrollbar even though the long line clearly overflowed.
        let with_blank_line = CodeEditor::new(
            "short\n\nabcdefghijklmnopqrstuvwxyz0123456789\n\n",
            "rs",
        );
        let without_blank_line = CodeEditor::new(
            "short\nabcdefghijklmnopqrstuvwxyz0123456789",
            "rs",
        );

        assert!(
            (with_blank_line.max_content_width()
                - without_blank_line.max_content_width())
            .abs()
                < f32::EPSILON,
            "blank lines must not affect the reported max content width"
        );
    }

    #[test]
    fn test_max_content_width_cached_by_revision() {
        let mut editor = CodeEditor::new("hello", "rs");
        let w1 = editor.max_content_width();

        // Same revision → cache hit
        let w2 = editor.max_content_width();
        assert!(
            (w1 - w2).abs() < f32::EPSILON,
            "Repeated calls with same revision should return identical value"
        );

        // Bump revision to simulate edit
        editor.buffer_revision = editor.buffer_revision.wrapping_add(1);
        // Update the buffer to reflect a longer line
        editor.buffer =
            crate::buffer::TextBuffer::new("hello world with extra content");
        let w3 = editor.max_content_width();
        assert!(
            w3 > w1,
            "After revision bump with longer content, width should increase"
        );
    }

    #[test]
    fn test_max_content_width_cache_updates_incrementally_after_newline() {
        let mut editor =
            CodeEditor::new("short\nthis is the longest line\ntail", "rs");
        editor.set_wrap_enabled(false);
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;
        editor.cursors.primary_mut().position = (1, 7);
        let _ = editor.max_content_width();

        let _ = editor.update(&Message::Enter);
        let incremental = editor.max_content_width();
        let expected = CodeEditor::new(&editor.content(), "rs");

        assert!(
            (incremental - expected.max_content_width()).abs() < f32::EPSILON
        );
        let cache = editor.max_content_width_cache.borrow();
        assert_eq!(
            cache.as_ref().map(|cache| cache.line_widths.len()),
            Some(editor.buffer.line_count())
        );
        assert_eq!(
            cache.as_ref().map(|cache| cache.revision),
            Some(editor.buffer_revision)
        );
    }
}
