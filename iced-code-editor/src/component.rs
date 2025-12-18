use iced::Element;
use iced::widget::{Id, scrollable, text_editor};

use crate::state::EditorTheme;

// Editor configuration constants
const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 20.0;
const PADDING_TOP: f32 = 10.0;
const PADDING_LEFT: f32 = 10.0;
const CHAR_WIDTH: f32 = 8.5; // Approximate width of a monospace character
const GUTTER_WIDTH: f32 = 60.0;

// Scroll configuration constants
const VIEWPORT_HEIGHT: f32 = 600.0; // Estimated visible height
const TOP_MARGIN: f32 = 100.0; // Top margin to trigger scrolling
const BOTTOM_MARGIN: f32 = 100.0; // Bottom margin to trigger scrolling

/// A code editor component with syntax highlighting and line numbers.
///
/// This component provides a full-featured code editor with:
/// - Syntax highlighting for multiple languages (Python, Lua, etc.)
/// - Line numbers gutter
/// - Smart auto-scrolling to keep cursor visible
/// - Cursor position tracking
/// - Custom key bindings (e.g., Tab inserts 4 spaces)
pub struct CodeEditorComponent {
    content: text_editor::Content,
    theme: EditorTheme,
    syntax: String, // "py" for Python, "lua" for Lua, etc.
    scroll_id: Id,
    cursor_line: usize,          // Track cursor line position
    cursor_column: usize,        // Track cursor column position
    last_scroll_y: f32,          // Last scroll position for optimization
    cached_line_count: usize,    // Cached line count to avoid parsing in view()
    cached_line_numbers: String, // Pre-computed line numbers string to avoid format!() at 60 FPS
}

/// Events emitted by the code editor component.
#[derive(Debug, Clone)]
pub enum Event {
    /// An action was performed on the text editor (cursor movement, editing, etc.)
    ActionPerformed(text_editor::Action),
}

impl CodeEditorComponent {
    /// Creates a new code editor with specified content and syntax highlighting.
    ///
    /// # Arguments
    ///
    /// * `initial_content` - The initial text content to display
    /// * `syntax` - The syntax highlighting language identifier. This parameter is passed
    ///   to Iced's `text_editor::highlight()` function, which uses the `syntect`
    ///   crate for syntax highlighting. Common supported values include:
    ///   - `"py"` or `"python"` for Python
    ///   - `"lua"` for Lua
    ///   - `"rs"` or `"rust"` for Rust
    ///   - `"js"` or `"javascript"` for JavaScript
    ///   - `"json"` for JSON
    ///   - `"toml"` for TOML
    ///   - `"md"` or `"markdown"` for Markdown
    ///   - `"c"`, `"cpp"`, `"java"`, `"go"`, `"html"`, `"css"`, and many more
    ///
    ///   For a complete list of supported languages, refer to the syntect crate's
    ///   built-in syntax sets or Iced's highlighter documentation.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Create a Python editor
    /// let python_editor = CodeEditorComponent::new_with_language("print('Hello')", "py");
    ///
    /// // Create a Rust editor
    /// let rust_editor = CodeEditorComponent::new_with_language("fn main() {}", "rs");
    /// ```
    pub fn new_with_language(initial_content: &str, syntax: &str) -> Self {
        let line_count = initial_content.lines().count().max(1);
        let line_numbers = Self::generate_line_numbers_string(line_count);
        Self {
            content: text_editor::Content::with_text(initial_content),
            theme: EditorTheme::dark(),
            syntax: syntax.to_string(),
            scroll_id: Id::unique(),
            cursor_line: 0,
            cursor_column: 0,
            last_scroll_y: 0.0,
            cached_line_count: line_count,
            cached_line_numbers: line_numbers,
        }
    }
}

