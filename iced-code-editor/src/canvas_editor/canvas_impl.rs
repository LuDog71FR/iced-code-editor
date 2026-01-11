//! Canvas rendering implementation using Iced's `canvas::Program`.

use iced::mouse;
use iced::widget::canvas::{self, Geometry};
use iced::{Color, Event, Point, Rectangle, Size, Theme, keyboard};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;

use super::{
    ArrowDirection, CHAR_WIDTH, CodeEditor, FONT_SIZE, GUTTER_WIDTH,
    LINE_HEIGHT, Message, wrapping::WrappingCalculator,
};
use iced::widget::canvas::Action;

impl canvas::Program<Message> for CodeEditor {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            // Initialize wrapping calculator
            let wrapping_calc =
                WrappingCalculator::new(self.wrap_enabled, self.wrap_column);

            // Calculate visual lines
            let visual_lines = wrapping_calc
                .calculate_visual_lines(&self.buffer, bounds.width);

            // Calculate visible line range based on viewport for optimized rendering
            // Use bounds.height as fallback when viewport_height is not yet initialized
            let effective_viewport_height = if self.viewport_height > 0.0 {
                self.viewport_height
            } else {
                bounds.height
            };
            let first_visible_line =
                (self.viewport_scroll / LINE_HEIGHT).floor() as usize;
            let visible_lines_count =
                (effective_viewport_height / LINE_HEIGHT).ceil() as usize + 2;
            let last_visible_line = (first_visible_line + visible_lines_count)
                .min(visual_lines.len());

            // Load syntax highlighting
            let syntax_set = SyntaxSet::load_defaults_newlines();
            let theme_set = ThemeSet::load_defaults();
            let syntax_theme = &theme_set.themes["base16-ocean.dark"];

            let syntax_ref = match self.syntax.as_str() {
                "py" | "python" => syntax_set.find_syntax_by_extension("py"),
                "lua" => syntax_set.find_syntax_by_extension("lua"),
                "rs" | "rust" => syntax_set.find_syntax_by_extension("rs"),
                "js" | "javascript" => {
                    syntax_set.find_syntax_by_extension("js")
                }
                "html" | "htm" => syntax_set.find_syntax_by_extension("html"),
                "xml" | "svg" => syntax_set.find_syntax_by_extension("xml"),
                "css" => syntax_set.find_syntax_by_extension("css"),
                "json" => syntax_set.find_syntax_by_extension("json"),
                "md" | "markdown" => syntax_set.find_syntax_by_extension("md"),
                _ => Some(syntax_set.find_syntax_plain_text()),
            };

