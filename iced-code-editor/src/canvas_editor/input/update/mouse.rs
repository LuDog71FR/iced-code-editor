//! Mouse click, drag, release, double/triple-click, and context-menu message handlers.

use iced::Task;

use crate::canvas_editor::{CodeEditor, Message, VimMode};

impl CodeEditor {
    /// Synchronises the active search result with a manual primary-cursor
    /// position or selection.
    pub(crate) fn sync_search_match_from_primary_cursor(&mut self) {
        if !self.search_matches_visible() || self.search_state.query.is_empty()
        {
            return;
        }

        let primary = self.cursors.primary();
        let cursor = primary.position;
        let selection = primary.selection_range();
        if self.search_state.select_match_at_cursor(cursor, selection) {
            self.overlay_cache.clear();
        }
    }

    /// Handles mouse click operations.
    ///
    /// Sets focus, ends command grouping, positions cursor, starts selection tracking.
    ///
    /// # Arguments
    ///
    /// * `point` - The click position
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none() as no scrolling is needed)
    pub(crate) fn handle_mouse_click_msg(
        &mut self,
        point: iced::Point,
    ) -> Task<Message> {
        // Capture focus when clicked using the new focus method
        self.request_focus();

        // Set internal canvas focus state
        self.has_canvas_focus = true;

        // End grouping on mouse click
        self.end_grouping_if_active();

        // Regular click collapses any multi-cursor state to a single cursor
        // positioned at the click location.
        self.cursors.remove_all_but_primary();

        self.handle_mouse_click(point);
        self.reset_cursor_blink();
        // Clear selection on click, then set anchor for potential drag selection
        self.clear_selection();
        self.is_dragging = true;
        self.drag_viewport_point = None;
        self.cursors.primary_mut().set_anchor();
        self.sync_search_match_from_primary_cursor();

        // Show cursor when focused
        self.show_cursor = true;

        Task::none()
    }

    /// Handles mouse drag operations for selection.
    ///
    /// # Arguments
    ///
    /// * `point` - The drag position
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none() as no scrolling is needed)
    pub(crate) fn handle_mouse_drag_msg(
        &mut self,
        point: iced::Point,
    ) -> Task<Message> {
        if self.is_dragging {
            self.drag_viewport_point =
                Some(iced::Point::new(point.x, point.y - self.viewport_scroll));
            let before_pos = self.cursors.primary_position();
            self.extend_drag_selection();
            if self.cursors.primary_position() != before_pos {
                // Mouse move events can be very frequent. Only invalidate the
                // overlay cache if the drag actually changed selection/cursor.
                self.overlay_cache.clear();
            }
        }
        Task::none()
    }

    /// Vertical edge-scroll speed in pixels per second.
    pub(crate) fn selection_scroll_speed(&self) -> f32 {
        if !self.is_dragging || self.viewport_height <= 0.0 {
            return 0.0;
        }
        let Some(point) = self.drag_viewport_point else {
            return 0.0;
        };
        let edge = (self.line_height * 0.5).min(self.viewport_height / 4.0);
        let distance = if point.y < edge {
            point.y - edge
        } else if point.y > self.viewport_height - edge {
            point.y - (self.viewport_height - edge)
        } else {
            return 0.0;
        };
        let height = self.visual_lines_cached(self.viewport_width).len() as f32
            * self.line_height;
        let max_scroll = (height - self.viewport_height).max(0.0);
        if (distance < 0.0 && self.viewport_scroll <= 0.0)
            || (distance > 0.0 && self.viewport_scroll >= max_scroll)
        {
            return 0.0;
        }
        // Ease into scrolling in the narrow edge zone; accelerate continuously
        // outside it, scaled to viewport height for short and tall editors.
        let outside_lines = distance.abs() / self.line_height;
        let ramp = (outside_lines / 0.5).min(1.0);
        let viewport_lines = self.viewport_height / self.line_height;
        let lines_per_second = (viewport_lines.max(30.0)
            * (1.0 + outside_lines + outside_lines * outside_lines * 0.25))
            .min(300.0);
        distance.signum() * ramp * ramp * lines_per_second * self.line_height
    }

    pub(crate) fn handle_selection_auto_scroll(
        &mut self,
        seconds: f32,
    ) -> Task<Message> {
        if !self.has_focus() {
            return Task::none();
        }
        self.advance_selection_auto_scroll(seconds)
    }

    /// Advances drag geometry independently of application-wide focus routing.
    fn advance_selection_auto_scroll(&mut self, seconds: f32) -> Task<Message> {
        let speed = self.selection_scroll_speed();
        if speed.abs() < f32::EPSILON {
            return Task::none();
        }
        let height = self.visual_lines_cached(self.viewport_width).len() as f32
            * self.line_height;
        let offset = (self.viewport_scroll + speed * seconds.clamp(0.0, 0.05))
            .clamp(0.0, (height - self.viewport_height).max(0.0));
        self.update_drag_viewport(
            offset,
            self.viewport_height,
            self.viewport_width,
        );
        iced::widget::operation::scroll_to(
            self.scrollable_id.clone(),
            iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: offset },
        )
    }

    pub(crate) fn extend_drag_selection(&mut self) {
        if self.is_dragging
            && let Some(point) = self.drag_viewport_point
        {
            let before = self.cursors.primary_position();
            self.handle_mouse_drag(iced::Point::new(
                point.x.max(self.gutter_width() + 5.0),
                self.viewport_scroll
                    + point.y.clamp(0.0, (self.viewport_height - 1.0).max(0.0)),
            ));
            if self.cursors.primary_position() != before {
                self.overlay_cache.clear();
            }
        }
    }

    /// Handles mouse release operations.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none() as no scrolling is needed)
    pub(crate) fn handle_mouse_release_msg(&mut self) -> Task<Message> {
        self.is_dragging = false;
        self.drag_viewport_point = None;
        if self.vim_enabled {
            if self.cursors.primary().has_selection() {
                let anchor = self.cursors.primary().anchor.unwrap_or_default();
                let position = self.cursors.primary_position();
                let active = if position >= anchor && position.1 > 0 {
                    (position.0, position.1 - 1)
                } else {
                    position
                };
                let anchor = self.vim_normal_position(anchor);
                let active = self.vim_normal_position(active);
                self.vim_state.set_mode_from_mouse(VimMode::Visual);
                self.vim_state.begin_visual(anchor);
                self.vim_state.set_visual_active(active);
            } else {
                let position =
                    self.vim_normal_position(self.cursors.primary_position());
                self.cursors.set_single(position);
            }
            self.overlay_cache.clear();
        }
        self.sync_search_match_from_primary_cursor();
        Task::none()
    }

    /// Handles a double-click: selects the word under the cursor.
    ///
    /// If the click lands outside any word (e.g. on whitespace), the
    /// selection is cleared and the caret is simply placed there.
    pub(crate) fn handle_double_click_msg(
        &mut self,
        point: iced::Point,
    ) -> Task<Message> {
        self.request_focus();
        self.has_canvas_focus = true;
        self.end_grouping_if_active();
        self.cursors.remove_all_but_primary();
        if let Some((line, col)) = self.calculate_cursor_from_point(point) {
            let line_content = self.buffer.line(line);
            let start = Self::word_start_in_line(line_content, col);
            let end = Self::word_end_in_line(line_content, col);
            let cursor = self.cursors.primary_mut();
            if start < end {
                cursor.anchor = Some((line, start));
                cursor.position = (line, end);
            } else {
                cursor.anchor = None;
                cursor.position = (line, col);
            }
        }
        self.is_dragging = false;
        self.drag_viewport_point = None;
        self.show_cursor = true;
        self.reset_cursor_blink();
        self.overlay_cache.clear();
        self.sync_search_match_from_primary_cursor();
        Task::none()
    }

    /// Handles a triple-click: selects the whole line under the cursor.
    pub(crate) fn handle_triple_click_msg(
        &mut self,
        point: iced::Point,
    ) -> Task<Message> {
        self.request_focus();
        self.has_canvas_focus = true;
        self.end_grouping_if_active();
        self.cursors.remove_all_but_primary();
        if let Some((line, _col)) = self.calculate_cursor_from_point(point) {
            let line_len = self.buffer.line_len(line);
            let cursor = self.cursors.primary_mut();
            cursor.anchor = Some((line, 0));
            cursor.position = (line, line_len);
        }
        self.is_dragging = false;
        self.drag_viewport_point = None;
        self.show_cursor = true;
        self.reset_cursor_blink();
        self.overlay_cache.clear();
        self.sync_search_match_from_primary_cursor();
        Task::none()
    }

    /// Handles a right-click before the context menu is displayed.
    ///
    /// A click inside any existing selection preserves it so Cut and Copy act
    /// on the selected text. A click elsewhere collapses the selection and
    /// moves the caret to the clicked position.
    pub(crate) fn handle_context_menu_requested_msg(
        &mut self,
        point: iced::Point,
    ) -> Task<Message> {
        self.request_focus();
        self.has_canvas_focus = true;
        self.focus_locked = false;
        self.show_cursor = true;
        self.is_dragging = false;
        self.drag_viewport_point = None;
        self.end_grouping_if_active();

        if let Some(position) = self.calculate_cursor_from_point(point) {
            let inside_selection = self.cursors.iter().any(|cursor| {
                cursor.selection_range().is_some_and(|(start, end)| {
                    (start..=end).contains(&position)
                })
            });

            if !inside_selection {
                self.cursors.set_single(position);
                self.overlay_cache.clear();
            }
        }

        self.reset_cursor_blink();
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_editor::ArrowDirection;

    #[test]
    fn drag_speed_ramps_and_stops_inside_or_at_document_end() {
        let content = "hello world\n".repeat(200);
        let mut editor = CodeEditor::new(&content, "txt");
        editor.viewport_height = editor.line_height * 10.0;
        editor.viewport_width = 800.0;
        let x = editor.gutter_width() + 10.0;
        let _ = editor.handle_mouse_click_msg(iced::Point::new(x, 5.0));
        let bottom = editor.viewport_height;
        let _ = editor.handle_mouse_drag_msg(iced::Point::new(
            x,
            bottom - editor.line_height * 0.4,
        ));
        let slow = editor.selection_scroll_speed();
        let _ = editor.handle_mouse_drag_msg(iced::Point::new(
            x,
            bottom + editor.line_height,
        ));
        let fast = editor.selection_scroll_speed();
        assert!(slow > 0.0 && fast > slow);
        assert!(editor.cursors.primary_position().0 <= 10);
        let _ = editor.handle_mouse_drag_msg(iced::Point::new(x, bottom / 2.0));
        assert!(editor.selection_scroll_speed().abs() < f32::EPSILON);
        editor.viewport_scroll =
            (editor.visual_lines_cached(editor.viewport_width).len() as f32
                * editor.line_height
                - editor.viewport_height)
                .max(0.0);
        editor.drag_viewport_point = Some(iced::Point::new(x, bottom + 100.0));
        assert!(editor.selection_scroll_speed().abs() < f32::EPSILON);
    }

    #[test]
    fn focus_locked_editor_does_not_auto_scroll() {
        let mut editor = CodeEditor::new(&"hello world\n".repeat(200), "txt");
        editor.viewport_height = editor.line_height * 10.0;
        editor.viewport_width = 800.0;
        let x = editor.gutter_width() + 10.0;
        let _ = editor.handle_mouse_click_msg(iced::Point::new(x, 5.0));
        let _ = editor.handle_mouse_drag_msg(iced::Point::new(
            x,
            editor.viewport_height + 5.0,
        ));
        assert!(editor.selection_scroll_speed() > 0.0);
        editor.focus_locked = true;
        let _ = editor.handle_selection_auto_scroll(0.016);
        assert!(editor.viewport_scroll.abs() < f32::EPSILON);
    }

    #[test]
    fn drag_scroll_distance_is_independent_of_frame_rate() {
        let content = "hello world\n".repeat(200);
        // Exercise scroll geometry without the global focus ID, which other
        // parallel editor tests legitimately change while this test is running.
        let run = |step: f32, frames: usize| {
            let mut editor = CodeEditor::new(&content, "txt");
            editor.viewport_height = editor.line_height * 10.0;
            editor.viewport_width = 800.0;
            let x = editor.gutter_width() + 10.0;
            let _ = editor.handle_mouse_click_msg(iced::Point::new(x, 5.0));
            let _ = editor.handle_mouse_drag_msg(iced::Point::new(
                x,
                editor.viewport_height + 5.0,
            ));
            for _ in 0..frames {
                let _ = editor.advance_selection_auto_scroll(step);
            }
            editor.viewport_scroll
        };
        assert!((run(0.008, 120) - run(0.016, 60)).abs() < 0.1);
    }

    #[test]
    fn drag_edge_scroll_extends_selection_and_stops_on_release() {
        let content = (0..100).map(|_| "hello world\n").collect::<String>();
        let mut editor = CodeEditor::new(&content, "txt");
        editor.viewport_height = editor.line_height * 5.0;
        editor.viewport_width = 800.0;
        let x = editor.gutter_width() + 10.0;
        let _ = editor.handle_mouse_click_msg(iced::Point::new(x, 5.0));
        let anchor = editor.cursors.primary().anchor;
        let _ = editor.handle_mouse_drag_msg(iced::Point::new(
            x,
            editor.viewport_height + 20.0,
        ));
        let before = editor.cursors.primary_position();
        for _ in 0..10 {
            let _ = editor.advance_selection_auto_scroll(0.016);
        }
        assert!(editor.viewport_scroll > 0.0);
        assert!(editor.cursors.primary_position().0 > before.0);
        assert_eq!(editor.cursors.primary().anchor, anchor);
        let _ = editor.handle_mouse_release_msg();
        let offset = editor.viewport_scroll;
        let _ = editor.advance_selection_auto_scroll(0.016);
        assert!((editor.viewport_scroll - offset).abs() < f32::EPSILON);
    }

    #[test]
    fn test_manual_search_match_selection_updates_current_index() {
        let mut editor =
            CodeEditor::new("foo bar foo baz foo\nno result", "txt");
        editor.search_state.open_search();
        editor.search_state.set_query("foo".to_owned(), &editor.buffer);
        assert_eq!(editor.search_state.current_match_index, Some(0));

        let text_start = editor.gutter_width() + 5.0;
        let char_width = editor.char_width;
        let line_y = editor.line_height / 2.0;
        let point_at_col = |col: usize| {
            iced::Point::new(text_start + char_width * col as f32, line_y)
        };

        let _ = editor.update(&Message::MouseClick(point_at_col(8)));
        let _ = editor.update(&Message::MouseDrag(point_at_col(11)));
        let _ = editor.update(&Message::MouseRelease);

        assert_eq!(
            editor.cursors.primary().selection_range(),
            Some(((0, 8), (0, 11)))
        );
        assert_eq!(editor.search_state.current_match_index, Some(1));

        let no_match_line = iced::Point::new(
            text_start + char_width * 4.0,
            editor.line_height * 1.5,
        );
        let _ = editor.update(&Message::MouseClick(no_match_line));
        let _ = editor.update(&Message::MouseRelease);
        assert_eq!(editor.search_state.current_match_index, Some(1));

        let _ = editor.update(&Message::FindNext);
        assert_eq!(editor.search_state.current_match_index, Some(2));
        let _ = editor.update(&Message::FindPrevious);
        assert_eq!(editor.search_state.current_match_index, Some(1));
    }

    #[test]
    fn test_manual_line_selection_updates_current_search_index() {
        let mut editor =
            CodeEditor::new("foo\nprefix foo suffix\nlast foo", "txt");
        editor.search_state.open_search();
        editor.search_state.set_query("foo".to_owned(), &editor.buffer);
        assert_eq!(editor.search_state.current_match_index, Some(0));

        let line_start = iced::Point::new(
            editor.gutter_width() + 5.0,
            editor.line_height * 1.5,
        );
        let _ = editor.update(&Message::MouseClick(line_start));
        let _ = editor.update(&Message::MouseRelease);

        assert_eq!(editor.cursors.primary_position(), (1, 0));
        assert_eq!(editor.search_state.current_match_index, Some(1));

        let _ = editor.update(&Message::FindNext);
        assert_eq!(editor.search_state.current_match_index, Some(2));

        let mut keyboard_editor =
            CodeEditor::new("foo\nprefix foo suffix\nlast foo", "txt");
        keyboard_editor.search_state.open_search();
        keyboard_editor
            .search_state
            .set_query("foo".to_owned(), &keyboard_editor.buffer);

        let _ = keyboard_editor
            .update(&Message::ArrowKey(ArrowDirection::Down, false));

        assert_eq!(keyboard_editor.cursors.primary_position(), (1, 0));
        assert_eq!(keyboard_editor.search_state.current_match_index, Some(1));
    }

    #[test]
    fn test_mouse_click_gains_focus() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.has_canvas_focus = false;
        editor.show_cursor = false;

        let _ =
            editor.update(&Message::MouseClick(iced::Point::new(100.0, 10.0)));

        assert!(editor.has_canvas_focus);
        assert!(editor.show_cursor);
    }

    #[test]
    fn test_context_click_inside_selection_preserves_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (0, 5);
        let point = iced::Point::new(
            editor.gutter_width() + 5.0 + editor.char_width * 2.0,
            editor.line_height / 2.0,
        );

        let _ = editor.update(&Message::ContextMenuRequested(point));

        assert_eq!(
            editor.cursors.primary().selection_range(),
            Some(((0, 0), (0, 5)))
        );
    }

    #[test]
    fn test_context_click_outside_selection_moves_caret() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (0, 5);
        let point = iced::Point::new(
            editor.gutter_width() + 5.0 + editor.char_width * 8.0,
            editor.line_height / 2.0,
        );

        let _ = editor.update(&Message::ContextMenuRequested(point));

        assert_eq!(editor.cursors.primary_position(), (0, 8));
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_context_menu_cut_and_select_all() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (0, 5);

        let _ = editor.update(&Message::Cut);
        assert_eq!(editor.content(), " world");

        let _ = editor.update(&Message::SelectAll);
        assert_eq!(editor.get_selected_text(), Some(" world".to_string()));

        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.content(), "hello world");
    }
}