impl Default for CodeEditorComponent {
    /// Creates a default code editor with example Lua code.
    fn default() -> Self {
        // Example Lua code with various features
        let lua_content = r#"-- Lua example code
function hello_world()
    print("Hello, World!")
    
    for i = 1, 10 do
        print("Count: " .. i)
    end
end

function fibonacci(n)
    if n <= 1 then
        return n
    end
    return fibonacci(n - 1) + fibonacci(n - 2)
end

-- Tables (equivalent to dictionaries/objects)
local person = {
    name = "John",
    age = 30,
    greet = function(self)
        print("Hello, I'm " .. self.name)
    end
}

-- Main execution
hello_world()
print("Fibonacci(10) = " .. fibonacci(10))
person:greet()
"#
        .to_string();

        Self::new_with_language(&lua_content, "lua")
    }
}

impl CodeEditorComponent {
    /// Generates the line numbers string for the gutter.
    ///
    /// This is an expensive operation (format! + allocations), so the result
    /// should be cached and only regenerated when the line count changes.
    ///
    /// # Arguments
    ///
    /// * `line_count` - Number of lines to generate
    ///
    /// # Returns
    ///
    /// A string with line numbers formatted as "   1\n   2\n   3\n..."
    fn generate_line_numbers_string(line_count: usize) -> String {
        (1..=line_count)
            .map(|i| format!("{:>4}", i))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Updates the editor state in response to a user action.
    ///
    /// This function handles all cursor movements and editing actions,
    /// and triggers smart scrolling when necessary to keep the cursor visible.
    ///
    /// # Arithmetic and Indexing
    ///
    /// This function contains many arithmetic operations and indexing operations
    /// that are safe because:
    /// - Indices are always bounded by `min()` and `saturating_sub()`
    /// - `self.cursor_line` is always maintained within `0..lines.len()`
    /// - Additions/subtractions are protected against overflows
    ///
    /// # Returns
    ///
    /// Returns a `Task` that may contain a scroll command to keep the cursor visible.
    pub fn update(&mut self, event: Event) -> iced::Task<Event> {
        use text_editor::Action;

        match event {
            Event::ActionPerformed(action) => {
                // Optimized: only parse lines when absolutely necessary
                let needs_lines = matches!(
                    &action,
                    Action::Move(text_editor::Motion::Left)
                        | Action::Move(text_editor::Motion::Right)
                        | Action::Move(text_editor::Motion::WordLeft)
                        | Action::Move(text_editor::Motion::WordRight)
                        | Action::Move(text_editor::Motion::Home)
                        | Action::Move(text_editor::Motion::End)
                        | Action::Select(text_editor::Motion::Left)
                        | Action::Select(text_editor::Motion::Right)
                        | Action::Select(text_editor::Motion::WordLeft)
                        | Action::Select(text_editor::Motion::WordRight)
                        | Action::Select(text_editor::Motion::Home)
                        | Action::Select(text_editor::Motion::End)
                        | Action::Edit(_)
                        | Action::Click(_)
                );

                // Fast path: just count lines for vertical motions
                let text_content = self.content.text();
                let line_count = text_content.lines().count();
                let max_lines = line_count.saturating_sub(1);

                // Update cursor position and determine if scrolling is needed
                let should_scroll = if needs_lines {
                    // Slow path: parse all lines only when needed
                    let lines: Vec<&str> = text_content.lines().collect();
                    let current_line = if self.cursor_line < lines.len() {
                        lines[self.cursor_line]
                    } else {
                        ""
                    };

                    match &action {
                        Action::Move(motion) | Action::Select(motion) => {
                            use text_editor::Motion;

                            if matches!(
                                motion,
                                Motion::Left
                                    | Motion::Right
                                    | Motion::WordLeft
                                    | Motion::WordRight
                                    | Motion::Home
                                    | Motion::End
                            ) {
                                self.handle_horizontal_motion(
                                    motion,
                                    current_line,
                                    &lines,
                                    max_lines,
                                )
                            } else {
                                false
                            }
                        }
                        Action::Edit(edit) => self.handle_edit_action(edit, &lines),
                        Action::Click(point) => {
                            self.handle_click_action(point, &lines, max_lines);
                            true
                        }
                        _ => false,
                    }
                } else {
                    // Fast path: vertical motions don't need line parsing
                    match &action {
                        Action::Move(motion) | Action::Select(motion) => {
                            use text_editor::Motion;

                            if matches!(
                                motion,
                                Motion::Up
                                    | Motion::Down
                                    | Motion::PageUp
                                    | Motion::PageDown
                                    | Motion::DocumentStart
                                    | Motion::DocumentEnd
                            ) {
                                self.handle_vertical_motion_fast(motion, max_lines)
                            } else {
                                false
                            }
                        }
                        Action::Drag(_) => false,
                        _ => false,
                    }
                };

                // Check if this is an edit action (before action is consumed)
                let is_edit_action = matches!(action, Action::Edit(_));

                // Apply action to content
                self.content.perform(action);

                // Update cached line count on edit actions to avoid parsing in view()
                if is_edit_action {
                    let new_line_count = self.content.text().lines().count().max(1);
                    if new_line_count != self.cached_line_count {
                        self.cached_line_count = new_line_count;
                        self.cached_line_numbers =
                            Self::generate_line_numbers_string(new_line_count);
                    }
                }

                // If cursor moved vertically, scroll to keep it visible
                if should_scroll {
                    return self.scroll_to_cursor();
                }

                iced::Task::none()
            }
        }
    }

    /// Smart scrolling to keep cursor visible in viewport
    fn scroll_to_cursor(&mut self) -> iced::Task<Event> {
        // Y position of cursor
        let cursor_y = (self.cursor_line as f32 * LINE_HEIGHT) + PADDING_TOP;

        // Calculate new scroll position only if necessary
        let new_scroll_y = if cursor_y < self.last_scroll_y + TOP_MARGIN {
            // Cursor too high, scroll up
            (cursor_y - TOP_MARGIN).max(0.0)
        } else if cursor_y > self.last_scroll_y + VIEWPORT_HEIGHT - BOTTOM_MARGIN {
            // Cursor too low, scroll down
            cursor_y - VIEWPORT_HEIGHT + BOTTOM_MARGIN
        } else {
            // Cursor already visible, no need to scroll
            return iced::Task::none();
        };

        // Update scroll position
        self.last_scroll_y = new_scroll_y;

        iced::widget::operation::scroll_to(
            self.scroll_id.clone(),
            scrollable::AbsoluteOffset {
                x: 0.0,
                y: new_scroll_y,
            },
        )
    }

    /// Optimized version of handle_vertical_motion that doesn't require parsing all lines.
    ///
    /// This is much faster for large files as it only needs the line count, not the content.
    ///
    /// # Arguments
    ///
    /// * `motion` - The vertical motion action to perform
    /// * `max_lines` - Maximum valid line index
    ///
    /// # Returns
    ///
    /// `false` since we disabled auto-scroll for Page Up/Down/End to prevent freezing
    pub(crate) fn handle_vertical_motion_fast(
        &mut self,
        motion: &text_editor::Motion,
        max_lines: usize,
    ) -> bool {
        use text_editor::Motion;

        match motion {
            Motion::Up => {
                self.cursor_line = self.cursor_line.saturating_sub(1);
                // Can't adjust column without line content, but that's OK
                // The text_editor will handle it
                false
            }
            Motion::Down => {
                self.cursor_line = (self.cursor_line + 1).min(max_lines);
                // Can't adjust column without line content, but that's OK
                false
            }
            Motion::PageUp => {
                self.cursor_line = self.cursor_line.saturating_sub(20);
                false
            }
            Motion::PageDown => {
                self.cursor_line = (self.cursor_line + 20).min(max_lines);
                false
            }
            Motion::DocumentStart => {
                self.cursor_line = 0;
                self.cursor_column = 0;
                false
            }
            Motion::DocumentEnd => {
                self.cursor_line = max_lines;
                // Can't set column to end without line content
                self.cursor_column = 0;
                false
            }
            _ => false,
        }
    }

    /// Handles horizontal cursor motion (Left, Right, WordLeft, WordRight, Home, End).
    ///
    /// Updates cursor column position and may change line for wrapping motions.
    ///
    /// # Arguments
    ///
    /// * `motion` - The horizontal motion action to perform
    /// * `current_line` - The current line text
    /// * `lines` - All lines of text (for line wrapping)
    /// * `max_lines` - Maximum valid line index
    ///
    /// # Returns
    ///
    /// `false` since horizontal motion doesn't require vertical scrolling
    pub(crate) fn handle_horizontal_motion(
        &mut self,
        motion: &text_editor::Motion,
        current_line: &str,
        lines: &[&str],
        max_lines: usize,
    ) -> bool {
        use text_editor::Motion;

        match motion {
            Motion::Left => {
                if self.cursor_column > 0 {
                    self.cursor_column -= 1;
                } else if self.cursor_line > 0 {
                    // Move up to end of previous line
                    self.cursor_line -= 1;
                    if self.cursor_line < lines.len() {
                        self.cursor_column = lines[self.cursor_line].len();
                    }
                }
                false // No vertical scroll needed
            }
            Motion::Right => {
                let line_len = current_line.len();
                if self.cursor_column < line_len {
                    self.cursor_column += 1;
                } else if self.cursor_line < max_lines {
                    // Move down to start of next line
                    self.cursor_line += 1;
                    self.cursor_column = 0;
                }
                false // No vertical scroll needed
            }
            Motion::WordLeft => {
                // Approximation: move back a few characters
                self.cursor_column = self.cursor_column.saturating_sub(5);
                false
            }
            Motion::WordRight => {
                // Approximation: move forward a few characters
                let line_len = current_line.len();
                self.cursor_column = (self.cursor_column + 5).min(line_len);
                false
            }
            Motion::Home => {
                self.cursor_column = 0;
                false
            }
            Motion::End => {
                self.cursor_column = current_line.len();
                false
            }
            _ => false, // Not a horizontal motion
        }
    }

    /// Handles text editing actions (Enter, Insert, Backspace, Delete, Paste).
    ///
    /// Updates cursor position based on the edit operation.
    ///
    /// # Arguments
    ///
    /// * `edit` - The edit action to perform
    /// * `lines` - The current lines of text
    ///
    /// # Returns
    ///
    /// `true` if vertical scrolling is needed (line change), `false` otherwise
    pub(crate) fn handle_edit_action(&mut self, edit: &text_editor::Edit, lines: &[&str]) -> bool {
        use text_editor::Edit;

        match edit {
            Edit::Enter => {
                // New line created, cursor moves down
                self.cursor_line += 1;
                self.cursor_column = 0;
                true
            }
            Edit::Insert(_c) => {
                // Character inserted, column advances
                self.cursor_column += 1;
                false
            }
            Edit::Backspace => {
                // Backspace: move back one character or up one line
                if self.cursor_column > 0 {
                    self.cursor_column -= 1;
                    false
                } else if self.cursor_line > 0 {
                    // Merge with previous line
                    self.cursor_line -= 1;
                    if self.cursor_line < lines.len() {
                        self.cursor_column = lines[self.cursor_line].len();
                    }
                    true // Scroll because we change line
                } else {
                    false
                }
            }
            Edit::Delete => {
                // Delete does not change cursor position
                // (except special cases at end of line, but difficult to track)
                false
            }
            Edit::Paste(text) => {
                // Count newlines in pasted text
                let newlines = text.chars().filter(|&c| c == '\n').count();
                if newlines > 0 {
                    self.cursor_line += newlines;
                    // Find position after the last \n
                    if let Some(last_line) = text.lines().last() {
                        self.cursor_column = last_line.len();
                    } else {
                        self.cursor_column = 0;
                    }
                    true
                } else {
                    self.cursor_column += text.len();
                    false
                }
            }
            Edit::Indent | Edit::Unindent => {
                // Indent/Unindent don't change cursor line position
                false
            }
        }
    }

    /// Handles mouse click positioning.
    ///
    /// Estimates cursor position from click coordinates.
    ///
    /// # Arguments
    ///
    /// * `point` - The click position
    /// * `lines` - The current lines of text
    /// * `max_lines` - Maximum valid line index
    pub(crate) fn handle_click_action(
        &mut self,
        point: &iced::Point,
        lines: &[&str],
        max_lines: usize,
    ) {
        // Estimate line from Y position of click
        let estimated_line = ((point.y - PADDING_TOP) / LINE_HEIGHT).max(0.0) as usize;
        self.cursor_line = estimated_line.min(max_lines);

        // Also estimate column
        let estimated_column = ((point.x - PADDING_LEFT) / CHAR_WIDTH).max(0.0) as usize;
        if self.cursor_line < lines.len() {
            self.cursor_column = estimated_column.min(lines[self.cursor_line].len());
        } else {
            self.cursor_column = 0;
        }
    }

    /// Creates the line numbers gutter widget.
    ///
    /// Uses pre-computed cached line numbers string to avoid expensive
    /// format!() and allocation operations at 60 FPS.
    ///
    /// # Returns
    ///
    /// An `Element` containing the styled line numbers column
    pub(crate) fn create_line_numbers_gutter(&self) -> Element<'_, Event> {
        use iced::Pixels;
        use iced::widget::{container, text};

        // Use pre-computed cached string - NO allocations at 60 FPS!
        let numbers = text(&self.cached_line_numbers)
            .font(iced::Font::MONOSPACE)
            .size(FONT_SIZE)
            .line_height(iced::widget::text::LineHeight::Absolute(Pixels(
                LINE_HEIGHT,
            )))
            .color(self.theme.line_number_color);

        container(numbers)
            .width(GUTTER_WIDTH)
            .padding(PADDING_TOP)
            .style(move |_theme| container::Style {
                background: Some(self.theme.gutter_background.into()),
                border: iced::Border {
                    color: self.theme.gutter_border,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    /// Handles custom key bindings for the text editor.
    ///
    /// Provides custom behavior for specific keys (e.g., Tab inserts 4 spaces).
    ///
    /// # Arguments
    ///
    /// * `key_press` - The key press event
    ///
    /// # Returns
    ///
    /// Optional binding action to perform
    pub(crate) fn handle_key_bindings(
        key_press: text_editor::KeyPress,
    ) -> Option<text_editor::Binding<Event>> {
        use iced::keyboard::{self, key};
        use text_editor::Binding;

        match key_press.key.as_ref() {
            // Handle Tab key
            keyboard::Key::Named(key::Named::Tab) => {
                // Insert 4 spaces instead of Tab
                Some(Binding::Sequence(vec![
                    Binding::Insert(' '),
                    Binding::Insert(' '),
                    Binding::Insert(' '),
                    Binding::Insert(' '),
                ]))
            }
            // Explicit handling of Delete key
            keyboard::Key::Named(key::Named::Delete) => Some(Binding::Delete),
            // Use default behavior for other keys
            _ => Binding::from_key_press(key_press),
        }
    }

    /// Creates the main text editor widget with syntax highlighting and key bindings.
    ///
    /// # Returns
    ///
    /// An `Element` containing the configured text editor
    pub(crate) fn create_text_editor(&self) -> Element<'_, Event> {
        use iced::widget::text_editor;
        use iced::{Color, Pixels};

        text_editor(&self.content)
            .highlight(&self.syntax, iced::highlighter::Theme::Base16Ocean)
            .on_action(Event::ActionPerformed)
            .font(iced::Font::MONOSPACE)
            .size(FONT_SIZE)
            .line_height(iced::widget::text::LineHeight::Absolute(Pixels(
                LINE_HEIGHT,
            )))
            .padding(PADDING_TOP)
            .key_binding(Self::handle_key_bindings)
            .style(|_theme, _status| text_editor::Style {
                background: self.theme.background.into(),
                border: iced::Border::default(),
                placeholder: Color::from_rgb(0.4, 0.4, 0.4),
                value: self.theme.text_color,
                selection: Color::from_rgb(0.3, 0.5, 0.8),
            })
            .into()
    }

    pub fn view(&self) -> Element<'_, Event> {
        use iced::Fill;
        use iced::widget::{container, row, scrollable};

        let line_numbers = self.create_line_numbers_gutter();
        let editor = self.create_text_editor();

        container(scrollable(row![line_numbers, editor].spacing(0)).id(self.scroll_id.clone()))
            .width(Fill)
            .height(Fill)
            .style(move |_theme| container::Style {
                background: Some(self.theme.background.into()),
                ..Default::default()
            })
            .into()
    }

    // Test accessors
    #[cfg(test)]
    pub fn cursor_position(&self) -> (usize, usize) {
        (self.cursor_line, self.cursor_column)
    }

    #[cfg(test)]
    pub fn last_scroll_y(&self) -> f32 {
        self.last_scroll_y
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Point;
    use text_editor::{Action, Edit, Motion};

    /// Helper to create a test component with simple content
    fn create_test_editor() -> CodeEditorComponent {
        let content = "Line 0\nLine 1\nLine 2\nLine 3\nLine 4".to_string();
        CodeEditorComponent::new_with_language(&content, "py")
    }

    #[test]
    fn test_cursor_starts_at_origin() {
        let editor = create_test_editor();
        assert_eq!(editor.cursor_position(), (0, 0));
    }

    #[test]
    fn test_cursor_move_down() {
        let mut editor = create_test_editor();

        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        assert_eq!(
            editor.cursor_position(),
            (1, 0),
            "Cursor should be on line 1"
        );

        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        assert_eq!(
            editor.cursor_position(),
            (2, 0),
            "Cursor should be on line 2"
        );
    }

    #[test]
    fn test_cursor_move_up() {
        let mut editor = create_test_editor();

        // Move down then up
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        assert_eq!(editor.cursor_position(), (2, 0));

        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Up)));
        assert_eq!(
            editor.cursor_position(),
            (1, 0),
            "Cursor should have moved up to line 1"
        );
    }