            // Draw only visible lines (virtual scrolling optimization)
            for (idx, visual_line) in visual_lines
                .iter()
                .enumerate()
                .skip(first_visible_line)
                .take(last_visible_line - first_visible_line)
            {
                let y = idx as f32 * LINE_HEIGHT;

                // Note: Gutter background is handled by a container in view.rs
                // to ensure proper clipping when the pane is resized.

                // Draw line number only for first segment
                if visual_line.is_first_segment() {
                    let line_num_text =
                        format!("{:>4}", visual_line.logical_line + 1);
                    frame.fill_text(canvas::Text {
                        content: line_num_text,
                        position: Point::new(5.0, y + 2.0),
                        color: self.style.line_number_color,
                        size: FONT_SIZE.into(),
                        font: iced::Font::MONOSPACE,
                        ..canvas::Text::default()
                    });
                } else {
                    // Draw wrap indicator for continuation lines
                    frame.fill_text(canvas::Text {
                        content: "↪".to_string(),
                        position: Point::new(GUTTER_WIDTH - 20.0, y + 2.0),
                        color: self.style.line_number_color,
                        size: FONT_SIZE.into(),
                        font: iced::Font::MONOSPACE,
                        ..canvas::Text::default()
                    });
                }

                // Highlight current line (based on logical line)
                if visual_line.logical_line == self.cursor.0 {
                    frame.fill_rectangle(
                        Point::new(GUTTER_WIDTH, y),
                        Size::new(bounds.width - GUTTER_WIDTH, LINE_HEIGHT),
                        self.style.current_line_highlight,
                    );
                }

                // Draw text content with syntax highlighting
                let full_line_content =
                    self.buffer.line(visual_line.logical_line);

                // Convert character indices to byte indices for UTF-8 string slicing
                let start_byte = full_line_content
                    .char_indices()
                    .nth(visual_line.start_col)
                    .map_or(full_line_content.len(), |(idx, _)| idx);
                let end_byte = full_line_content
                    .char_indices()
                    .nth(visual_line.end_col)
                    .map_or(full_line_content.len(), |(idx, _)| idx);
                let line_segment = &full_line_content[start_byte..end_byte];

                if let Some(syntax) = syntax_ref {
                    let mut highlighter =
                        HighlightLines::new(syntax, syntax_theme);

                    // Highlight the full line to get correct token colors
                    let full_line_ranges = highlighter
                        .highlight_line(full_line_content, &syntax_set)
                        .unwrap_or_else(|_| {
                            vec![(Style::default(), full_line_content)]
                        });

                    // Extract only the ranges that fall within our segment
                    let mut x_offset = GUTTER_WIDTH + 5.0;
                    let mut char_pos = 0;

                    for (style, text) in full_line_ranges {
                        let text_len = text.chars().count();
                        let text_end = char_pos + text_len;

                        // Check if this token intersects with our segment
                        if text_end > visual_line.start_col
                            && char_pos < visual_line.end_col
                        {
                            // Calculate the intersection
                            let segment_start =
                                char_pos.max(visual_line.start_col);
                            let segment_end = text_end.min(visual_line.end_col);

                            let text_start_offset =
                                segment_start.saturating_sub(char_pos);
                            let text_end_offset = text_start_offset
                                + (segment_end - segment_start);

                            // Convert character offsets to byte offsets for UTF-8 slicing
                            let start_byte = text
                                .char_indices()
                                .nth(text_start_offset)
                                .map_or(text.len(), |(idx, _)| idx);
                            let end_byte = text
                                .char_indices()
                                .nth(text_end_offset)
                                .map_or(text.len(), |(idx, _)| idx);

                            let segment_text = &text[start_byte..end_byte];

                            let color = Color::from_rgb(
                                f32::from(style.foreground.r) / 255.0,
                                f32::from(style.foreground.g) / 255.0,
                                f32::from(style.foreground.b) / 255.0,
                            );

                            frame.fill_text(canvas::Text {
                                content: segment_text.to_string(),
                                position: Point::new(x_offset, y + 2.0),
                                color,
                                size: FONT_SIZE.into(),
                                font: iced::Font::MONOSPACE,
                                ..canvas::Text::default()
                            });

                            x_offset += segment_text.chars().count() as f32
                                * CHAR_WIDTH;
                        }

                        char_pos = text_end;
                    }
                } else {
                    // Fallback to plain text
                    frame.fill_text(canvas::Text {
                        content: line_segment.to_string(),
                        position: Point::new(GUTTER_WIDTH + 5.0, y + 2.0),
                        color: self.style.text_color,
                        size: FONT_SIZE.into(),
                        font: iced::Font::MONOSPACE,
                        ..canvas::Text::default()
                    });
                }
            }

