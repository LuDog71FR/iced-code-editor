//! Fold/unfold operations on [`CodeEditor`](crate::canvas_editor::CodeEditor):
//! toggling, folding, and unfolding regions, plus the cursor/cache upkeep
//! that keeps folded state consistent with the buffer.

use std::collections::HashSet;
use std::rc::Rc;

use crate::canvas_editor::CodeEditor;

impl CodeEditor {
    /// Returns whether the region whose header is `header_line` is collapsed.
    ///
    /// # Arguments
    ///
    /// * `header_line` - Logical line index of the region header
    ///
    /// # Returns
    ///
    /// `true` if that region is currently collapsed, `false` otherwise --
    /// including when `header_line` is not a fold header at all.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// // A single block: the `fn` header on line 0 encloses lines 1 and 2.
    /// let mut editor = CodeEditor::new(
    ///     "fn main() {\n    let a = 1;\n    let b = 2;\n}",
    ///     "rs",
    /// );
    ///
    /// assert!(!editor.is_folded(0));
    ///
    /// editor.toggle_fold(0);
    /// assert!(editor.is_folded(0));
    /// ```
    pub fn is_folded(&self, header_line: usize) -> bool {
        self.collapsed_folds.contains(&header_line)
    }

    /// Toggles the collapsed state of the foldable region whose header is
    /// `header_line`.
    ///
    /// The call is a no-op if `header_line` is not currently a fold header.
    /// When collapsing, any cursor that would land on a hidden line is moved up
    /// to the header line so the caret stays visible.
    ///
    /// # Arguments
    ///
    /// * `header_line` - Logical line index of the region header
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// // A single block: the `fn` header on line 0 encloses lines 1 and 2.
    /// let mut editor = CodeEditor::new(
    ///     "fn main() {\n    let a = 1;\n    let b = 2;\n}",
    ///     "rs",
    /// );
    ///
    /// editor.toggle_fold(0);
    /// assert!(editor.is_folded(0));
    ///
    /// editor.toggle_fold(0);
    /// assert!(!editor.is_folded(0));
    ///
    /// // Line 1 sits inside the block but does not head it, so nothing moves.
    /// editor.toggle_fold(1);
    /// assert!(!editor.is_folded(0));
    /// assert!(!editor.is_folded(1));
    /// ```
    pub fn toggle_fold(&mut self, header_line: usize) {
        let regions = self.foldable_regions();
        if !super::is_fold_header(&regions, header_line) {
            return; // Not a fold header: nothing to toggle.
        }

        if self.collapsed_folds.contains(&header_line) {
            self.collapsed_folds.remove(&header_line);
        } else {
            self.collapsed_folds.insert(header_line);
        }
        self.after_fold_change();
    }

    /// Toggles the collapsed state of the innermost foldable region containing
    /// `line`.
    ///
    /// Folds the region if it is expanded, unfolds it if it is collapsed. Does
    /// nothing if `line` is not inside any foldable region. This is the
    /// cursor-driven primitive used by the keyboard shortcut and mirrors a click
    /// on the fold chevron.
    ///
    /// # Arguments
    ///
    /// * `line` - A logical line inside (or heading) the region to toggle
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// // A single block: the `fn` header on line 0 encloses lines 1 and 2.
    /// let mut editor = CodeEditor::new(
    ///     "fn main() {\n    let a = 1;\n    let b = 2;\n}",
    ///     "rs",
    /// );
    ///
    /// // Line 1 is inside the block, so the block's header collapses.
    /// editor.toggle_fold_at(1);
    /// assert!(editor.is_folded(0));
    ///
    /// editor.toggle_fold_at(1);
    /// assert!(!editor.is_folded(0));
    /// ```
    pub fn toggle_fold_at(&mut self, line: usize) {
        let regions = self.foldable_regions();
        let header = regions
            .iter()
            .filter(|r| r.start_line <= line && line <= r.end_line)
            .map(|r| r.start_line)
            .max();
        if let Some(header) = header {
            if self.collapsed_folds.contains(&header) {
                self.collapsed_folds.remove(&header);
            } else {
                self.collapsed_folds.insert(header);
            }
            self.after_fold_change();
        }
    }

    /// Folds the innermost foldable region containing `line`.
    ///
    /// Does nothing if `line` is not inside any foldable region or the region
    /// is already collapsed. This is the cursor-driven counterpart to
    /// [`Self::toggle_fold`].
    ///
    /// # Arguments
    ///
    /// * `line` - A logical line inside (or heading) the region to fold
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// // A single block: the `fn` header on line 0 encloses lines 1 and 2.
    /// let mut editor = CodeEditor::new(
    ///     "fn main() {\n    let a = 1;\n    let b = 2;\n}",
    ///     "rs",
    /// );
    ///
    /// editor.fold_at(2);
    /// assert!(editor.is_folded(0));
    ///
    /// // Already collapsed: a second call is a no-op rather than a re-open.
    /// editor.fold_at(2);
    /// assert!(editor.is_folded(0));
    /// ```
    pub fn fold_at(&mut self, line: usize) {
        let regions = self.foldable_regions();
        // Innermost containing region: the one with the greatest start line.
        let header = regions
            .iter()
            .filter(|r| r.start_line <= line && line <= r.end_line)
            .map(|r| r.start_line)
            .max();
        if let Some(header) = header
            && self.collapsed_folds.insert(header)
        {
            self.after_fold_change();
        }
    }

