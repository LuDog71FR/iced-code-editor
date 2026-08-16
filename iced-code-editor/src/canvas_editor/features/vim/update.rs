//! Message handling for Vim-mode key dispatch (motions, operators, paste,
//! undo/redo, mode switches, and insert-position handling).

use iced::Task;

use super::{
    VimAction, VimInsertPosition, VimMode, VimMotion, VimOperator,
    VimPastePosition, VimRegister, VimRegisterKind,
};
use crate::canvas_editor::editing::command::{
    Command, DeleteRangeCommand, InsertTextCommand,
};
use crate::canvas_editor::{CodeEditor, Message};

impl CodeEditor {
    /// Dispatches a parsed Vim key action to the corresponding handler.
    ///
    /// # Arguments
    ///
    /// * `key` - The key character just pressed
    ///
    /// # Returns
    ///
    /// A `Task<Message>` produced by the matched Vim action, or
    /// `Task::none()` if the key did not parse into an action
    pub(crate) fn handle_vim_key_msg(&mut self, key: char) -> Task<Message> {
        if !self.vim_enabled {
            return Task::none();
        }

        let previous_mode = self.vim_state.mode();
        let action = self.vim_state.parse_key(key);
        match action {
            Some(VimAction::Mode(mode)) => {
                self.handle_vim_mode(mode, previous_mode)
            }
            Some(VimAction::Motion { motion, count, explicit_count }) => {
                self.handle_vim_motion(motion, count, explicit_count)
            }
            Some(VimAction::Insert { position, count }) => {
                self.handle_vim_insert(position, count)
            }
            Some(VimAction::Operator {
                operator,
                motion,
                count,
                explicit_count,
            }) => self.handle_vim_motion_operator(
                operator,
                motion,
                count,
                explicit_count,
            ),
            Some(VimAction::LineOperator { operator, count }) => {
                let start_line = self.cursors.primary_position().0;
                let end_line = start_line
                    .saturating_add(count.saturating_sub(1))
                    .min(self.buffer.line_count().saturating_sub(1));
                self.handle_vim_line_operator(
                    operator, start_line, end_line, false,
                )
            }
            Some(VimAction::VisualOperator(operator)) => {
                self.handle_vim_visual_operator(operator)
            }
            Some(VimAction::DeleteCharacters { count }) => {
                self.handle_vim_delete_characters(count)
            }
            Some(VimAction::Paste { position, count }) => {
                self.handle_vim_paste(position, count)
            }
            Some(VimAction::Undo { count }) => {
                self.handle_vim_history(false, count)
            }
            Some(VimAction::Redo { count }) => {
                self.handle_vim_history(true, count)
            }
            Some(VimAction::RepeatSearch { reverse }) => {
                self.handle_vim_repeat_search(reverse)
            }
            Some(VimAction::SubmitSearch(query)) => {
                self.handle_vim_search(&query)
            }
            Some(VimAction::SubmitGotoLine(line)) => {
                self.handle_goto_position(line.saturating_sub(1), 0)
            }
            Some(VimAction::WriteFile { exit_vim }) => {
                if exit_vim {
                    self.set_vim_enabled(false);
                }
                Task::done(Message::WriteRequested)
            }
            Some(VimAction::ExitVimMode) => {
                self.set_vim_enabled(false);
                Task::none()
            }
            Some(VimAction::CommandLineChanged) => {
                self.overlay_cache.clear();
                Task::none()
            }
            None => Task::none(),
        }
    }