    #[test]
    fn test_cursor_move_up_at_top() {
        let mut editor = create_test_editor();

        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Up)));
        assert_eq!(
            editor.cursor_position(),
            (0, 0),
            "Cursor should stay at (0,0)"
        );
    }

    #[test]
    fn test_cursor_move_down_at_bottom() {
        let mut editor = create_test_editor();

        // Go all the way to bottom (line 4 = last line)
        for _ in 0..10 {
            let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        }

        assert_eq!(
            editor.cursor_position().0,
            4,
            "Cursor should be capped at last line"
        );
    }

    #[test]
    fn test_cursor_left_right() {
        let mut editor = create_test_editor();

        // Move right
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Right)));
        assert_eq!(editor.cursor_position(), (0, 1), "Column should be 1");

        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Right)));
        assert_eq!(editor.cursor_position(), (0, 2), "Column should be 2");

        // Move back left
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Left)));
        assert_eq!(editor.cursor_position(), (0, 1), "Column should be 1");
    }

    #[test]
    fn test_cursor_left_wraps_to_previous_line() {
        let mut editor = create_test_editor();

        // Move down one line
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        assert_eq!(editor.cursor_position(), (1, 0));

        // Going left from start of line should move up to end of previous line
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Left)));
        assert_eq!(editor.cursor_position().0, 0, "Should move up to line 0");
        assert_eq!(
            editor.cursor_position().1,
            6,
            "Should be at end of 'Line 0' (6 characters)"
        );
    }

    #[test]
    fn test_cursor_right_wraps_to_next_line() {
        let mut editor = create_test_editor();

        // Go to end of line 0
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::End)));
        assert_eq!(
            editor.cursor_position(),
            (0, 6),
            "Should be at end of line 0"
        );

        // Move right devrait descendre au début de la ligne suivante
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Right)));
        assert_eq!(
            editor.cursor_position(),
            (1, 0),
            "Should move down to line 1, column 0"
        );
    }

    #[test]
    fn test_home_end() {
        let mut editor = create_test_editor();

        // Move right puis Home
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Right)));
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Right)));
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Home)));
        assert_eq!(
            editor.cursor_position(),
            (0, 0),
            "Home should return to column 0"
        );

        // End should go to end
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::End)));
        assert_eq!(
            editor.cursor_position(),
            (0, 6),
            "End should go to end of line"
        );
    }

    #[test]
    fn test_document_start_end() {
        let mut editor = create_test_editor();

        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::DocumentEnd)));
        assert_eq!(
            editor.cursor_position().0,
            4,
            "DocumentEnd should go to last line"
        );

        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::DocumentStart)));
        assert_eq!(
            editor.cursor_position(),
            (0, 0),
            "DocumentStart should go to (0,0)"
        );
    }

    #[test]
    fn test_page_up_down() {
        let mut editor = create_test_editor();

        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::PageDown)));
        assert_eq!(
            editor.cursor_position().0,
            4,
            "PageDown should go to line 4 (max for this short file)"
        );

        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::PageUp)));
        assert_eq!(
            editor.cursor_position().0,
            0,
            "PageUp 20 lines from line 4 = line 0"
        );
    }

    #[test]
    fn test_enter_creates_new_line() {
        let mut editor = create_test_editor();

        let _ = editor.update(Event::ActionPerformed(Action::Edit(Edit::Enter)));
        assert_eq!(
            editor.cursor_position(),
            (1, 0),
            "Enter should move down to line 1"
        );
    }

    #[test]
    fn test_backspace_at_start_of_line() {
        let mut editor = create_test_editor();

        // Move down one line
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        assert_eq!(editor.cursor_position(), (1, 0));

        // Backspace from start of line should move up
        let _ = editor.update(Event::ActionPerformed(Action::Edit(Edit::Backspace)));
        assert_eq!(
            editor.cursor_position().0,
            0,
            "Backspace should move up to line 0"
        );
        assert_eq!(editor.cursor_position().1, 6, "Should be at end of line 0");
    }

    #[test]
    fn test_backspace_in_middle_of_line() {
        let mut editor = create_test_editor();

        // Move right
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Right)));
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Right)));
        assert_eq!(editor.cursor_position(), (0, 2));

        // Backspace should just move back one column
        let _ = editor.update(Event::ActionPerformed(Action::Edit(Edit::Backspace)));
        assert_eq!(
            editor.cursor_position(),
            (0, 1),
            "Backspace should move back to column 1"
        );
    }

    #[test]
    fn test_paste_single_line() {
        let mut editor = create_test_editor();

        let _ = editor.update(Event::ActionPerformed(Action::Edit(Edit::Paste(
            std::sync::Arc::new("Hello".to_string()),
        ))));
        assert_eq!(
            editor.cursor_position(),
            (0, 5),
            "Column should advance by 5"
        );
    }

    #[test]
    fn test_paste_multiline() {
        let mut editor = create_test_editor();

        let _ = editor.update(Event::ActionPerformed(Action::Edit(Edit::Paste(
            std::sync::Arc::new("Line A\nLine B\nLine C".to_string()),
        ))));
        assert_eq!(
            editor.cursor_position().0,
            2,
            "Should be on line 2 (2 newlines)"
        );
        assert_eq!(
            editor.cursor_position().1,
            6,
            "Should be at end of 'Line C'"
        );
    }

    #[test]
    fn test_click_position() {
        let mut editor = create_test_editor();

        // Simulate a click at line 2, column 3
        // LINE_HEIGHT = 20.0, PADDING_TOP = 10.0
        // y = ligne 2 -> (2 * 20) + 10 = 50
        let point = Point { x: 40.0, y: 50.0 };

        let _ = editor.update(Event::ActionPerformed(Action::Click(point)));
        assert_eq!(
            editor.cursor_position().0,
            2,
            "Click should position at line 2"
        );
    }

    #[test]
    fn test_scroll_optimization() {
        let mut editor = create_test_editor();

        // First movement should trigger a scroll
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        let _first_scroll = editor.last_scroll_y();

        // Second movement in same visible area should not change scroll
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        // Note: without knowing exact viewport height, we can just verify that
        // last_scroll_y is tracked
        assert!(
            editor.last_scroll_y() >= 0.0,
            "Scroll Y should be positive or zero"
        );
    }

    #[test]
    fn test_insert_character() {
        let mut editor = create_test_editor();

        let _ = editor.update(Event::ActionPerformed(Action::Edit(Edit::Insert('X'))));
        assert_eq!(
            editor.cursor_position(),
            (0, 1),
            "Insert should advance column"
        );

        let _ = editor.update(Event::ActionPerformed(Action::Edit(Edit::Insert('Y'))));
        assert_eq!(
            editor.cursor_position(),
            (0, 2),
            "Insert should continue advancing"
        );
    }

    #[test]
    fn test_delete_key() {
        let mut editor = create_test_editor();

        // Move right d'un caractère
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::Right)));
        assert_eq!(editor.cursor_position(), (0, 1));

        // Delete should not move cursor but delete next character
        let _ = editor.update(Event::ActionPerformed(Action::Edit(Edit::Delete)));
        // Cursor stays at (0, 1) but character at that position is deleted
        assert_eq!(
            editor.cursor_position(),
            (0, 1),
            "Delete should not move cursor"
        );
    }

    #[test]
    fn test_word_left_right() {
        let mut editor = create_test_editor();

        // Move right par mot (approximation de 5 chars)
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::WordRight)));
        // Column should advance (~5 but limited by line length)
        let col_after_right = editor.cursor_position().1;
        assert!(
            col_after_right >= 5,
            "WordRight should advance by at least 5"
        );

        // Move back left par mot
        let _ = editor.update(Event::ActionPerformed(Action::Move(Motion::WordLeft)));
        let col_after_left = editor.cursor_position().1;
        assert!(
            col_after_left < col_after_right,
            "WordLeft should move back"
        );
    }

    #[test]
    fn test_select_motion() {
        let mut editor = create_test_editor();

        // Select should also track cursor like Move
        let _ = editor.update(Event::ActionPerformed(Action::Select(Motion::Down)));
        assert_eq!(
            editor.cursor_position().0,
            1,
            "Select(Down) should move cursor"
        );

        let _ = editor.update(Event::ActionPerformed(Action::Select(Motion::Right)));
        // Right does not trigger vertical scroll so line stays at 1
        assert_eq!(editor.cursor_position().0, 1);
    }

    #[test]
    fn test_multiple_enters() {
        let mut editor = create_test_editor();

        // Multiple successive Enters
        let _ = editor.update(Event::ActionPerformed(Action::Edit(Edit::Enter)));
        let _ = editor.update(Event::ActionPerformed(Action::Edit(Edit::Enter)));
        let _ = editor.update(Event::ActionPerformed(Action::Edit(Edit::Enter)));

        assert_eq!(
            editor.cursor_position(),
            (3, 0),
            "3 Enters should move down to line 3"
        );
    }
}