    /// Unfolds the innermost collapsed region containing `line`.
    ///
    /// Does nothing if no collapsed region contains `line`.
    ///
    /// # Arguments
    ///
    /// * `line` - A logical line inside (or heading) the region to unfold
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// // A single block: the `fn` header on line 0 encloses lines 1 and 2.
    /// let mut editor = CodeEditor::new(
    ///     "fn main() {\n    let a = 1;\n    let b = 2;\n}",
    ///     "rs",
    /// );
    ///
    /// editor.fold_at(1);
    /// assert!(editor.is_folded(0));
    ///
    /// editor.unfold_at(1);
    /// assert!(!editor.is_folded(0));
    ///
    /// // Nothing is collapsed any more, so unfolding again does nothing.
    /// editor.unfold_at(1);
    /// assert!(!editor.is_folded(0));
    /// ```
    pub fn unfold_at(&mut self, line: usize) {
        let regions = self.foldable_regions();
        let header = regions
            .iter()
            .filter(|r| {
                r.start_line <= line
                    && line <= r.end_line
                    && self.collapsed_folds.contains(&r.start_line)
            })
            .map(|r| r.start_line)
            .max();
        if let Some(header) = header
            && self.collapsed_folds.remove(&header)
        {
            self.after_fold_change();
        }
    }

    /// Folds every foldable block in the buffer.
    ///
    /// Nested blocks are collapsed too, so the outermost header is the only
    /// line each top-level block leaves on screen.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// // Two sibling blocks: headers on lines 0 and 3.
    /// let mut editor = CodeEditor::new(
    ///     "fn a() {\n    let x = 1;\n}\nfn b() {\n    let y = 2;\n}",
    ///     "rs",
    /// );
    ///
    /// editor.fold_all();
    ///
    /// assert!(editor.is_folded(0));
    /// assert!(editor.is_folded(3));
    /// ```
    pub fn fold_all(&mut self) {
        let regions = self.foldable_regions();
        let mut changed = false;
        for region in regions.iter() {
            changed |= self.collapsed_folds.insert(region.start_line);
        }
        if changed {
            self.after_fold_change();
        }
    }

    /// Unfolds every collapsed block in the buffer.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// // Two sibling blocks: headers on lines 0 and 3.
    /// let mut editor = CodeEditor::new(
    ///     "fn a() {\n    let x = 1;\n}\nfn b() {\n    let y = 2;\n}",
    ///     "rs",
    /// );
    ///
    /// editor.fold_all();
    /// assert!(editor.is_folded(0));
    ///
    /// editor.unfold_all();
    ///
    /// assert!(!editor.is_folded(0));
    /// assert!(!editor.is_folded(3));
    /// ```
    pub fn unfold_all(&mut self) {
        if !self.collapsed_folds.is_empty() {
            self.collapsed_folds.clear();
            self.after_fold_change();
        }
    }

    /// Finalizes a change to the collapsed set: keeps every cursor on a visible
    /// line and invalidates fold-dependent caches.
    fn after_fold_change(&mut self) {
        let hidden = self.hidden_lines_set();
        self.move_cursors_out_of_hidden(&hidden);
        self.bump_fold_revision();
    }

    /// Moves any cursor sitting on a hidden line up to the nearest visible line
    /// above it (the header of the enclosing collapsed block).
    fn move_cursors_out_of_hidden(&mut self, hidden: &HashSet<usize>) {
        if hidden.is_empty() {
            return;
        }
        for cursor in self.cursors.as_mut_slice() {
            let mut line = cursor.position.0;
            while line > 0 && hidden.contains(&line) {
                line -= 1;
            }
            if line != cursor.position.0 {
                cursor.position = (line, 0);
            }
        }
        self.cursors.sort_and_merge();
    }

    /// Invalidates fold-dependent caches after a fold-state change.
    pub(crate) fn bump_fold_revision(&mut self) {
        self.fold_revision = self.fold_revision.wrapping_add(1);
        self.content_cache.clear();
        self.overlay_cache.clear();
    }

    /// Returns the indentation blocks of the current buffer, memoized by
    /// `buffer_revision`.
    ///
    /// Independent of [`CodeEditor::folding_enabled`]: these are the blocks the
    /// buffer *has*, not the ones the user may collapse. Sticky scroll reads
    /// them to know which headers enclose a line
    /// (see [`CodeEditor::sticky_headroom`]) without offering to fold anything,
    /// so the two features share the computation and its cache but not the
    /// toggle. Callers that act on *folding* want
    /// [`Self::foldable_regions`] instead.
    pub(crate) fn block_regions(&self) -> Rc<Vec<super::FoldRegion>> {
        let mut cache = self.foldable_regions_cache.borrow_mut();
        if let Some((revision, regions)) = cache.as_ref()
            && *revision == self.buffer_revision
        {
            return regions.clone();
        }

        let regions = Rc::new(super::compute_foldable_regions(&self.buffer));
        *cache = Some((self.buffer_revision, regions.clone()));
        regions
    }