    fn handle_vim_search(&mut self, query: &str) -> Task<Message> {
        if !self.search_replace_enabled || query.is_empty() {
            return Task::none();
        }

        self.search_state.close();
        self.search_state.set_query(query.to_owned(), &self.buffer);
        if self.search_state.matches.is_empty() {
            self.overlay_cache.clear();
            return Task::none();
        }

        let cursor = self.cursors.primary_position();
        let next_index = self
            .search_state
            .matches
            .partition_point(|item| (item.line, item.col) <= cursor);
        self.search_state.current_match_index =
            Some(if next_index == self.search_state.matches.len() {
                0
            } else {
                next_index
            });

        if let Some(search_match) = self.search_state.current_match() {
            self.cursors.set_single((search_match.line, search_match.col));
        }
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    fn handle_vim_repeat_search(&mut self, reverse: bool) -> Task<Message> {
        let Some(last_search) = self.vim_state.last_search().map(str::to_owned)
        else {
            return Task::none();
        };
        if self.search_state.query != last_search {
            self.search_state.set_query(last_search, &self.buffer);
            self.search_state
                .select_match_near_cursor(self.cursors.primary_position());
        }
        if self.search_state.matches.is_empty() {
            return Task::none();
        }

        if reverse {
            self.search_state.previous_match();
        } else {
            self.search_state.next_match();
        }
        if let Some(search_match) = self.search_state.current_match() {
            self.cursors.set_single((search_match.line, search_match.col));
        }
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    fn handle_vim_delete_characters(&mut self, count: usize) -> Task<Message> {
        let start = self.vim_normal_position(self.cursors.primary_position());
        let line_len = self.buffer.line_len(start.0);
        let end = (start.0, start.1.saturating_add(count).min(line_len));
        self.handle_vim_character_operator(
            VimOperator::Delete,
            start,
            end,
            false,
        )
    }

    fn handle_vim_motion_operator(
        &mut self,
        operator: VimOperator,
        motion: VimMotion,
        count: usize,
        explicit_count: bool,
    ) -> Task<Message> {
        let start = self.vim_normal_position(self.cursors.primary_position());
        if matches!(
            motion,
            VimMotion::Up
                | VimMotion::Down
                | VimMotion::DocumentStart
                | VimMotion::DocumentEnd
        ) {
            let target =
                self.vim_motion_target(start, motion, count, explicit_count);
            return self.handle_vim_line_operator(
                operator,
                start.0.min(target.0),
                start.0.max(target.0),
                false,
            );
        }

        let target =
            self.vim_motion_target(start, motion, count, explicit_count);
        let (range_start, range_end) = match motion {
            VimMotion::Right => (
                start,
                (
                    start.0,
                    start
                        .1
                        .saturating_add(count)
                        .min(self.buffer.line_len(start.0)),
                ),
            ),
            VimMotion::Left => {
                ((start.0, start.1.saturating_sub(count)), start)
            }
            VimMotion::WordEnd | VimMotion::LineEnd => {
                let end = if motion == VimMotion::LineEnd {
                    (start.0, self.buffer.line_len(start.0))
                } else {
                    (
                        target.0,
                        target
                            .1
                            .saturating_add(1)
                            .min(self.buffer.line_len(target.0)),
                    )
                };
                (start.min(end), start.max(end))
            }
            VimMotion::WordForward => {
                let end = if target > start {
                    target
                } else {
                    (start.0, self.buffer.line_len(start.0))
                };
                (start.min(end), start.max(end))
            }
            VimMotion::WordBackward
            | VimMotion::LineStart
            | VimMotion::FirstNonBlank => {
                (start.min(target), start.max(target))
            }
            VimMotion::Up
            | VimMotion::Down
            | VimMotion::DocumentStart
            | VimMotion::DocumentEnd => return Task::none(),
        };

        self.handle_vim_character_operator(
            operator,
            range_start,
            range_end,
            false,
        )
    }

    fn handle_vim_visual_operator(
        &mut self,
        operator: VimOperator,
    ) -> Task<Message> {
        if self.vim_state.mode() == VimMode::VisualLine {
            let (anchor, active) =
                self.vim_state.visual_positions().unwrap_or_else(|| {
                    let position = self.cursors.primary_position();
                    (position, position)
                });
            self.handle_vim_line_operator(
                operator,
                anchor.0.min(active.0),
                anchor.0.max(active.0),
                true,
            )
        } else {
            let Some((start, end)) = self.cursors.primary().selection_range()
            else {
                return Task::none();
            };
            self.handle_vim_character_operator(operator, start, end, true)
        }
    }

    fn handle_vim_character_operator(
        &mut self,
        operator: VimOperator,
        start: (usize, usize),
        end: (usize, usize),
        from_visual: bool,
    ) -> Task<Message> {
        if start == end {
            return Task::none();
        }
        let register = VimRegister {
            text: self.extract_text_range(start, end),
            kind: VimRegisterKind::Characterwise,
        };
        self.apply_vim_operator(operator, start, end, register, from_visual)
    }

    fn handle_vim_line_operator(
        &mut self,
        operator: VimOperator,
        start_line: usize,
        end_line: usize,
        from_visual: bool,
    ) -> Task<Message> {
        let last_line = self.buffer.line_count().saturating_sub(1);
        let start_line = start_line.min(last_line);
        let end_line = end_line.min(last_line).max(start_line);
        let mut text = String::new();
        for line in start_line..=end_line {
            text.push_str(self.buffer.line(line));
            text.push('\n');
        }

        let (start, end) = if end_line < last_line {
            ((start_line, 0), (end_line + 1, 0))
        } else if start_line > 0 {
            (
                (start_line - 1, self.buffer.line_len(start_line - 1)),
                (end_line, self.buffer.line_len(end_line)),
            )
        } else {
            ((0, 0), (end_line, self.buffer.line_len(end_line)))
        };
        self.apply_vim_operator(
            operator,
            start,
            end,
            VimRegister { text, kind: VimRegisterKind::Linewise },
            from_visual,
        )
    }

    fn apply_vim_operator(
        &mut self,
        operator: VimOperator,
        start: (usize, usize),
        end: (usize, usize),
        register: VimRegister,
        from_visual: bool,
    ) -> Task<Message> {
        self.vim_state.register = register;

        if operator == VimOperator::Yank {
            if from_visual {
                self.cursors.set_single(self.vim_normal_position(start));
            } else {
                let position =
                    self.vim_normal_position(self.cursors.primary_position());
                self.cursors.set_single(position);
            }
            self.vim_state.enter_clean_normal_mode();
            self.finish_navigation_operation();
            return self.scroll_to_cursor();
        }

        self.end_grouping_if_active();
        if operator == VimOperator::Change {
            self.ensure_grouping_started();
        }

        self.pre_edit_line = start.0.min(end.0);
        self.pre_edit_last_line = start.0.max(end.0);
        self.capture_lsp_edit_snapshot(&Message::DeleteSelection);

        let cursor_before = self.cursors.primary_position();
        let mut command =
            DeleteRangeCommand::new(&self.buffer, start, end, cursor_before);
        let mut cursor_after = cursor_before;
        command.execute(&mut self.buffer, &mut cursor_after);
        self.history.push(Box::new(command));
        self.cursors.set_single(self.vim_normal_position(cursor_after));

        if operator == VimOperator::Change {
            self.vim_state.enter_insert_mode();
        } else {
            self.vim_state.enter_clean_normal_mode();
        }
        self.finish_edit_operation();
        self.scroll_to_cursor()
    }

    fn handle_vim_paste(
        &mut self,
        position: VimPastePosition,
        count: usize,
    ) -> Task<Message> {
        let register = self.vim_state.register.clone();
        if register.text.is_empty() {
            return Task::none();
        }
        self.end_grouping_if_active();

        let current = self.vim_normal_position(self.cursors.primary_position());
        let (insert_at, text, cursor_after) = match register.kind {
            VimRegisterKind::Characterwise => {
                let insert_at = match position {
                    VimPastePosition::BeforeCursor => current,
                    VimPastePosition::AfterCursor => (
                        current.0,
                        current
                            .1
                            .saturating_add(usize::from(
                                self.buffer.line_len(current.0) > 0,
                            ))
                            .min(self.buffer.line_len(current.0)),
                    ),
                };
                (insert_at, register.text.repeat(count.max(1)), insert_at)
            }
            VimRegisterKind::Linewise => {
                let repeated = register.text.repeat(count.max(1));
                match position {
                    VimPastePosition::BeforeCursor => {
                        ((current.0, 0), repeated, (current.0, 0))
                    }
                    VimPastePosition::AfterCursor
                        if current.0 + 1 < self.buffer.line_count() =>
                    {
                        ((current.0 + 1, 0), repeated, (current.0 + 1, 0))
                    }
                    VimPastePosition::AfterCursor => {
                        let text = format!(
                            "\n{}",
                            repeated.strip_suffix('\n').unwrap_or(&repeated)
                        );
                        (
                            (current.0, self.buffer.line_len(current.0)),
                            text,
                            (current.0 + 1, 0),
                        )
                    }
                }
            }
        };

        self.pre_edit_line = insert_at.0;
        self.pre_edit_last_line = insert_at.0;
        self.capture_lsp_edit_snapshot(&Message::Paste(text.clone()));
        let mut command =
            InsertTextCommand::new(insert_at.0, insert_at.1, text, current)
                .with_cursor_after(cursor_after);
        let mut command_cursor = current;
        command.execute(&mut self.buffer, &mut command_cursor);
        self.history.push(Box::new(command));
        self.cursors.set_single(self.vim_normal_position(command_cursor));
        self.vim_state.enter_clean_normal_mode();
        self.finish_edit_operation();
        self.scroll_to_cursor()
    }

    fn handle_vim_history(
        &mut self,
        redo: bool,
        count: usize,
    ) -> Task<Message> {
        self.end_grouping_if_active();
        self.pre_edit_line = 0;
        self.pre_edit_last_line = usize::MAX;
        self.capture_lsp_edit_snapshot(if redo {
            &Message::Redo
        } else {
            &Message::Undo
        });

        let mut cursor = self.cursors.primary_position();
        let mut changed = false;
        for _ in 0..count.max(1) {
            let applied = if redo {
                self.history.redo(&mut self.buffer, &mut cursor)
            } else {
                self.history.undo(&mut self.buffer, &mut cursor)
            };
            if !applied {
                break;
            }
            changed = true;
        }
        if !changed {
            return Task::none();
        }

        self.cursors.set_single(self.vim_normal_position(cursor));
        self.vim_state.enter_clean_normal_mode();
        self.finish_edit_operation();
        self.scroll_to_cursor()
    }

    fn handle_vim_mode(
        &mut self,
        mode: VimMode,
        previous_mode: VimMode,
    ) -> Task<Message> {
        self.end_grouping_if_active();
        match mode {
            VimMode::Normal => {
                let mut active = self
                    .vim_state
                    .visual_positions()
                    .map(|(_, active)| active)
                    .unwrap_or_else(|| self.cursors.primary_position());
                if previous_mode == VimMode::Insert {
                    active.1 = active.1.saturating_sub(1);
                }
                self.vim_state.clear_visual();
                self.cursors.set_single(self.vim_normal_position(active));
            }
            VimMode::Visual | VimMode::VisualLine => {
                let position =
                    self.vim_normal_position(self.cursors.primary_position());
                self.vim_state.begin_visual(position);
                self.apply_vim_visual_selection(
                    position,
                    position,
                    mode == VimMode::VisualLine,
                );
            }
            VimMode::Insert => {}
        }
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    fn handle_vim_motion(
        &mut self,
        motion: VimMotion,
        count: usize,
        explicit_count: bool,
    ) -> Task<Message> {
        self.end_grouping_if_active();
        match self.vim_state.mode() {
            VimMode::Visual | VimMode::VisualLine => {
                let (anchor, active) =
                    self.vim_state.visual_positions().unwrap_or_else(|| {
                        let position = self.vim_normal_position(
                            self.cursors.primary_position(),
                        );
                        (position, position)
                    });
                let target = self.vim_motion_target(
                    active,
                    motion,
                    count,
                    explicit_count,
                );
                self.vim_state.set_visual_active(target);
                self.apply_vim_visual_selection(
                    anchor,
                    target,
                    self.vim_state.mode() == VimMode::VisualLine,
                );
            }
            VimMode::Normal => {
                let target = self.vim_motion_target(
                    self.cursors.primary_position(),
                    motion,
                    count,
                    explicit_count,
                );
                self.cursors.set_single(target);
                self.overlay_cache.clear();
            }
            VimMode::Insert => return Task::none(),
        }
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    fn handle_vim_insert(
        &mut self,
        position: VimInsertPosition,
        count: usize,
    ) -> Task<Message> {
        self.end_grouping_if_active();
        let current = self
            .vim_state
            .visual_positions()
            .map(|(_, active)| active)
            .unwrap_or_else(|| self.cursors.primary_position());
        self.vim_state.clear_visual();
        let current = self.vim_normal_position(current);
        self.cursors.set_single(current);
        self.ensure_grouping_started();

        match position {
            VimInsertPosition::BeforeCursor => {}
            VimInsertPosition::AfterCursor => {
                let line_len = self.buffer.line_len(current.0);
                self.cursors.primary_mut().position.1 =
                    current.1.saturating_add(1).min(line_len);
            }
            VimInsertPosition::FirstNonBlank => {
                self.cursors.primary_mut().position.1 = self
                    .buffer
                    .line(current.0)
                    .chars()
                    .position(|ch| !ch.is_whitespace())
                    .unwrap_or(0);
            }
            VimInsertPosition::EndOfLine => {
                self.cursors.primary_mut().position.1 =
                    self.buffer.line_len(current.0);
            }
            VimInsertPosition::NewLineBelow => {
                self.cursors.primary_mut().position.1 =
                    self.buffer.line_len(current.0);
                for _ in 0..count.max(1) {
                    let _ = self.update(&Message::Enter);
                }
            }
            VimInsertPosition::NewLineAbove => {
                self.cursors.primary_mut().position.1 = 0;
                let line = current.0;
                for _ in 0..count.max(1) {
                    let _ = self.update(&Message::Enter);
                    self.cursors.set_single((line, 0));
                }
            }
        }

        self.overlay_cache.clear();
        self.reset_cursor_blink();
        self.scroll_to_cursor()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_editor::VimMode;
    use crate::canvas_editor::lsp;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn vim_keys(editor: &mut CodeEditor, keys: &str) {
        for key in keys.chars() {
            let _ = editor.update(&Message::VimKey(key));
        }
    }

    fn focus_editor(editor: &mut CodeEditor) {
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;
    }

    fn assert_vim_delete(
        content: &str,
        cursor: (usize, usize),
        keys: &str,
        expected: &str,
        register: &str,
    ) {
        let mut editor = CodeEditor::new(content, "txt").with_vim_enabled(true);
        editor.cursors.set_single(cursor);
        vim_keys(&mut editor, keys);
        assert_eq!(editor.content(), expected, "keys: {keys}");
        assert_eq!(editor.vim_state.register.text, register, "keys: {keys}");
    }

    #[derive(Default)]
    struct VimTestLspClient {
        changes: Rc<RefCell<Vec<Vec<lsp::LspTextChange>>>>,
    }

    impl lsp::LspClient for VimTestLspClient {
        fn did_change(
            &mut self,
            _document: &lsp::LspDocument,
            changes: &[lsp::LspTextChange],
        ) {
            self.changes.borrow_mut().push(changes.to_vec());
        }
    }

    #[test]
    fn test_vim_navigation_normal_key_does_not_insert() {
        let mut editor = CodeEditor::new("abc", "txt").with_vim_enabled(true);

        vim_keys(&mut editor, "l");

        assert_eq!(editor.content(), "abc");
        assert_eq!(editor.cursors.primary_position(), (0, 1));

        let mut standard = CodeEditor::new("abc", "txt");
        focus_editor(&mut standard);
        let _ = standard.update(&Message::CharacterInput('l'));
        assert_eq!(standard.content(), "labc");
    }

    #[test]
    fn test_vim_navigation_insert_and_escape_round_trip() {
        let mut editor = CodeEditor::new("abc", "txt").with_vim_enabled(true);
        focus_editor(&mut editor);

        vim_keys(&mut editor, "i");
        assert_eq!(editor.vim_mode(), Some(VimMode::Insert));
        let _ = editor.update(&Message::CharacterInput('X'));
        assert_eq!(editor.content(), "Xabc");

        let _ = editor.update(&Message::VimKey('\u{1b}'));
        assert_eq!(editor.vim_mode(), Some(VimMode::Normal));
        vim_keys(&mut editor, "l");
        assert_eq!(editor.content(), "Xabc");
        assert_eq!(editor.cursors.primary_position(), (0, 1));
    }

    #[test]
    fn test_vim_navigation_counted_word_and_line_motions() {
        let mut editor =
            CodeEditor::new("one two\nthree four\nfive six", "txt")
                .with_vim_enabled(true);

        vim_keys(&mut editor, "2w");
        assert_eq!(editor.cursors.primary_position(), (1, 0));
        vim_keys(&mut editor, "e");
        assert_eq!(editor.cursors.primary_position(), (1, 4));
        vim_keys(&mut editor, "b");
        assert_eq!(editor.cursors.primary_position(), (1, 0));
        vim_keys(&mut editor, "G");
        assert_eq!(editor.cursors.primary_position(), (2, 0));
        vim_keys(&mut editor, "gg");
        assert_eq!(editor.cursors.primary_position(), (0, 0));
        vim_keys(&mut editor, "2j");
        assert_eq!(editor.cursors.primary_position(), (2, 0));
        vim_keys(&mut editor, "k$");
        assert_eq!(editor.cursors.primary_position(), (1, 9));
        vim_keys(&mut editor, "0");
        assert_eq!(editor.cursors.primary_position(), (1, 0));

        let mut folded = CodeEditor::new(
            "fn main() {\n    let x = 1;\n    if x > 0 {\n        print();\n    }\n}",
            "rs",
        )
        .with_vim_enabled(true);
        folded.toggle_fold(0);
        vim_keys(&mut folded, "j");
        assert_eq!(folded.cursors.primary_position(), (5, 0));
    }

    #[test]
    fn test_vim_navigation_visual_and_visual_line_ranges() {
        let mut editor = CodeEditor::new("abcd\nefgh\nijkl\nmnop", "txt")
            .with_vim_enabled(true);
        editor.cursors.set_single((0, 1));

        vim_keys(&mut editor, "vl");
        assert_eq!(editor.vim_mode(), Some(VimMode::Visual));
        assert_eq!(
            editor.cursors.primary().selection_range(),
            Some(((0, 1), (0, 3)))
        );

        let _ = editor.update(&Message::VimKey('\u{1b}'));
        assert_eq!(editor.vim_mode(), Some(VimMode::Normal));
        assert!(editor.cursors.primary().anchor.is_none());

        editor.cursors.set_single((1, 2));
        vim_keys(&mut editor, "Vj");
        assert_eq!(editor.vim_mode(), Some(VimMode::VisualLine));
        assert_eq!(
            editor.cursors.primary().selection_range(),
            Some(((1, 0), (3, 0)))
        );
    }

    #[test]
    fn test_vim_navigation_unicode_and_empty_line_bounds() {
        let mut editor =
            CodeEditor::new("你🙂好\n\nz", "txt").with_vim_enabled(true);

        vim_keys(&mut editor, "lll");
        assert_eq!(editor.cursors.primary_position(), (0, 2));
        vim_keys(&mut editor, "j");
        assert_eq!(editor.cursors.primary_position(), (1, 0));
        vim_keys(&mut editor, "j");
        assert_eq!(editor.cursors.primary_position(), (2, 0));
        vim_keys(&mut editor, "k");
        assert_eq!(editor.cursors.primary_position(), (1, 0));
        vim_keys(&mut editor, "k$");
        assert_eq!(editor.cursors.primary_position(), (0, 2));
        vim_keys(&mut editor, "0");
        assert_eq!(editor.cursors.primary_position(), (0, 0));
    }

    #[test]
    fn test_vim_navigation_ime_only_commits_in_insert() {
        let mut editor = CodeEditor::new("abc", "txt").with_vim_enabled(true);

        let _ = editor.update(&Message::ImeCommit("中".to_owned()));
        assert_eq!(editor.content(), "abc");

        vim_keys(&mut editor, "i");
        let _ = editor.update(&Message::ImeCommit("中".to_owned()));
        assert_eq!(editor.content(), "中abc");

        let mut standard = CodeEditor::new("abc", "txt");
        let _ = standard.update(&Message::ImeCommit("中".to_owned()));
        assert_eq!(standard.content(), "中abc");
    }

    #[test]
    fn test_vim_navigation_collapses_and_blocks_extra_cursors() {
        let mut editor = CodeEditor::new("same\nsame\nsame", "txt");
        editor.cursors.add_cursor((1, 0));
        assert_eq!(editor.cursors.len(), 2);

        editor.set_vim_enabled(true);
        assert_eq!(editor.cursors.len(), 1);

        let _ = editor.update(&Message::AddCursorBelow);
        let _ = editor.update(&Message::SelectNextOccurrence);
        let _ = editor.update(&Message::SelectNextOccurrence);
        let _ = editor.update(&Message::AltClick(iced::Point::new(
            editor.gutter_width() + 5.0,
            editor.line_height,
        )));
        assert_eq!(editor.cursors.len(), 1);
    }

    #[test]
    fn test_vim_editing_x_and_count() {
        let mut editor =
            CodeEditor::new("abcdef", "txt").with_vim_enabled(true);

        vim_keys(&mut editor, "x");
        assert_eq!(editor.content(), "bcdef");
        assert_eq!(editor.vim_state.register.text, "a");
        assert_eq!(
            editor.vim_state.register.kind,
            VimRegisterKind::Characterwise
        );

        vim_keys(&mut editor, "2x");
        assert_eq!(editor.content(), "def");
        assert_eq!(editor.vim_state.register.text, "bc");
        assert_eq!(editor.cursors.primary_position(), (0, 0));
    }

    #[test]
    fn test_vim_editing_delete_change_yank_motions() {
        let mut deleted =
            CodeEditor::new("one two three", "txt").with_vim_enabled(true);
        vim_keys(&mut deleted, "dw");
        assert_eq!(deleted.content(), "two three");
        assert_eq!(deleted.vim_state.register.text, "one ");

        let mut yanked =
            CodeEditor::new("one two", "txt").with_vim_enabled(true);
        vim_keys(&mut yanked, "yw");
        assert_eq!(yanked.content(), "one two");
        assert_eq!(yanked.vim_state.register.text, "one ");

        let mut changed =
            CodeEditor::new("one two", "txt").with_vim_enabled(true);
        focus_editor(&mut changed);
        vim_keys(&mut changed, "ce");
        assert_eq!(changed.content(), " two");
        assert_eq!(changed.vim_state.register.text, "one");
        assert_eq!(changed.vim_mode(), Some(VimMode::Insert));
        let _ = changed.update(&Message::CharacterInput('X'));
        vim_keys(&mut changed, "\u{1b}");
        assert_eq!(changed.content(), "X two");

        assert_vim_delete("abc", (0, 2), "dh", "ac", "b");
        assert_vim_delete("abc", (0, 1), "dl", "ac", "b");
        assert_vim_delete("abc def", (0, 6), "db", "abc f", "de");
        assert_vim_delete("abcdef", (0, 3), "d0", "def", "abc");
        assert_vim_delete("  abc", (0, 4), "d^", "  c", "ab");
        assert_vim_delete("abcde", (0, 2), "d$", "ab", "cde");
        assert_vim_delete(
            "one\ntwo\nthree\nfour",
            (2, 0),
            "dgg",
            "four",
            "one\ntwo\nthree\n",
        );
        assert_vim_delete(
            "one\ntwo\nthree\nfour",
            (1, 0),
            "dG",
            "one",
            "two\nthree\nfour\n",
        );
    }

    #[test]
    fn test_vim_editing_doubled_line_operators() {
        let mut deleted =
            CodeEditor::new("one\ntwo\nthree", "txt").with_vim_enabled(true);
        vim_keys(&mut deleted, "dd");
        assert_eq!(deleted.content(), "two\nthree");
        assert_eq!(deleted.vim_state.register.text, "one\n");
        assert_eq!(deleted.vim_state.register.kind, VimRegisterKind::Linewise);

        let mut yanked =
            CodeEditor::new("one\ntwo\nthree", "txt").with_vim_enabled(true);
        yanked.cursors.set_single((1, 1));
        vim_keys(&mut yanked, "yy");
        assert_eq!(yanked.content(), "one\ntwo\nthree");
        assert_eq!(yanked.vim_state.register.text, "two\n");
        assert_eq!(yanked.vim_state.register.kind, VimRegisterKind::Linewise);

        let mut changed =
            CodeEditor::new("one\ntwo\nthree", "txt").with_vim_enabled(true);
        changed.cursors.set_single((1, 1));
        vim_keys(&mut changed, "cc");
        assert_eq!(changed.content(), "one\nthree");
        assert_eq!(changed.vim_state.register.text, "two\n");
        assert_eq!(changed.vim_mode(), Some(VimMode::Insert));

        assert_vim_delete(
            "one\ntwo\nthree\nfour",
            (1, 0),
            "dk",
            "three\nfour",
            "one\ntwo\n",
        );
        assert_vim_delete(
            "one\ntwo\nthree\nfour",
            (1, 0),
            "dj",
            "one\nfour",
            "two\nthree\n",
        );
        assert_vim_delete(
            "one\ntwo\nthree",
            (0, 0),
            "2dd",
            "three",
            "one\ntwo\n",
        );
    }

    #[test]
    fn test_vim_editing_visual_operators() {
        let mut deleted =
            CodeEditor::new("abcd\nefgh", "txt").with_vim_enabled(true);
        deleted.cursors.set_single((0, 1));
        vim_keys(&mut deleted, "vld");
        assert_eq!(deleted.content(), "ad\nefgh");
        assert_eq!(deleted.vim_state.register.text, "bc");
        assert_eq!(
            deleted.vim_state.register.kind,
            VimRegisterKind::Characterwise
        );
        assert_eq!(deleted.vim_mode(), Some(VimMode::Normal));

        let mut yanked =
            CodeEditor::new("one\ntwo\nthree", "txt").with_vim_enabled(true);
        yanked.cursors.set_single((1, 1));
        vim_keys(&mut yanked, "Vjy");
        assert_eq!(yanked.content(), "one\ntwo\nthree");
        assert_eq!(yanked.vim_state.register.text, "two\nthree\n");
        assert_eq!(yanked.vim_state.register.kind, VimRegisterKind::Linewise);
        assert_eq!(yanked.vim_mode(), Some(VimMode::Normal));

        let mut changed = CodeEditor::new("abcd", "txt").with_vim_enabled(true);
        focus_editor(&mut changed);
        changed.cursors.set_single((0, 1));
        vim_keys(&mut changed, "vlc");
        let _ = changed.update(&Message::CharacterInput('X'));
        vim_keys(&mut changed, "\u{1b}");
        assert_eq!(changed.content(), "aXd");
        assert_eq!(changed.vim_state.register.text, "bc");
        assert_eq!(changed.history.undo_count(), 1);
    }

    #[test]
    fn test_vim_editing_characterwise_and_linewise_paste() {
        let mut characterwise =
            CodeEditor::new("abc", "txt").with_vim_enabled(true);
        vim_keys(&mut characterwise, "yl2lp");
        assert_eq!(characterwise.content(), "abca");
        assert_eq!(characterwise.cursors.primary_position(), (0, 3));

        let mut characterwise_before =
            CodeEditor::new("abc", "txt").with_vim_enabled(true);
        vim_keys(&mut characterwise_before, "yl2lP");
        assert_eq!(characterwise_before.content(), "abac");
        assert_eq!(characterwise_before.cursors.primary_position(), (0, 2));

        let mut linewise =
            CodeEditor::new("one\ntwo", "txt").with_vim_enabled(true);
        vim_keys(&mut linewise, "yyp");
        assert_eq!(linewise.content(), "one\none\ntwo");
        assert_eq!(linewise.cursors.primary_position(), (1, 0));

        let mut linewise_before =
            CodeEditor::new("one\ntwo", "txt").with_vim_enabled(true);
        linewise_before.cursors.set_single((1, 0));
        vim_keys(&mut linewise_before, "yyP");
        assert_eq!(linewise_before.content(), "one\ntwo\ntwo");
        assert_eq!(linewise_before.cursors.primary_position(), (1, 0));
    }

    #[test]
    fn test_vim_editing_paste_undo_restores_buffer() {
        // Vim paste rests the caret on the pasted text rather than after it;
        // undo must still remove exactly what was pasted.
        for keys in ["ylp", "ylP", "yl2lp", "yl2lP"] {
            let mut editor =
                CodeEditor::new("abc", "txt").with_vim_enabled(true);
            vim_keys(&mut editor, keys);
            assert_ne!(editor.content(), "abc", "keys: {keys}");

            vim_keys(&mut editor, "u");
            assert_eq!(editor.content(), "abc", "keys: {keys}");
        }

        for keys in ["yyp", "yyP"] {
            let mut editor =
                CodeEditor::new("one\ntwo", "txt").with_vim_enabled(true);
            vim_keys(&mut editor, keys);
            assert_ne!(editor.content(), "one\ntwo", "keys: {keys}");

            vim_keys(&mut editor, "u");
            assert_eq!(editor.content(), "one\ntwo", "keys: {keys}");
        }
    }

    #[test]
    fn test_vim_editing_operator_counts_multiply() {
        let mut editor =
            CodeEditor::new("one two three four five six seven", "txt")
                .with_vim_enabled(true);

        vim_keys(&mut editor, "2d3w");

        assert_eq!(editor.content(), "seven");
        assert_eq!(
            editor.vim_state.register.text,
            "one two three four five six "
        );
    }

    #[test]
    fn test_vim_editing_undo_redo_is_one_command() {
        let original = "one two three";
        let mut editor =
            CodeEditor::new(original, "txt").with_vim_enabled(true);
        focus_editor(&mut editor);

        vim_keys(&mut editor, "cw");
        let _ = editor.update(&Message::CharacterInput('X'));
        let _ = editor.update(&Message::CharacterInput('Y'));
        vim_keys(&mut editor, "\u{1b}");
        assert_eq!(editor.content(), "XYtwo three");
        assert_eq!(editor.history.undo_count(), 1);

        vim_keys(&mut editor, "u");
        assert_eq!(editor.content(), original);
        assert_eq!(editor.history.redo_count(), 1);

        vim_keys(&mut editor, "\u{12}");
        assert_eq!(editor.content(), "XYtwo three");
        assert_eq!(editor.history.undo_count(), 1);

        let mut opened = CodeEditor::new("one", "txt").with_vim_enabled(true);
        focus_editor(&mut opened);
        vim_keys(&mut opened, "o");
        let _ = opened.update(&Message::CharacterInput('X'));
        vim_keys(&mut opened, "\u{1b}");
        assert_eq!(opened.content(), "one\nX");
        assert_eq!(opened.history.undo_count(), 1);
        vim_keys(&mut opened, "u");
        assert_eq!(opened.content(), "one");
    }

    #[test]
    fn test_vim_editing_emits_incremental_lsp_change() {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let client = VimTestLspClient { changes: Rc::clone(&changes) };
        let content = (0..10)
            .map(|line| format!("line{line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut editor = CodeEditor::new(&content, "rs").with_vim_enabled(true);
        editor.attach_lsp(
            Box::new(client),
            lsp::LspDocument::new("file:///vim.rs", "rust"),
        );
        editor.cursors.set_single((5, 2));

        vim_keys(&mut editor, "x");

        let changes = changes.borrow();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].len(), 1);
        let change = &changes[0][0];
        assert_eq!(change.range.start.line, 4);
        assert_eq!(change.range.start.character, 0);
        assert_eq!(change.range.end.line, 7);
        assert_eq!(change.range.end.character, 0);
        assert_eq!(change.text, "line4\nlie5\nline6\n");
    }
}
