//! Canvas rendering implementation using Iced's `canvas::Program`.

use iced::mouse;
use iced::widget::canvas::{self, Geometry};
use iced::{Color, Event, Point, Rectangle, Size, Theme, keyboard};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;

use super::{
    ArrowDirection, CHAR_WIDTH, CodeEditor, FONT_SIZE, GUTTER_WIDTH,
    LINE_HEIGHT, Message,
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
            // Background
            frame.fill_rectangle(
                Point::ORIGIN,
                bounds.size(),
                self.style.background,
            );

            // Calculate visible lines (virtual scrolling - Scrollable handles the offset)
            // Since the canvas has full height, we need to draw all lines
            let total_lines = self.buffer.line_count();

            // Draw gutter background (full height)
            frame.fill_rectangle(
                Point::ORIGIN,
                Size::new(GUTTER_WIDTH, bounds.height),
                self.style.gutter_background,
            );

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
                _ => Some(syntax_set.find_syntax_plain_text()),
            };

            // Draw line numbers and text for all lines (Scrollable clips viewport)
            for line_idx in 0..total_lines {
                let y = line_idx as f32 * LINE_HEIGHT;

                // Draw line number
                let line_num_text = format!("{:>4}", line_idx + 1);
                frame.fill_text(canvas::Text {
                    content: line_num_text,
                    position: Point::new(5.0, y + 2.0),
                    color: self.style.line_number_color,
                    size: FONT_SIZE.into(),
                    font: iced::Font::MONOSPACE,
                    ..canvas::Text::default()
                });

                // Highlight current line
                if line_idx == self.cursor.0 {
                    frame.fill_rectangle(
                        Point::new(GUTTER_WIDTH, y),
                        Size::new(bounds.width - GUTTER_WIDTH, LINE_HEIGHT),
                        self.style.current_line_highlight,
                    );
                }

                // Draw text content with syntax highlighting
                let line_content = self.buffer.line(line_idx);

                if let Some(syntax) = syntax_ref {
                    let mut highlighter =
                        HighlightLines::new(syntax, syntax_theme);
                    let ranges = highlighter
                        .highlight_line(line_content, &syntax_set)
                        .unwrap_or_else(|_| {
                            vec![(Style::default(), line_content)]
                        });

                    let mut x_offset = GUTTER_WIDTH + 5.0;
                    for (style, text) in ranges {
                        let color = Color::from_rgb(
                            f32::from(style.foreground.r) / 255.0,
                            f32::from(style.foreground.g) / 255.0,
                            f32::from(style.foreground.b) / 255.0,
                        );

                        frame.fill_text(canvas::Text {
                            content: text.to_string(),
                            position: Point::new(x_offset, y + 2.0),
                            color,
                            size: FONT_SIZE.into(),
                            font: iced::Font::MONOSPACE,
                            ..canvas::Text::default()
                        });

                        x_offset += text.len() as f32 * CHAR_WIDTH;
                    }
                } else {
                    // Fallback to plain text
                    frame.fill_text(canvas::Text {
                        content: line_content.to_string(),
                        position: Point::new(GUTTER_WIDTH + 5.0, y + 2.0),
                        color: self.style.text_color,
                        size: FONT_SIZE.into(),
                        font: iced::Font::MONOSPACE,
                        ..canvas::Text::default()
                    });
                }
            }

            // Draw selection highlight
            if let Some((start, end)) = self.get_selection_range()
                && start != end
            {
                let selection_color = Color { r: 0.3, g: 0.5, b: 0.8, a: 0.3 };

                if start.0 == end.0 {
                    // Single line selection
                    let y = start.0 as f32 * LINE_HEIGHT;
                    let x_start =
                        GUTTER_WIDTH + 5.0 + start.1 as f32 * CHAR_WIDTH;
                    let x_end = GUTTER_WIDTH + 5.0 + end.1 as f32 * CHAR_WIDTH;

                    frame.fill_rectangle(
                        Point::new(x_start, y + 2.0),
                        Size::new(x_end - x_start, LINE_HEIGHT - 4.0),
                        selection_color,
                    );
                } else {
                    // Multi-line selection
                    // First line - from start column to end of line
                    let y_start = start.0 as f32 * LINE_HEIGHT;
                    let x_start =
                        GUTTER_WIDTH + 5.0 + start.1 as f32 * CHAR_WIDTH;
                    let first_line_len = self.buffer.line_len(start.0);
                    let x_end_first =
                        GUTTER_WIDTH + 5.0 + first_line_len as f32 * CHAR_WIDTH;

                    frame.fill_rectangle(
                        Point::new(x_start, y_start + 2.0),
                        Size::new(x_end_first - x_start, LINE_HEIGHT - 4.0),
                        selection_color,
                    );

                    // Middle lines - full width
                    for line_idx in (start.0 + 1)..end.0 {
                        let y = line_idx as f32 * LINE_HEIGHT;
                        let line_len = self.buffer.line_len(line_idx);
                        let width = line_len as f32 * CHAR_WIDTH;

                        frame.fill_rectangle(
                            Point::new(GUTTER_WIDTH + 5.0, y + 2.0),
                            Size::new(width, LINE_HEIGHT - 4.0),
                            selection_color,
                        );
                    }

                    // Last line - from start of line to end column
                    let y_end = end.0 as f32 * LINE_HEIGHT;
                    let x_end = GUTTER_WIDTH + 5.0 + end.1 as f32 * CHAR_WIDTH;

                    frame.fill_rectangle(
                        Point::new(GUTTER_WIDTH + 5.0, y_end + 2.0),
                        Size::new(
                            x_end - (GUTTER_WIDTH + 5.0),
                            LINE_HEIGHT - 4.0,
                        ),
                        selection_color,
                    );
                }
            }

            // Draw cursor
            if self.cursor_visible {
                let cursor_x =
                    GUTTER_WIDTH + 5.0 + self.cursor.1 as f32 * CHAR_WIDTH;
                let cursor_y = self.cursor.0 as f32 * LINE_HEIGHT;

                frame.fill_rectangle(
                    Point::new(cursor_x, cursor_y + 2.0),
                    Size::new(2.0, LINE_HEIGHT - 4.0),
                    self.style.text_color,
                );
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
                // Handle mouse drag for selection
                cursor.position_in(bounds).map(|position| {
                    Action::publish(Message::MouseDrag(position)).and_capture()
                })
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                Some(Action::publish(Message::MouseRelease).and_capture())
            }
            _ => None,
        }
    }
}