            // Draw search match highlights
            if self.search_state.is_open && !self.search_state.query.is_empty()
            {
                let query_len = self.search_state.query.chars().count();

                for (match_idx, search_match) in
                    self.search_state.matches.iter().enumerate()
                {
                    // Determine if this is the current match
                    let is_current = self.search_state.current_match_index
                        == Some(match_idx);

                    let highlight_color = if is_current {
                        // Orange for current match
                        Color { r: 1.0, g: 0.6, b: 0.0, a: 0.4 }
                    } else {
                        // Yellow for other matches
                        Color { r: 1.0, g: 1.0, b: 0.0, a: 0.3 }
                    };

                    // Convert logical position to visual line
                    let start_visual = WrappingCalculator::logical_to_visual(
                        &visual_lines,
                        search_match.line,
                        search_match.col,
                    );
                    let end_visual = WrappingCalculator::logical_to_visual(
                        &visual_lines,
                        search_match.line,
                        search_match.col + query_len,
                    );

                    if let (Some(start_v), Some(end_v)) =
                        (start_visual, end_visual)
                    {
                        if start_v == end_v {
                            // Match within same visual line
                            let y = start_v as f32 * LINE_HEIGHT;
                            let x_start = GUTTER_WIDTH
                                + 5.0
                                + search_match.col as f32 * CHAR_WIDTH;
                            let x_end = GUTTER_WIDTH
                                + 5.0
                                + (search_match.col + query_len) as f32
                                    * CHAR_WIDTH;

                            frame.fill_rectangle(
                                Point::new(x_start, y + 2.0),
                                Size::new(x_end - x_start, LINE_HEIGHT - 4.0),
                                highlight_color,
                            );
                        } else {
                            // Match spans multiple visual lines
                            for (v_idx, vl) in visual_lines
                                .iter()
                                .enumerate()
                                .skip(start_v)
                                .take(end_v - start_v + 1)
                            {
                                let y = v_idx as f32 * LINE_HEIGHT;

                                let match_start_col = search_match.col;
                                let match_end_col =
                                    search_match.col + query_len;

                                let sel_start_col = if v_idx == start_v {
                                    match_start_col
                                } else {
                                    vl.start_col
                                };
                                let sel_end_col = if v_idx == end_v {
                                    match_end_col
                                } else {
                                    vl.end_col
                                };

                                let x_start = GUTTER_WIDTH
                                    + 5.0
                                    + (sel_start_col - vl.start_col) as f32
                                        * CHAR_WIDTH;
                                let x_end = GUTTER_WIDTH
                                    + 5.0
                                    + (sel_end_col - vl.start_col) as f32
                                        * CHAR_WIDTH;

                                frame.fill_rectangle(
                                    Point::new(x_start, y + 2.0),
                                    Size::new(
                                        x_end - x_start,
                                        LINE_HEIGHT - 4.0,
                                    ),
                                    highlight_color,
                                );
                            }
                        }
                    }
                }
            }

            // Draw selection highlight
            if let Some((start, end)) = self.get_selection_range()
                && start != end
            {
                let selection_color = Color { r: 0.3, g: 0.5, b: 0.8, a: 0.3 };

                if start.0 == end.0 {
                    // Single line selection - need to handle wrapped segments
                    let start_visual = WrappingCalculator::logical_to_visual(
                        &visual_lines,
                        start.0,
                        start.1,
                    );
                    let end_visual = WrappingCalculator::logical_to_visual(
                        &visual_lines,
                        end.0,
                        end.1,
                    );

                    if let (Some(start_v), Some(end_v)) =
                        (start_visual, end_visual)
                    {
                        if start_v == end_v {
                            // Selection within same visual line
                            let y = start_v as f32 * LINE_HEIGHT;
                            let x_start = GUTTER_WIDTH
                                + 5.0
                                + start.1 as f32 * CHAR_WIDTH;
                            let x_end =
                                GUTTER_WIDTH + 5.0 + end.1 as f32 * CHAR_WIDTH;

                            frame.fill_rectangle(
                                Point::new(x_start, y + 2.0),
                                Size::new(x_end - x_start, LINE_HEIGHT - 4.0),
                                selection_color,
                            );
                        } else {
                            // Selection spans multiple visual lines (same logical line)
                            for (v_idx, vl) in visual_lines
                                .iter()
                                .enumerate()
                                .skip(start_v)
                                .take(end_v - start_v + 1)
                            {
                                let y = v_idx as f32 * LINE_HEIGHT;

                                let sel_start_col = if v_idx == start_v {
                                    start.1
                                } else {
                                    vl.start_col
                                };
                                let sel_end_col = if v_idx == end_v {
                                    end.1
                                } else {
                                    vl.end_col
                                };

                                let x_start = GUTTER_WIDTH
                                    + 5.0
                                    + (sel_start_col - vl.start_col) as f32
                                        * CHAR_WIDTH;
                                let x_end = GUTTER_WIDTH
                                    + 5.0
                                    + (sel_end_col - vl.start_col) as f32
                                        * CHAR_WIDTH;

                                frame.fill_rectangle(
                                    Point::new(x_start, y + 2.0),
                                    Size::new(
                                        x_end - x_start,
                                        LINE_HEIGHT - 4.0,
                                    ),
                                    selection_color,
                                );
                            }
                        }
                    }
                } else {
                    // Multi-line selection
                    let start_visual = WrappingCalculator::logical_to_visual(
                        &visual_lines,
                        start.0,
                        start.1,
                    );
                    let end_visual = WrappingCalculator::logical_to_visual(
                        &visual_lines,
                        end.0,
                        end.1,
                    );

                    if let (Some(start_v), Some(end_v)) =
                        (start_visual, end_visual)
                    {
                        for (v_idx, vl) in visual_lines
                            .iter()
                            .enumerate()
                            .skip(start_v)
                            .take(end_v - start_v + 1)
                        {
                            let y = v_idx as f32 * LINE_HEIGHT;

                            let sel_start_col = if vl.logical_line == start.0
                                && v_idx == start_v
                            {
                                start.1
                            } else {
                                vl.start_col
                            };

                            let sel_end_col =
                                if vl.logical_line == end.0 && v_idx == end_v {
                                    end.1
                                } else {
                                    vl.end_col
                                };

                            let x_start = GUTTER_WIDTH
                                + 5.0
                                + (sel_start_col - vl.start_col) as f32
                                    * CHAR_WIDTH;
                            let x_end = GUTTER_WIDTH
                                + 5.0
                                + (sel_end_col - vl.start_col) as f32
                                    * CHAR_WIDTH;

                            frame.fill_rectangle(
                                Point::new(x_start, y + 2.0),
                                Size::new(x_end - x_start, LINE_HEIGHT - 4.0),
                                selection_color,
                            );
                        }
                    }
                }
            }

