//! Keyboard, mouse, and IME event routing for [`CodeEditor`]'s canvas.
//!
//! These are the entry points the canvas program calls: they decide whether
//! the editor should react at all (focus, focus lock, an active IME preedit),
//! then dispatch. Recognising *which* shortcut a key combination stands for
//! is [`super::shortcuts`]'s job, not this module's.

use iced::advanced::input_method;
use iced::widget::canvas::Action;
use iced::{Point, Rectangle, keyboard, mouse};

use super::shortcuts::command_pressed;
use crate::canvas_editor::features::folding;
use crate::canvas_editor::features::vim::VimMode;
use crate::canvas_editor::{
    ArrowDirection, CodeEditor, Message, focused_editor_id,
};

impl CodeEditor {
    /// Checks if the editor has focus (both Iced focus and internal canvas focus).
    ///
    /// # Returns
    ///
    /// `true` if the editor has both Iced focus and internal canvas focus and is not focus-locked; `false` otherwise
    pub(crate) fn has_focus(&self) -> bool {
        // Check if this editor has Iced focus
        focused_editor_id() == self.editor_id
            && self.has_canvas_focus
            && !self.focus_locked
    }

    fn printable_input_message(&self, ch: char) -> Message {
        if self.vim_enabled && self.vim_state.mode() != VimMode::Insert {
            Message::VimKey(ch)
        } else {
            Message::CharacterInput(ch)
        }
    }

    /// Handles character input and special navigation keys.
    ///
    /// This implementation includes focus event propagation and focus chain management
    /// for proper focus handling without mouse bounds checking.
    ///
    /// # Arguments
    ///
    /// * `key` - The keyboard key that was pressed
    /// * `modifiers` - The keyboard modifiers (Ctrl, Shift, Alt, etc.)
    /// * `text` - Optional text content from the keyboard event
    ///
    /// # Returns
    ///
    /// `Some(Action<Message>)` if input should be processed, `None` otherwise
    fn handle_character_input(
        &self,
        key: &keyboard::Key,
        modifiers: &keyboard::Modifiers,
        text: Option<&str>,
    ) -> Option<Action<Message>> {
        // Early exit: Only process character input when editor has focus
        // This prevents focus stealing where characters typed in other input fields
        // appear in the editor
        if !self.has_focus() {
            return None;
        }

        // PRIORITY 1: Check if 'text' field has valid printable character
        // This handles:
        // - Numpad keys with NumLock ON (key=Named(ArrowDown), text=Some("2"))
        // - Regular typing with shift, accents, international layouts
        if let Some(text_content) = text
            && !text_content.is_empty()
            && !modifiers.control()
            && !modifiers.alt()
        {
            // Check if it's a printable character (not a control character)
            // This filters out Enter (\n), Tab (\t), Delete (U+007F), etc.
            if let Some(first_char) = text_content.chars().next()
                && !first_char.is_control()
            {
                return Some(
                    Action::publish(self.printable_input_message(first_char))
                        .and_capture(),
                );
            }
        }

        // PRIORITY 2: Handle special named keys (navigation, editing)
        // These are only processed if text didn't contain a printable character
        let message = match key {
            keyboard::Key::Named(keyboard::key::Named::Backspace)
                if !self.vim_enabled
                    || self.vim_state.mode() == VimMode::Insert
                    || self.vim_state.command_line_active() =>
            {
                if self.vim_state.command_line_active() {
                    Some(Message::VimKey('\u{8}'))
                } else {
                    Some(Message::Backspace)
                }
            }
            keyboard::Key::Named(keyboard::key::Named::Delete)
                if !self.vim_enabled
                    || self.vim_state.mode() == VimMode::Insert =>
            {
                Some(Message::Delete)
            }
            keyboard::Key::Named(keyboard::key::Named::Enter)
                if !self.vim_enabled
                    || self.vim_state.mode() == VimMode::Insert
                    || self.vim_state.command_line_active() =>
            {
                if self.vim_state.command_line_active() {
                    Some(Message::VimKey('\n'))
                } else {
                    Some(Message::Enter)
                }
            }
            keyboard::Key::Named(keyboard::key::Named::Tab)
                if !self.vim_enabled
                    || self.vim_state.mode() == VimMode::Insert =>
            {
                // Regular Tab: check if search dialog is open
                if self.search_state.is_open {
                    Some(Message::SearchDialogTab)
                } else {
                    if modifiers.shift() {
                        Some(Message::Backtab)
                    } else {
                        Some(Message::Tab)
                    }
                }
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                Some(Message::ArrowKey(ArrowDirection::Up, modifiers.shift()))
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                Some(Message::ArrowKey(ArrowDirection::Down, modifiers.shift()))
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                Some(Message::ArrowKey(ArrowDirection::Left, modifiers.shift()))
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowRight) => Some(
                Message::ArrowKey(ArrowDirection::Right, modifiers.shift()),
            ),
            // Ctrl+Page and Alt+Page are the host's to bind, so the editor
            // declines them: falling through to the catch-all returns `None`,
            // the event stays uncaptured, and it reaches the application's own
            // `event::listen()` -- which only ever sees events no widget took.
            // Ctrl+Page Up/Down is the conventional previous/next-tab
            // combination, and tabs belong to the application. There is also
            // nothing for the editor to do with it: the document extremes it
            // would otherwise mirror are already Ctrl+Home and Ctrl+End.
            keyboard::Key::Named(keyboard::key::Named::PageUp)
                if !command_pressed(modifiers) && !modifiers.alt() =>
            {
                Some(Message::PageUp(modifiers.shift()))
            }
            keyboard::Key::Named(keyboard::key::Named::PageDown)
                if !command_pressed(modifiers) && !modifiers.alt() =>
            {
                Some(Message::PageDown(modifiers.shift()))
            }
            keyboard::Key::Named(keyboard::key::Named::Home) => {
                Some(Message::Home(modifiers.shift()))
            }
            keyboard::Key::Named(keyboard::key::Named::End) => {
                Some(Message::End(modifiers.shift()))
            }
            // PRIORITY 3: Fallback to extracting from 'key' if text was empty/control char
            // This handles edge cases where text field is not populated
            _ => {
                if !modifiers.control()
                    && !modifiers.alt()
                    && let keyboard::Key::Character(c) = key
                    && !c.is_empty()
                {
                    return c
                        .chars()
                        .next()
                        .map(|ch| self.printable_input_message(ch))
                        .map(|msg| Action::publish(msg).and_capture());
                }
                None
            }
        };

        message.map(|msg| Action::publish(msg).and_capture())
    }

