//! Canvas rendering implementation using Iced's `canvas::Program`.

use iced::mouse;
use iced::widget::canvas::{self, Geometry};
use iced::{Color, Event, Point, Rectangle, Size, Theme, keyboard};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;

use super::{
    ArrowDirection, CHAR_WIDTH, CanvasEditor, CanvasEditorMessage, FONT_SIZE, GUTTER_WIDTH,
    LINE_HEIGHT,
};
use iced::widget::canvas::Action;

impl canvas::Program<CanvasEditorMessage> for CanvasEditor {
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
            frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.theme.background);

            // Calculate visible lines (virtual scrolling - Scrollable handles the offset)
            // Since the canvas has full height, we need to draw all lines
            let total_lines = self.buffer.line_count();

            // Draw gutter background (full height)
            frame.fill_rectangle(
                Point::ORIGIN,
                Size::new(GUTTER_WIDTH, bounds.height),
                self.theme.gutter_background,
            );

            // Load syntax highlighting
            let syntax_set = SyntaxSet::load_defaults_newlines();
            let theme_set = ThemeSet::load_defaults();
            let syntax_theme = &theme_set.themes["base16-ocean.dark"];

            let syntax_ref = match self.syntax.as_str() {
                "py" | "python" => syntax_set.find_syntax_by_extension("py"),
                "lua" => syntax_set.find_syntax_by_extension("lua"),
                "rs" | "rust" => syntax_set.find_syntax_by_extension("rs"),
                "js" | "javascript" => syntax_set.find_syntax_by_extension("js"),
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
                    color: self.theme.line_number_color,
                    size: FONT_SIZE.into(),
                    font: iced::Font::MONOSPACE,
                    ..canvas::Text::default()
                });

                // Highlight current line
                if line_idx == self.cursor.0 {
                    frame.fill_rectangle(
                        Point::new(GUTTER_WIDTH, y),
                        Size::new(bounds.width - GUTTER_WIDTH, LINE_HEIGHT),
                        Color::from_rgb(0.15, 0.15, 0.2),
                    );
                }

                // Draw text content with syntax highlighting
                let line_content = self.buffer.line(line_idx);

                if let Some(syntax) = syntax_ref {
                    let mut highlighter = HighlightLines::new(syntax, syntax_theme);
                    let ranges = highlighter
                        .highlight_line(line_content, &syntax_set)
                        .unwrap_or_else(|_| vec![(Style::default(), line_content)]);

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
                        color: self.theme.text_color,
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
                let selection_color = Color {
                    r: 0.3,
                    g: 0.5,
                    b: 0.8,
                    a: 0.3,
                };

                if start.0 == end.0 {
                    // Single line selection
                    let y = start.0 as f32 * LINE_HEIGHT;
                    let x_start = GUTTER_WIDTH + 5.0 + start.1 as f32 * CHAR_WIDTH;
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
                    let x_start = GUTTER_WIDTH + 5.0 + start.1 as f32 * CHAR_WIDTH;
                    let first_line_len = self.buffer.line_len(start.0);
                    let x_end_first = GUTTER_WIDTH + 5.0 + first_line_len as f32 * CHAR_WIDTH;

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
                        Size::new(x_end - (GUTTER_WIDTH + 5.0), LINE_HEIGHT - 4.0),
                        selection_color,
                    );
                }
            }

            // Draw cursor
            if self.cursor_visible {
                let cursor_x = GUTTER_WIDTH + 5.0 + self.cursor.1 as f32 * CHAR_WIDTH;
                let cursor_y = self.cursor.0 as f32 * LINE_HEIGHT;

                frame.fill_rectangle(
                    Point::new(cursor_x, cursor_y + 2.0),
                    Size::new(2.0, LINE_HEIGHT - 4.0),
                    self.theme.text_color,
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
    ) -> Option<Action<CanvasEditorMessage>> {
        match event {
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                // Handle Ctrl+C (copy)
                if modifiers.control()
                    && matches!(key, keyboard::Key::Character(c) if c.as_str() == "c")
                {
                    return Some(Action::publish(CanvasEditorMessage::Copy).and_capture());
                }

                // Handle Ctrl+V (paste) - read clipboard and send paste message
                if modifiers.control()
                    && matches!(key, keyboard::Key::Character(v) if v.as_str() == "v")
                {
                    // Return an action that requests clipboard read
                    return Some(Action::publish(CanvasEditorMessage::Paste(String::new())));
                }

                let message = match key {
                    keyboard::Key::Character(c) if !modifiers.control() => {
                        c.chars().next().map(CanvasEditorMessage::CharacterInput)
                    }
                    keyboard::Key::Named(keyboard::key::Named::Backspace) => {
                        Some(CanvasEditorMessage::Backspace)
                    }
                    keyboard::Key::Named(keyboard::key::Named::Delete) => {
                        Some(CanvasEditorMessage::Delete)
                    }
                    keyboard::Key::Named(keyboard::key::Named::Enter) => {
                        Some(CanvasEditorMessage::Enter)
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowUp) => Some(
                        CanvasEditorMessage::ArrowKey(ArrowDirection::Up, modifiers.shift()),
                    ),
                    keyboard::Key::Named(keyboard::key::Named::ArrowDown) => Some(
                        CanvasEditorMessage::ArrowKey(ArrowDirection::Down, modifiers.shift()),
                    ),
                    keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => Some(
                        CanvasEditorMessage::ArrowKey(ArrowDirection::Left, modifiers.shift()),
                    ),
                    keyboard::Key::Named(keyboard::key::Named::ArrowRight) => Some(
                        CanvasEditorMessage::ArrowKey(ArrowDirection::Right, modifiers.shift()),
                    ),
                    keyboard::Key::Named(keyboard::key::Named::PageUp) => {
                        Some(CanvasEditorMessage::PageUp)
                    }
                    keyboard::Key::Named(keyboard::key::Named::PageDown) => {
                        Some(CanvasEditorMessage::PageDown)
                    }
                    keyboard::Key::Named(keyboard::key::Named::Home) => {
                        Some(CanvasEditorMessage::Home(modifiers.shift()))
                    }
                    keyboard::Key::Named(keyboard::key::Named::End) => {
                        Some(CanvasEditorMessage::End(modifiers.shift()))
                    }
                    _ => None,
                };

                message.map(|msg| Action::publish(msg).and_capture())
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                cursor.position_in(bounds).map(|position| {
                    // Don't capture the event so it can bubble up for focus management
                    Action::publish(CanvasEditorMessage::MouseClick(position))
                })
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                // Handle mouse drag for selection
                cursor.position_in(bounds).map(|position| {
                    Action::publish(CanvasEditorMessage::MouseDrag(position)).and_capture()
                })
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                Some(Action::publish(CanvasEditorMessage::MouseRelease).and_capture())
            }
            _ => None,
        }
    }
}