            // Draw cursor (only when editor has focus)
            if self.cursor_visible && self.is_focused() {
                // Find the visual line containing the cursor
                if let Some(cursor_visual) =
                    WrappingCalculator::logical_to_visual(
                        &visual_lines,
                        self.cursor.0,
                        self.cursor.1,
                    )
                {
                    let vl = &visual_lines[cursor_visual];
                    let cursor_x = GUTTER_WIDTH
                        + 5.0
                        + (self.cursor.1 - vl.start_col) as f32 * CHAR_WIDTH;
                    let cursor_y = cursor_visual as f32 * LINE_HEIGHT;

                    frame.fill_rectangle(
                        Point::new(cursor_x, cursor_y + 2.0),
                        Size::new(2.0, LINE_HEIGHT - 4.0),
                        self.style.text_color,
                    );
                }
            }
        });

        vec![geometry]
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        match event {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            }) => {
                // Only process keyboard events if this editor has focus
                let focused_id = super::FOCUSED_EDITOR_ID
                    .load(std::sync::atomic::Ordering::Relaxed);
                if focused_id != self.editor_id {
                    return None;
                }

                // Handle Ctrl+C / Ctrl+Insert (copy)
                if (modifiers.control()
                    && matches!(key, keyboard::Key::Character(c) if c.as_str() == "c"))
                    || (modifiers.control()
                        && matches!(
                            key,
                            keyboard::Key::Named(keyboard::key::Named::Insert)
                        ))
                {
                    return Some(Action::publish(Message::Copy).and_capture());
                }

                // Handle Ctrl+Z (undo)
                if modifiers.control()
                    && matches!(key, keyboard::Key::Character(z) if z.as_str() == "z")
                {
                    return Some(Action::publish(Message::Undo).and_capture());
                }

                // Handle Ctrl+Y (redo)
                if modifiers.control()
                    && matches!(key, keyboard::Key::Character(y) if y.as_str() == "y")
                {
                    return Some(Action::publish(Message::Redo).and_capture());
                }

                // Handle Ctrl+F (open search)
                if modifiers.control()
                    && matches!(key, keyboard::Key::Character(f) if f.as_str() == "f")
                    && self.search_replace_enabled
                {
                    return Some(
                        Action::publish(Message::OpenSearch).and_capture(),
                    );
                }

                // Handle Ctrl+H (open search and replace)
                if modifiers.control()
                    && matches!(key, keyboard::Key::Character(h) if h.as_str() == "h")
                    && self.search_replace_enabled
                {
                    return Some(
                        Action::publish(Message::OpenSearchReplace)
                            .and_capture(),
                    );
                }

                // Handle Escape (close search dialog if open)
                if matches!(
                    key,
                    keyboard::Key::Named(keyboard::key::Named::Escape)
                ) {
                    return Some(
                        Action::publish(Message::CloseSearch).and_capture(),
                    );
                }

                // Handle Tab (cycle forward in search dialog if open)
                if matches!(
                    key,
                    keyboard::Key::Named(keyboard::key::Named::Tab)
                ) && self.search_state.is_open
                {
                    if modifiers.shift() {
                        // Shift+Tab: cycle backward
                        return Some(
                            Action::publish(Message::SearchDialogShiftTab)
                                .and_capture(),
                        );
                    } else {
                        // Tab: cycle forward
                        return Some(
                            Action::publish(Message::SearchDialogTab)
                                .and_capture(),
                        );
                    }
                }

                // Handle F3 (find next) and Shift+F3 (find previous)
                if matches!(key, keyboard::Key::Named(keyboard::key::Named::F3))
                    && self.search_replace_enabled
                {
                    if modifiers.shift() {
                        return Some(
                            Action::publish(Message::FindPrevious)
                                .and_capture(),
                        );
                    } else {
                        return Some(
                            Action::publish(Message::FindNext).and_capture(),
                        );
                    }
                }

                // Handle Ctrl+V / Shift+Insert (paste) - read clipboard and send paste message
                if (modifiers.control()
                    && matches!(key, keyboard::Key::Character(v) if v.as_str() == "v"))
                    || (modifiers.shift()
                        && matches!(
                            key,
                            keyboard::Key::Named(keyboard::key::Named::Insert)
                        ))
                {
                    // Return an action that requests clipboard read
                    return Some(Action::publish(
                        Message::Paste(String::new()),
                    ));
                }

                // Handle Ctrl+Home (go to start of document)
                if modifiers.control()
                    && matches!(
                        key,
                        keyboard::Key::Named(keyboard::key::Named::Home)
                    )
                {
                    return Some(
                        Action::publish(Message::CtrlHome).and_capture(),
                    );
                }

                // Handle Ctrl+End (go to end of document)
                if modifiers.control()
                    && matches!(
                        key,
                        keyboard::Key::Named(keyboard::key::Named::End)
                    )
                {
                    return Some(
                        Action::publish(Message::CtrlEnd).and_capture(),
                    );
                }

                // Handle Shift+Delete (delete selection)
                if modifiers.shift()
                    && matches!(
                        key,
                        keyboard::Key::Named(keyboard::key::Named::Delete)
                    )
                {
                    return Some(
                        Action::publish(Message::DeleteSelection).and_capture(),
                    );
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
                            Action::publish(Message::CharacterInput(
                                first_char,
                            ))
                            .and_capture(),
                        );
                    }
                }

                // PRIORITY 2: Handle special named keys (navigation, editing)
                // These are only processed if text didn't contain a printable character
                let message = match key {
                    keyboard::Key::Named(keyboard::key::Named::Backspace) => {
                        Some(Message::Backspace)
                    }
                    keyboard::Key::Named(keyboard::key::Named::Delete) => {
                        Some(Message::Delete)
                    }
                    keyboard::Key::Named(keyboard::key::Named::Enter) => {
                        Some(Message::Enter)
                    }
                    keyboard::Key::Named(keyboard::key::Named::Tab) => {
                        // Insert 4 spaces for Tab
                        Some(Message::Tab)
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                        Some(Message::ArrowKey(
                            ArrowDirection::Up,
                            modifiers.shift(),
                        ))
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                        Some(Message::ArrowKey(
                            ArrowDirection::Down,
                            modifiers.shift(),
                        ))
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                        Some(Message::ArrowKey(
                            ArrowDirection::Left,
                            modifiers.shift(),
                        ))
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                        Some(Message::ArrowKey(
                            ArrowDirection::Right,
                            modifiers.shift(),
                        ))
                    }
                    keyboard::Key::Named(keyboard::key::Named::PageUp) => {
                        Some(Message::PageUp)
                    }
                    keyboard::Key::Named(keyboard::key::Named::PageDown) => {
                        Some(Message::PageDown)
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
                                .map(Message::CharacterInput)
                                .map(|msg| Action::publish(msg).and_capture());
                        }
                        None
                    }
                };

                message.map(|msg| Action::publish(msg).and_capture())
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                cursor.position_in(bounds).map(|position| {
                    // Don't capture the event so it can bubble up for focus management
                    Action::publish(Message::MouseClick(position))
                })
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                // Handle mouse drag for selection only when cursor is within bounds
                cursor.position_in(bounds).map(|position| {
                    Action::publish(Message::MouseDrag(position)).and_capture()
                })
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                // Only handle mouse release when cursor is within bounds
                // This prevents capturing events meant for other widgets
                if cursor.is_over(bounds) {
                    Some(Action::publish(Message::MouseRelease).and_capture())
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