    /// Returns the foldable regions for the current buffer, memoized by
    /// `buffer_revision`.
    ///
    /// Returns an empty list when folding is disabled — which is what makes
    /// every folding operation a no-op without each of them testing the flag.
    pub(crate) fn foldable_regions(&self) -> Rc<Vec<super::FoldRegion>> {
        if !self.folding_enabled {
            return Rc::new(Vec::new());
        }

        self.block_regions()
    }

    /// Returns the set of logical lines hidden by the currently collapsed folds.
    ///
    /// Empty when folding is disabled or nothing is collapsed.
    pub(crate) fn hidden_lines_set(&self) -> HashSet<usize> {
        if !self.folding_enabled || self.collapsed_folds.is_empty() {
            return HashSet::new();
        }
        let regions = self.foldable_regions();
        super::hidden_lines(&regions, &self.collapsed_folds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Buffer with one outer block (lines 0..=4) and a nested inner block
    /// (lines 2..=3), used by the folding tests.
    fn folding_editor() -> CodeEditor {
        CodeEditor::new(
            "fn main() {\n    let x = 1;\n    if x > 0 {\n        print();\n    }\n}",
            "rs",
        )
    }

    #[test]
    fn test_foldable_regions_detected() {
        let editor = folding_editor();
        let regions = editor.foldable_regions();
        assert_eq!(
            *regions,
            vec![
                super::super::FoldRegion::new(0, 4),
                super::super::FoldRegion::new(2, 3)
            ]
        );
    }

    #[test]
    fn test_toggle_fold_hides_and_shows_lines() {
        let mut editor = folding_editor();
        let width = editor.viewport_width;
        let total = editor.visual_lines_cached(width).len();

        editor.toggle_fold(0);
        assert!(editor.is_folded(0));
        // Outer block hides lines 1..=4: only lines 0 and 5 remain.
        assert_eq!(editor.visual_lines_cached(width).len(), 2);

        editor.toggle_fold(0);
        assert!(!editor.is_folded(0));
        assert_eq!(editor.visual_lines_cached(width).len(), total);
    }

    #[test]
    fn test_toggle_fold_ignores_non_header() {
        let mut editor = folding_editor();
        editor.toggle_fold(3); // line 3 is not a header
        assert!(!editor.is_folded(3));
        assert!(editor.collapsed_folds.is_empty());
    }

    #[test]
    fn test_fold_at_picks_innermost_region() {
        let mut editor = folding_editor();
        // Line 3 is inside both (0,4) and (2,3); the innermost (header 2) folds.
        editor.fold_at(3);
        assert!(editor.is_folded(2));
        assert!(!editor.is_folded(0));
        assert_eq!(editor.hidden_lines_set(), [3].into_iter().collect());
    }

    #[test]
    fn test_unfold_at_expands_innermost_region() {
        let mut editor = folding_editor();
        editor.fold_at(3);
        editor.unfold_at(2);
        assert!(!editor.is_folded(2));
        assert!(editor.hidden_lines_set().is_empty());
    }

    #[test]
    fn test_toggle_fold_at_cursor_folds_then_unfolds() {
        let mut editor = folding_editor();
        // Line 3 is inside the innermost region (header 2).
        editor.toggle_fold_at(3);
        assert!(editor.is_folded(2));

        // Toggling again on the same line expands it back.
        editor.toggle_fold_at(2);
        assert!(!editor.is_folded(2));
    }

    #[test]
    fn test_toggle_fold_at_ignores_unfoldable_line() {
        let mut editor = CodeEditor::new("a\nb\nc", "rs");
        editor.toggle_fold_at(1);
        assert!(editor.collapsed_folds.is_empty());
    }

    #[test]
    fn test_fold_all_and_unfold_all() {
        let mut editor = folding_editor();
        editor.fold_all();
        assert!(editor.is_folded(0));
        assert!(editor.is_folded(2));
        // Outer fold hides 1..=4, inner hides 3: union is 1..=4.
        assert_eq!(
            editor.hidden_lines_set(),
            [1, 2, 3, 4].into_iter().collect()
        );

        editor.unfold_all();
        assert!(editor.collapsed_folds.is_empty());
        assert!(editor.hidden_lines_set().is_empty());
    }

    #[test]
    fn test_fold_moves_cursor_out_of_hidden_lines() {
        let mut editor = folding_editor();
        editor.cursors.set_single((3, 2));
        editor.fold_all();
        // Line 3 is hidden; the cursor moves up to the nearest visible line (0).
        assert_eq!(editor.cursors.primary_position(), (0, 0));
    }

    #[test]
    fn test_disabled_folding_yields_no_regions() {
        let mut editor = folding_editor();
        editor.set_folding_enabled(false);
        assert!(editor.foldable_regions().is_empty());
        // Collapsed state is preserved but produces no hidden lines while off.
        editor.collapsed_folds.insert(0);
        assert!(editor.hidden_lines_set().is_empty());
    }
}