    /// Handles keyboard events with focus event propagation through widget hierarchy.
    ///
    /// This implementation completes focus handling without mouse bounds checking
    /// and ensures proper focus chain management.
    ///
    /// # Arguments
    ///
    /// * `key` - The keyboard key that was pressed (base key, no modifiers applied)
    /// * `modified_key` - The key with all modifiers applied except Ctrl; used
    ///   for character shortcuts so they work on layouts where the glyph needs
    ///   Shift (e.g. `/` on French AZERTY)
    /// * `modifiers` - The keyboard modifiers (Ctrl, Shift, Alt, etc.)
    /// * `text` - Optional text content from the keyboard event
    /// * `bounds` - The rectangle bounds of the canvas widget (unused in this implementation)
    /// * `cursor` - The current mouse cursor position and status (unused in this implementation)
    ///
    /// # Returns
    ///
    /// `Some(Action<Message>)` if the event was handled, `None` otherwise
    pub(crate) fn handle_keyboard_event(
        &self,
        key: &keyboard::Key,
        modified_key: &keyboard::Key,
        modifiers: &keyboard::Modifiers,
        text: &Option<iced::advanced::graphics::core::SmolStr>,
        _bounds: Rectangle,
        _cursor: &mouse::Cursor,
    ) -> Option<Action<Message>> {
        // Early exit: Check if editor has focus and is not focus-locked
        // This prevents focus stealing where keyboard input meant for other widgets
        // is incorrectly processed by this editor during focus transitions
        if !self.has_focus() || self.focus_locked {
            return None;
        }

        // Skip if IME is active (unless Ctrl/Command is pressed)
        if self.ime_preedit.is_some()
            && !(modifiers.control() || modifiers.command())
        {
            return None;
        }

        // Try keyboard shortcuts first
        if let Some(action) =
            self.handle_keyboard_shortcuts(key, modified_key, modifiers)
        {
            return Some(action);
        }

        // Handle character input and special keys
        // Convert Option<SmolStr> to Option<&str>
        let text_str = text.as_ref().map(|s| s.as_str());
        self.handle_character_input(key, modifiers, text_str)
    }

    /// Returns the logical line of the fold header whose chevron is at `point`,
    /// if any.
    ///
    /// Returns `None` when folding is disabled, when the point is outside the
    /// fold margin (always the case with [`crate::FoldMarkers::Never`]), or
    /// when the targeted line is not a fold header.
    ///
    /// # Arguments
    ///
    /// * `point` - The click position in canvas coordinates
    pub(crate) fn fold_header_at_point(&self, point: Point) -> Option<usize> {
        if !self.folding_enabled {
            return None;
        }

        // The fold margin is on either side of the line numbers; it is empty
        // (so never hit) when fold markers are never shown.
        if !self.gutter_columns().fold_markers.contains(&point.x) {
            return None;
        }

        let visual_line_idx = (point.y / self.line_height) as usize;
        let visual_lines = self.visual_lines_cached(self.viewport_width);
        let visual_line = visual_lines.get(visual_line_idx)?;
        if !visual_line.is_first_segment() {
            return None;
        }

        folding::is_line_fold_header(&self.buffer, visual_line.logical_line)
            .then_some(visual_line.logical_line)
    }

    /// Records whether the pointer is over the gutter.
    ///
    /// # Arguments
    ///
    /// * `position` - The pointer position in canvas coordinates, or `None`
    ///   when it is outside the canvas
    ///
    /// # Returns
    ///
    /// `true` when the change alters what is drawn, i.e. the hover state
    /// flipped under [`crate::FoldMarkers::OnGutterHover`]. The content layer,
    /// where the markers are drawn, is then invalidated.
    pub(crate) fn update_gutter_hover(&self, position: Option<Point>) -> bool {
        let hovered =
            position.is_some_and(|point| point.x < self.gutter_width());
        let changed = self.gutter_hovered.replace(hovered) != hovered;
        let redraw = changed
            && self.folding_enabled
            && self.fold_markers == crate::FoldMarkers::OnGutterHover;
        if redraw {
            self.content_cache.clear();
        }
        redraw
    }

    /// Handles mouse events (button presses, movement, releases).
    ///
    /// # Arguments
    ///
    /// * `event` - The mouse event to handle
    /// * `bounds` - The rectangle bounds of the canvas widget
    /// * `cursor` - The current mouse cursor position and status
    ///
    /// # Returns
    ///
    /// `Some(Action<Message>)` if the event was handled, `None` otherwise
    pub(crate) fn handle_mouse_event(
        &self,
        event: &mouse::Event,
        bounds: Rectangle,
        cursor: &mouse::Cursor,
    ) -> Option<Action<Message>> {
        if matches!(
            event,
            mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft
        ) && self.update_gutter_hover(cursor.position_in(bounds))
            && !self.is_dragging
            && !cursor.is_over(bounds)
        {
            // The pointer left the gutter by leaving the canvas: no message
            // is published below, so ask for the redraw that hides the
            // hover-only fold markers.
            return Some(Action::request_redraw());
        }

        match event {
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                // A press outside the canvas means focus is moving to another
                // widget. iced 0.14 delivers keyboard events to every widget,
                // so without releasing focus here the editor would keep
                // consuming keystrokes (and Ctrl+C/X/A) meant for, say, a
                // `text_input` the user just clicked. The press is not
                // captured: it belongs to the widget under the pointer.
                let Some(position) = cursor.position_in(bounds) else {
                    return self
                        .has_canvas_focus
                        .then(|| Action::publish(Message::CanvasFocusLost));
                };
                Some(position).map(|position| {
                    // Clicking a fold chevron toggles the block instead of
                    // moving the caret.
                    if let Some(header) = self.fold_header_at_point(position) {
                        return Action::publish(Message::ToggleFold(header))
                            .and_capture();
                    }

                    // Check for Ctrl (or Command on macOS) + Click
                    #[cfg(target_os = "macos")]
                    let is_jump_click = self.modifiers.get().command();
                    #[cfg(not(target_os = "macos"))]
                    let is_jump_click = self.modifiers.get().control();

                    if is_jump_click {
                        return Action::publish(Message::JumpClick(position));
                    }

                    // Alt+Click: add a new cursor at the clicked position
                    if self.modifiers.get().alt() {
                        let message = if self.vim_enabled {
                            Message::MouseClick(position)
                        } else {
                            Message::AltClick(position)
                        };
                        return Action::publish(message).and_capture();
                    }

                    // Shift+Click: extend the selection to the click. Left
                    // uncaptured like a plain click, for focus management.
                    if self.modifiers.get().shift() {
                        return Action::publish(Message::ShiftClick(position));
                    }

                    let click_count = self.classify_click(position);
                    match click_count {
                        2 => Action::publish(Message::DoubleClick(position))
                            .and_capture(),
                        3 => Action::publish(Message::TripleClick(position))
                            .and_capture(),
                        // Don't capture the event so it can bubble up for focus management
                        // This implements focus event propagation through the widget hierarchy
                        _ => Action::publish(Message::MouseClick(position)),
                    }
                })
            }
            mouse::Event::ButtonPressed(mouse::Button::Right) => {
                cursor.position_in(bounds).map(|position| {
                    Action::publish(Message::ContextMenuRequested(position))
                        .and_capture()
                })
            }
            mouse::Event::CursorMoved { .. } => {
                if self.is_dragging {
                    cursor.land().position().map(|position| {
                        Action::publish(Message::MouseDrag(iced::Point::new(
                            position.x - bounds.x,
                            position.y - bounds.y,
                        )))
                        .and_capture()
                    })
                } else {
                    cursor.position_in(bounds).map(|position| {
                        Action::publish(Message::MouseHover(position))
                    })
                }
            }
            mouse::Event::ButtonReleased(mouse::Button::Left) => {
                if self.is_dragging || cursor.is_over(bounds) {
                    Some(Action::publish(Message::MouseRelease).and_capture())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Handles IME (Input Method Editor) events for complex text input.
    ///
    /// # Arguments
    ///
    /// * `event` - The IME event to handle
    /// * `bounds` - The rectangle bounds of the canvas widget
    /// * `cursor` - The current mouse cursor position and status
    ///
    /// # Returns
    ///
    /// `Some(Action<Message>)` if the event was handled, `None` otherwise
    pub(crate) fn handle_ime_event(
        &self,
        event: &input_method::Event,
        _bounds: Rectangle,
        _cursor: &mouse::Cursor,
    ) -> Option<Action<Message>> {
        // Early exit: Check if editor has focus and is not focus-locked
        // This prevents focus stealing where IME events meant for other widgets
        // are incorrectly processed by this editor during focus transitions
        if !self.has_focus() || self.focus_locked {
            return None;
        }
        if self.vim_enabled && self.vim_state.mode() != VimMode::Insert {
            return None;
        }

        // IME event handling
        // ---------------------------------------------------------------------
        // Core mapping: convert Iced IME events into editor Messages
        //
        // Flow:
        // 1. Opened: IME activated (e.g. switching input method). Clear old preedit state.
        // 2. Preedit: User is composing (e.g. typing "nihao" before commit).
        //    - content: current candidate text
        //    - selection: selection range within the text, in bytes
        // 3. Commit: User confirms a candidate and commits text into the buffer.
        // 4. Closed: IME closed or lost focus.
        //
        // Safety checks:
        // - handle only when `focused_id` matches this editor ID
        // - handle only when `has_canvas_focus` is true
        // This ensures IME events are not delivered to the wrong widget.
        // ---------------------------------------------------------------------
        let message = match event {
            input_method::Event::Opened => Message::ImeOpened,
            input_method::Event::Preedit(content, selection) => {
                Message::ImePreedit(content.clone(), selection.clone())
            }
            input_method::Event::Commit(content) => {
                Message::ImeCommit(content.clone())
            }
            input_method::Event::Closed => Message::ImeClosed,
        };

        Some(Action::publish(message).and_capture())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_editor::metrics::{FOLD_MARGIN_WIDTH, compare_floats};
    use iced::event;
    use std::cmp::Ordering;

    #[test]
    fn active_drag_tracks_and_releases_outside_canvas() {
        let mut editor = CodeEditor::new("one\ntwo", "txt");
        editor.is_dragging = true;
        let outside = cursor_at(-20.0, -20.0).levitate();
        assert!(matches!(
            mouse_message(
                &editor,
                &mouse::Event::CursorMoved { position: Point::ORIGIN },
                &outside
            ),
            Some(Message::MouseDrag(_))
        ));
        assert!(matches!(
            mouse_message(
                &editor,
                &mouse::Event::ButtonReleased(mouse::Button::Left),
                &outside
            ),
            Some(Message::MouseRelease)
        ));
    }

    #[test]
    fn test_vim_navigation_keyboard_route_uses_dedicated_message() {
        let mut editor = CodeEditor::new("abc", "txt").with_vim_enabled(true);

        assert!(matches!(
            editor.printable_input_message('l'),
            Message::VimKey('l')
        ));

        let _ = editor.vim_state.parse_key('i');
        assert!(matches!(
            editor.printable_input_message('x'),
            Message::CharacterInput('x')
        ));

        editor.set_vim_enabled(false);
        assert!(matches!(
            editor.printable_input_message('x'),
            Message::CharacterInput('x')
        ));

        editor.set_vim_enabled(true);
        let key = keyboard::Key::Character("r".into());
        let message = editor
            .handle_keyboard_shortcuts(&key, &key, &keyboard::Modifiers::CTRL)
            .map(|action| action.into_inner().0);
        assert!(matches!(message, Some(Some(Message::Redo))));
    }

    #[test]
    fn test_vim_command_line_routes_enter_and_backspace() {
        let mut editor = CodeEditor::new("abc", "txt").with_vim_enabled(true);
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;
        let _ = editor.vim_state.parse_key('/');

        let backspace = editor
            .handle_character_input(
                &keyboard::Key::Named(keyboard::key::Named::Backspace),
                &keyboard::Modifiers::NONE,
                None,
            )
            .map(|action| action.into_inner().0);
        assert!(matches!(backspace, Some(Some(Message::VimKey('\u{8}')))));

        let enter = editor
            .handle_character_input(
                &keyboard::Key::Named(keyboard::key::Named::Enter),
                &keyboard::Modifiers::NONE,
                None,
            )
            .map(|action| action.into_inner().0);
        assert!(matches!(enter, Some(Some(Message::VimKey('\n')))));
    }

    // =========================================================================
    // Mouse events
    // =========================================================================
    //
    // `handle_mouse_event` reports positions *relative to the canvas bounds*,
    // so `MOUSE_BOUNDS` is deliberately offset from the origin: with bounds at
    // (0, 0) an absolute-position lookup and a bounds-relative one are
    // indistinguishable, and every position assertion below would still pass
    // if `position_in` were swapped for `position`.

    /// Canvas bounds shared by the mouse tests, offset from the origin on
    /// purpose — see the note above.
    const MOUSE_BOUNDS: Rectangle =
        Rectangle { x: 10.0, y: 20.0, width: 400.0, height: 300.0 };

    /// A cursor sitting at `(x, y)` measured from [`MOUSE_BOUNDS`]'s top-left
    /// corner, i.e. in the same coordinate space as the published messages.
    fn cursor_at(x: f32, y: f32) -> mouse::Cursor {
        mouse::Cursor::Available(Point::new(
            MOUSE_BOUNDS.x + x,
            MOUSE_BOUNDS.y + y,
        ))
    }

    /// Feeds `event` to the editor and returns the message it publishes.
    ///
    /// Not idempotent for left presses: each one advances the editor's
    /// click-count state, so calling this twice at the same spot yields a
    /// double click. Use a fresh editor when a test needs two independent
    /// single clicks.
    fn mouse_message(
        editor: &CodeEditor,
        event: &mouse::Event,
        cursor: &mouse::Cursor,
    ) -> Option<Message> {
        editor
            .handle_mouse_event(event, MOUSE_BOUNDS, cursor)
            .and_then(|action| action.into_inner().0)
    }

    /// Returns the event status the editor reports for `event`.
    fn mouse_status(
        editor: &CodeEditor,
        event: &mouse::Event,
        cursor: &mouse::Cursor,
    ) -> Option<event::Status> {
        editor
            .handle_mouse_event(event, MOUSE_BOUNDS, cursor)
            .map(|action| action.into_inner().2)
    }

    /// Extracts the canvas position carried by the position-bearing mouse
    /// messages, so a test can assert on it without destructuring (`panic!`
    /// is denied workspace-wide, which rules out a `let ... else` binding).
    fn message_position(message: &Option<Message>) -> Option<Point> {
        match message {
            Some(
                Message::MouseClick(position)
                | Message::MouseHover(position)
                | Message::MouseDrag(position)
                | Message::AltClick(position)
                | Message::JumpClick(position)
                | Message::ContextMenuRequested(position),
            ) => Some(*position),
            _ => None,
        }
    }

    /// Asserts `message` carries the position `(x, y)`, compared within the
    /// project's float epsilon (`float_cmp` is denied workspace-wide).
    fn assert_position(message: &Option<Message>, x: f32, y: f32) {
        let position = message_position(message);
        assert!(
            position.is_some_and(|position| {
                compare_floats(position.x, x) == Ordering::Equal
                    && compare_floats(position.y, y) == Ordering::Equal
            }),
            "expected position ({x}, {y}), got {message:?}"
        );
    }

    /// A left-button press, the event most of the mouse tests start from.
    const LEFT_PRESS: mouse::Event =
        mouse::Event::ButtonPressed(mouse::Button::Left);

    #[test]
    fn test_left_click_publishes_a_bounds_relative_position() {
        let editor = CodeEditor::new("one\ntwo", "txt");

        let message =
            mouse_message(&editor, &LEFT_PRESS, &cursor_at(30.0, 40.0));
        assert!(
            matches!(message, Some(Message::MouseClick(_))),
            "expected a MouseClick, got {message:?}"
        );
        assert_position(&message, 30.0, 40.0);

        // A plain click stays uncaptured so it can bubble up for focus
        // management — unlike every other left-click outcome below. Checked on
        // a fresh editor because left clicks accumulate click-count state.
        let editor = CodeEditor::new("one\ntwo", "txt");
        assert!(matches!(
            mouse_status(&editor, &LEFT_PRESS, &cursor_at(30.0, 40.0)),
            Some(event::Status::Ignored)
        ));
    }

    #[test]
    fn test_mouse_events_outside_the_canvas_are_left_alone() {
        let editor = CodeEditor::new("one\ntwo", "txt");
        let outside = mouse::Cursor::Available(Point::new(
            MOUSE_BOUNDS.x + MOUSE_BOUNDS.width + 5.0,
            MOUSE_BOUNDS.y + MOUSE_BOUNDS.height + 5.0,
        ));
        let release = mouse::Event::ButtonReleased(mouse::Button::Left);
        let moved = mouse::Event::CursorMoved { position: Point::ORIGIN };

        for event in [&LEFT_PRESS, &release, &moved] {
            assert!(
                editor
                    .handle_mouse_event(event, MOUSE_BOUNDS, &outside)
                    .is_none(),
                "{event:?} outside the canvas must not be handled"
            );
            assert!(
                editor
                    .handle_mouse_event(
                        event,
                        MOUSE_BOUNDS,
                        &mouse::Cursor::Unavailable
                    )
                    .is_none(),
                "{event:?} with no cursor must not be handled"
            );
        }
    }

    /// A cursor just past the bottom-right corner of [`MOUSE_BOUNDS`].
    fn cursor_outside() -> mouse::Cursor {
        mouse::Cursor::Available(Point::new(
            MOUSE_BOUNDS.x + MOUSE_BOUNDS.width + 5.0,
            MOUSE_BOUNDS.y + MOUSE_BOUNDS.height + 5.0,
        ))
    }

    #[test]
    fn test_left_press_outside_releases_canvas_focus() {
        let mut editor = CodeEditor::new("one\ntwo", "txt");
        editor.has_canvas_focus = true;

        let message = mouse_message(&editor, &LEFT_PRESS, &cursor_outside());
        assert!(
            matches!(message, Some(Message::CanvasFocusLost)),
            "a press aimed at another widget must release focus, got {message:?}"
        );
    }

    #[test]
    fn test_left_press_outside_is_not_captured() {
        let mut editor = CodeEditor::new("one\ntwo", "txt");
        editor.has_canvas_focus = true;

        // The press belongs to the widget under the pointer, which must
        // still receive it.
        assert!(matches!(
            mouse_status(&editor, &LEFT_PRESS, &cursor_outside()),
            Some(event::Status::Ignored)
        ));
    }

    #[test]
    fn test_left_press_outside_is_silent_without_canvas_focus() {
        let editor = CodeEditor::new("one\ntwo", "txt");
        assert!(!editor.has_canvas_focus);

        // An unfocused editor has nothing to release: every editor in the
        // window sees the press, and only the focused one may react.
        assert!(
            editor
                .handle_mouse_event(
                    &LEFT_PRESS,
                    MOUSE_BOUNDS,
                    &cursor_outside()
                )
                .is_none()
        );
    }

    #[test]
    fn test_right_click_requests_the_context_menu() {
        let editor = CodeEditor::new("one\ntwo", "txt");
        let right_press = mouse::Event::ButtonPressed(mouse::Button::Right);
        let cursor = cursor_at(15.0, 25.0);

        let message = mouse_message(&editor, &right_press, &cursor);
        assert!(
            matches!(message, Some(Message::ContextMenuRequested(_))),
            "expected a ContextMenuRequested, got {message:?}"
        );
        assert_position(&message, 15.0, 25.0);
        assert!(matches!(
            mouse_status(&editor, &right_press, &cursor),
            Some(event::Status::Captured)
        ));
    }

    #[test]
    fn test_cursor_moved_hovers_when_idle_and_drags_while_dragging() {
        let mut editor = CodeEditor::new("one\ntwo", "txt");
        let moved = mouse::Event::CursorMoved {
            // Ignored by the handler, which reads the cursor rather than the
            // event payload; set to a decoy value to prove it.
            position: Point::new(999.0, 999.0),
        };
        let cursor = cursor_at(50.0, 60.0);

        assert!(!editor.is_dragging);
        let message = mouse_message(&editor, &moved, &cursor);
        assert!(
            matches!(message, Some(Message::MouseHover(_))),
            "expected a MouseHover, got {message:?}"
        );
        assert_position(&message, 50.0, 60.0);
        // Hover must stay uncaptured or it would swallow motion events the
        // host may need.
        assert!(matches!(
            mouse_status(&editor, &moved, &cursor),
            Some(event::Status::Ignored)
        ));

        editor.is_dragging = true;
        let message = mouse_message(&editor, &moved, &cursor);
        assert!(
            matches!(message, Some(Message::MouseDrag(_))),
            "expected a MouseDrag, got {message:?}"
        );
        assert_position(&message, 50.0, 60.0);
        assert!(matches!(
            mouse_status(&editor, &moved, &cursor),
            Some(event::Status::Captured)
        ));
    }

    #[test]
    fn test_left_release_is_handled_only_over_the_canvas() {
        let editor = CodeEditor::new("one\ntwo", "txt");
        let release = mouse::Event::ButtonReleased(mouse::Button::Left);

        assert!(matches!(
            mouse_message(&editor, &release, &cursor_at(5.0, 5.0)),
            Some(Message::MouseRelease)
        ));
        assert!(matches!(
            mouse_status(&editor, &release, &cursor_at(5.0, 5.0)),
            Some(event::Status::Captured)
        ));
    }

    /// Canvas point over column `col` of `line`, in [`MOUSE_BOUNDS`] space.
    fn text_point(editor: &CodeEditor, line: usize, col: usize) -> Point {
        Point::new(
            editor.gutter_width() + 5.0 + editor.char_width() * col as f32,
            editor.line_height() * (line as f32 + 0.5),
        )
    }

    /// Routes a left press at `(line, col)` through the canvas event handler
    /// with `modifiers` held, then applies the published message.
    fn press_at(
        editor: &mut CodeEditor,
        line: usize,
        col: usize,
        modifiers: keyboard::Modifiers,
    ) {
        editor.modifiers.set(modifiers);
        let point = text_point(editor, line, col);
        let message =
            mouse_message(editor, &LEFT_PRESS, &cursor_at(point.x, point.y));
        editor.modifiers.set(keyboard::Modifiers::default());
        if let Some(message) = message {
            let _ = editor.update(&message);
        }
        let _ = editor.update(&Message::MouseRelease);
    }

    #[test]
    fn test_shift_click_selects_from_caret_to_click() {
        let mut editor = CodeEditor::new("hello world\nsecond line", "txt");
        press_at(&mut editor, 0, 2, keyboard::Modifiers::default());

        press_at(&mut editor, 1, 3, keyboard::Modifiers::SHIFT);

        assert_eq!(editor.cursors.primary().anchor, Some((0, 2)));
        assert_eq!(editor.cursors.primary_position(), (1, 3));
        assert_eq!(editor.selected_text().as_deref(), Some("llo world\nsec"));
    }

    #[test]
    fn test_shift_click_moves_the_end_but_keeps_the_original_anchor() {
        let mut editor = CodeEditor::new("hello world\nsecond line", "txt");
        press_at(&mut editor, 0, 6, keyboard::Modifiers::default());
        press_at(&mut editor, 1, 6, keyboard::Modifiers::SHIFT);

        // A second Shift+click before the anchor flips the selection
        // around the same anchor instead of starting from the last click.
        press_at(&mut editor, 0, 1, keyboard::Modifiers::SHIFT);

        assert_eq!(editor.cursors.primary().anchor, Some((0, 6)));
        assert_eq!(editor.cursors.primary_position(), (0, 1));
        assert_eq!(editor.selected_text().as_deref(), Some("ello "));
    }

    #[test]
    fn test_shift_click_takes_focus_and_collapses_multi_cursor() {
        let mut editor = CodeEditor::new("aaaa\nbbbb\ncccc", "txt");
        editor.cursors.add_cursor((2, 1));
        let primary = editor.cursors.primary_position();
        assert!(!editor.has_canvas_focus);

        press_at(&mut editor, 1, 2, keyboard::Modifiers::SHIFT);

        // Like a plain click, only the primary cursor survives, and it
        // anchors the new selection.
        assert!(editor.has_canvas_focus);
        assert_eq!(editor.cursors.iter().count(), 1);
        assert_eq!(editor.cursors.primary().anchor, Some(primary));
        assert_eq!(editor.cursors.primary_position(), (1, 2));
    }

    #[test]
    fn test_shift_click_then_drag_extends_from_the_anchor() {
        let mut editor = CodeEditor::new("hello world\nsecond line", "txt");
        press_at(&mut editor, 0, 2, keyboard::Modifiers::SHIFT);
        editor.modifiers.set(keyboard::Modifiers::SHIFT);
        let start = text_point(&editor, 0, 4);
        let message =
            mouse_message(&editor, &LEFT_PRESS, &cursor_at(start.x, start.y));
        editor.modifiers.set(keyboard::Modifiers::default());
        if let Some(message) = message {
            let _ = editor.update(&message);
        }

        let _ = editor.update(&Message::MouseDrag(text_point(&editor, 1, 2)));

        assert_eq!(editor.cursors.primary().anchor, Some((0, 0)));
        assert_eq!(editor.cursors.primary_position(), (1, 2));
    }

    #[test]
    fn test_vim_shift_click_enters_visual_mode() {
        let mut editor = CodeEditor::new("hello world\nsecond line", "txt")
            .with_vim_enabled(true);

        press_at(&mut editor, 0, 4, keyboard::Modifiers::SHIFT);

        assert_eq!(editor.vim_state.mode(), VimMode::Visual);
        assert!(editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_alt_click_adds_a_cursor_unless_vim_is_enabled() {
        let mut editor = CodeEditor::new("one\ntwo", "txt");
        editor.modifiers.set(keyboard::Modifiers::ALT);
        let cursor = cursor_at(70.0, 10.0);

        let message = mouse_message(&editor, &LEFT_PRESS, &cursor);
        assert!(
            matches!(message, Some(Message::AltClick(_))),
            "expected an AltClick, got {message:?}"
        );
        assert_position(&message, 70.0, 10.0);

        // Vim owns its own multi-cursor model, so Alt+Click degrades to a
        // plain click rather than adding an editor cursor behind its back.
        editor.set_vim_enabled(true);
        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor),
            Some(Message::MouseClick(_))
        ));
    }

    #[test]
    fn test_control_click_jumps_to_definition_without_capturing() {
        let editor = CodeEditor::new("one\ntwo", "txt");
        // COMMAND is CTRL on every platform but macOS, where the handler reads
        // `command()` instead; setting both exercises the same branch on each.
        editor
            .modifiers
            .set(keyboard::Modifiers::CTRL | keyboard::Modifiers::COMMAND);
        let cursor = cursor_at(80.0, 12.0);

        let message = mouse_message(&editor, &LEFT_PRESS, &cursor);
        assert!(
            matches!(message, Some(Message::JumpClick(_))),
            "expected a JumpClick, got {message:?}"
        );
        assert_position(&message, 80.0, 12.0);

        // Left uncaptured: the host resolves the jump and may decline it.
        assert!(matches!(
            mouse_status(&editor, &LEFT_PRESS, &cursor),
            Some(event::Status::Ignored)
        ));
    }

    #[test]
    fn test_repeated_clicks_at_one_spot_escalate_to_double_then_triple() {
        let editor = CodeEditor::new("one two three", "txt");
        let cursor = cursor_at(90.0, 5.0);

        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor),
            Some(Message::MouseClick(_))
        ));
        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor),
            Some(Message::DoubleClick(_))
        ));
        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor),
            Some(Message::TripleClick(_))
        ));
        // A fourth click wraps back to a single click rather than sticking at
        // triple.
        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor),
            Some(Message::MouseClick(_))
        ));
    }

    #[test]
    fn test_a_click_far_from_the_previous_one_stays_a_single_click() {
        let editor = CodeEditor::new("one two three", "txt");

        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor_at(20.0, 5.0)),
            Some(Message::MouseClick(_))
        ));
        // Beyond the 6px tolerance, so this is a new click sequence.
        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor_at(120.0, 80.0)),
            Some(Message::MouseClick(_))
        ));
    }

    #[test]
    fn test_clicking_the_fold_chevron_toggles_the_block() {
        let editor = CodeEditor::new("fn a() {\n    body\n}\n", "rs");
        assert!(editor.folding_enabled);

        // The chevron column sits between the line-number area and the text;
        // aim at its middle, on the first visual line.
        let x = f32::midpoint(
            editor.line_number_gutter_width(),
            editor.gutter_width(),
        );
        let y = editor.line_height * 0.5;
        let cursor = cursor_at(x, y);

        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor),
            Some(Message::ToggleFold(0))
        ));

        // The chevron wins over the modifier-click bindings: Alt+Click on it
        // toggles the fold instead of dropping a cursor in the gutter.
        editor.modifiers.set(keyboard::Modifiers::ALT);
        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor),
            Some(Message::ToggleFold(0))
        ));
    }

    /// Message published by a left press at `x` on the first visual line.
    fn press_message_at_x(editor: &CodeEditor, x: f32) -> Option<Message> {
        mouse_message(
            editor,
            &LEFT_PRESS,
            &cursor_at(x, editor.line_height * 0.5),
        )
    }

    #[test]
    fn test_fold_click_follows_the_marker_column_position() {
        let mut editor = CodeEditor::new("fn a() {\n    body\n}\n", "rs");
        editor.set_fold_marker_position(
            crate::FoldMarkerPosition::BeforeLineNumbers,
        );

        // The markers now sit at the outer edge of the gutter...
        assert!(matches!(
            press_message_at_x(&editor, FOLD_MARGIN_WIDTH / 2.0),
            Some(Message::ToggleFold(0))
        ));
        // ...and their old place, right of the numbers, is a plain click.
        let old_column = f32::midpoint(
            editor.line_number_gutter_width(),
            editor.gutter_width(),
        );
        assert!(!matches!(
            press_message_at_x(&editor, old_column),
            Some(Message::ToggleFold(_))
        ));
    }

    #[test]
    fn test_never_fold_markers_leave_no_click_target() {
        let mut editor = CodeEditor::new("fn a() {\n    body\n}\n", "rs");
        let with_markers = editor.gutter_width();
        editor.set_fold_markers(crate::FoldMarkers::Never);

        assert!(
            (with_markers - editor.gutter_width() - FOLD_MARGIN_WIDTH).abs()
                < f32::EPSILON,
            "the fold-marker column must be freed"
        );
        for x in [1.0, editor.gutter_width() - 1.0] {
            assert!(!matches!(
                press_message_at_x(&editor, x),
                Some(Message::ToggleFold(_))
            ));
        }
    }

    #[test]
    fn test_gutter_hover_redraws_only_on_hover_mode_transitions() {
        let mut editor = CodeEditor::new("fn a() {\n    body\n}\n", "rs");
        let gutter = Some(Point::new(5.0, 5.0));
        let text = Some(Point::new(editor.gutter_width() + 30.0, 5.0));

        // `Always` draws markers regardless of hover: nothing to redraw.
        assert!(!editor.update_gutter_hover(gutter));
        assert!(!editor.update_gutter_hover(text));

        editor.set_fold_markers(crate::FoldMarkers::OnGutterHover);
        assert!(editor.update_gutter_hover(gutter), "entering the gutter");
        assert!(!editor.update_gutter_hover(gutter), "still in the gutter");
        assert!(editor.update_gutter_hover(text), "leaving to the text");
        assert!(editor.update_gutter_hover(gutter));
        assert!(editor.update_gutter_hover(None), "leaving the canvas");
    }

    #[test]
    fn test_leaving_the_canvas_from_the_gutter_requests_a_redraw() {
        let mut editor = CodeEditor::new("fn a() {\n    body\n}\n", "rs");
        editor.set_fold_markers(crate::FoldMarkers::OnGutterHover);
        let moved = mouse::Event::CursorMoved { position: Point::ORIGIN };
        let _ = editor.handle_mouse_event(
            &moved,
            MOUSE_BOUNDS,
            &cursor_at(5.0, 5.0),
        );
        assert!(editor.gutter_hovered.get());

        let action =
            editor.handle_mouse_event(&moved, MOUSE_BOUNDS, &cursor_outside());

        assert!(!editor.gutter_hovered.get());
        assert!(
            action.is_some(),
            "no message is published outside the canvas, so the markers \
             would stay visible without an explicit redraw"
        );
    }

    #[test]
    fn test_clicking_the_text_area_is_not_a_fold_toggle() {
        let editor = CodeEditor::new("fn a() {\n    body\n}\n", "rs");

        // Same line, but past the gutter: this is ordinary caret placement.
        let cursor =
            cursor_at(editor.gutter_width() + 20.0, editor.line_height * 0.5);
        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor),
            Some(Message::MouseClick(_))
        ));
    }

    #[test]
    fn test_unhandled_mouse_events_are_ignored() {
        let editor = CodeEditor::new("one\ntwo", "txt");
        let cursor = cursor_at(30.0, 30.0);
        let events = [
            mouse::Event::ButtonPressed(mouse::Button::Middle),
            mouse::Event::ButtonReleased(mouse::Button::Right),
            mouse::Event::CursorEntered,
            mouse::Event::CursorLeft,
            mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
            },
        ];

        for event in &events {
            assert!(
                editor
                    .handle_mouse_event(event, MOUSE_BOUNDS, &cursor)
                    .is_none(),
                "{event:?} should not be handled here"
            );
        }
    }
}
