//! Canvas rendering implementation using Iced's `canvas::Program`.

use iced::mouse;
use iced::widget::canvas::{self, Action, Geometry};
use iced::{Event, Rectangle, Theme, keyboard};
use std::rc::Rc;

use super::text::RenderContext;
use super::wrapping::VisualLine;
use crate::canvas_editor::features::color_preview;
use crate::canvas_editor::{CodeEditor, HIGHLIGHT_LINES_PER_FRAME, Message};

impl canvas::Program<Message> for CodeEditor {
    type State = ();

    /// Renders the code editor's visual elements on the canvas, including text layout, syntax highlighting,
    /// cursor positioning, and other graphical aspects.
    ///
    /// # Arguments
    ///
    /// * `state` - The current state of the canvas
    /// * `renderer` - The renderer used for drawing
    /// * `theme` - The theme for styling
    /// * `bounds` - The rectangle bounds of the canvas
    /// * `cursor` - The mouse cursor position
    ///
    /// # Returns
    ///
    /// A vector of `Geometry` objects representing the drawn elements
    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        // Track the real, live canvas width every frame, independent of the
        // vertical-scroll-driven `viewport_width` (which never gets corrected
        // for content that doesn't overflow vertically — see issue #26).
        self.last_canvas_width.set(bounds.width);

        let visual_lines: Rc<Vec<VisualLine>> =
            self.visual_lines_cached(bounds.width);

        // Prefer the tracked viewport height when available, but fall back to
        // the current bounds during initial layout when viewport metrics have
        // not been populated yet.
        let effective_viewport_height = if self.viewport_height > 0.0 {
            self.viewport_height
        } else {
            bounds.height
        };
        let first_visible_line =
            (self.viewport_scroll / self.line_height).floor() as usize;
        let visible_lines_count =
            (effective_viewport_height / self.line_height).ceil() as usize + 2;
        let last_visible_line =
            (first_visible_line + visible_lines_count).min(visual_lines.len());

        let (start_idx, end_idx) =
            if self.cache_window_end_line > self.cache_window_start_line {
                let s = self.cache_window_start_line.min(visual_lines.len());
                let e = self.cache_window_end_line.min(visual_lines.len());
                (s, e)
            } else {
                (first_visible_line, last_visible_line)
            };

        // Split rendering into two cached layers:
        // - content: expensive, mostly static text/gutter rendering
        // - overlay: frequently changing highlights/cursor/IME
        //
        // This keeps selection dragging and cursor blinking smooth by avoiding
        // invalidation of the text layer on every overlay update.
        let visual_lines_for_content = visual_lines.clone();
        let content_geometry =
            self.content_cache.draw(renderer, bounds.size(), |frame| {
                // Bound sequential syntect catch-up work for this frame. This
                // keeps a deep jump or a cache truncation in a huge file from
                // blocking the UI while parsing every preceding line.
                self.highlight_lines_remaining.set(HIGHLIGHT_LINES_PER_FRAME);

                // Shared with the sticky-scroll layer, which colors the same
                // lines outside the canvas.
                let (syntax_set, syntax_ref, syntax_theme) =
                    self.resolve_syntax();

                let ctx = RenderContext {
                    visual_lines: visual_lines_for_content.as_ref(),
                    bounds_width: bounds.width,
                    gutter_width: self.gutter_width(),
                    line_height: self.line_height,
                    font_size: self.font_size,
                    full_char_width: self.full_char_width,
                    char_width: self.char_width,
                    font: self.font,
                    horizontal_scroll_offset: self.horizontal_scroll_offset,
                };

                // Clip code text to the code area (right of gutter) so that
                // horizontal scrolling cannot cause text to bleed into the gutter.
                // Note: iced renders ALL text on top of ALL geometry, so a
                // fill_rectangle cannot mask text bleed — with_clip is required.
                let code_clip = Rectangle {
                    x: ctx.gutter_width,
                    y: 0.0,
                    width: (bounds.width - ctx.gutter_width).max(0.0),
                    height: bounds.height,
                };
                frame.with_clip(code_clip, |f| {
                    // Scoped to this pass: the memo is keyed on line index
                    // alone, so it must not outlive a buffer that cannot
                    // change underneath it.
                    let mut color_literals =
                        color_preview::LineLiterals::default();

                    for (idx, visual_line) in visual_lines_for_content
                        .iter()
                        .enumerate()
                        .skip(start_idx)
                        .take(end_idx.saturating_sub(start_idx))
                    {
                        let y = idx as f32 * self.line_height;
                        self.draw_indent_guides(f, &ctx, visual_line, y);
                        self.draw_text_with_syntax_highlighting(
                            f,
                            &ctx,
                            visual_line,
                            y,
                            syntax_ref,
                            syntax_set,
                            syntax_theme,
                        );
                        self.draw_bracket_pair_colors(f, &ctx, visual_line, y);
                        self.draw_color_swatches(
                            f,
                            &ctx,
                            visual_line,
                            y,
                            &mut color_literals,
                        );
                        self.draw_fold_collapsed_marker(
                            f,
                            &ctx,
                            visual_line,
                            y,
                        );
                    }
                });

                // Draw line numbers in the gutter (no clip — fixed position)
                for (idx, visual_line) in visual_lines_for_content
                    .iter()
                    .enumerate()
                    .skip(start_idx)
                    .take(end_idx.saturating_sub(start_idx))
                {
                    let y = idx as f32 * self.line_height;
                    self.draw_line_numbers(frame, &ctx, visual_line, y);
                }
            });

        let visual_lines_for_overlay = visual_lines;
        let overlay_geometry =
            self.overlay_cache.draw(renderer, bounds.size(), |frame| {
                // The overlay layer shares the same visual lines, but draws only
                // elements that change without modifying the buffer content.
                let ctx = RenderContext {
                    visual_lines: visual_lines_for_overlay.as_ref(),
                    bounds_width: bounds.width,
                    gutter_width: self.gutter_width(),
                    line_height: self.line_height,
                    font_size: self.font_size,
                    full_char_width: self.full_char_width,
                    char_width: self.char_width,
                    font: self.font,
                    horizontal_scroll_offset: self.horizontal_scroll_offset,
                };

                for (idx, visual_line) in visual_lines_for_overlay
                    .iter()
                    .enumerate()
                    .skip(start_idx)
                    .take(end_idx.saturating_sub(start_idx))
                {
                    let y = idx as f32 * self.line_height;
                    self.draw_current_line_highlight(
                        frame,
                        &ctx,
                        visual_line,
                        y,
                    );
                }

                self.draw_search_highlights(frame, &ctx, start_idx, end_idx);
                self.draw_matching_bracket_highlight(frame, &ctx);
                self.draw_selection_highlight(frame, &ctx);
                self.draw_jump_link_highlight(frame, &ctx, bounds, _cursor);
                self.draw_cursor(frame, &ctx);
            });

        vec![content_geometry, overlay_geometry]
    }

    /// Handles Canvas trait events, specifically keyboard input events and focus management for the code editor widget.
    ///
    /// # Arguments
    ///
    /// * `_state` - The mutable state of the canvas (unused in this implementation)
    /// * `event` - The input event to handle, such as keyboard presses
    /// * `bounds` - The rectangle bounds of the canvas widget
    /// * `cursor` - The current mouse cursor position and status
    ///
    /// # Returns
    ///
    /// An optional `Action<Message>` to perform, such as sending a message or redrawing the canvas
    fn update(
        &self,
        _state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        match event {
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                self.modifiers.set(*modifiers);
                None
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modified_key,
                modifiers,
                text,
                ..
            }) => {
                self.modifiers.set(*modifiers);
                self.handle_keyboard_event(
                    key,
                    modified_key,
                    modifiers,
                    text,
                    bounds,
                    &cursor,
                )
            }
            Event::Keyboard(keyboard::Event::KeyReleased {
                modifiers, ..
            }) => {
                self.modifiers.set(*modifiers);
                None
            }
            Event::Mouse(mouse_event) => {
                self.handle_mouse_event(mouse_event, bounds, &cursor)
            }
            Event::InputMethod(ime_event) => {
                self.handle_ime_event(ime_event, bounds, &cursor)
            }
            _ => None,
        }
    }

    /// Uses the text-selection cursor over the editable code area.
    ///
    /// The gutter keeps the default cursor, except for an interactive fold
    /// chevron. Checking the cursor against `bounds` is important because a
    /// canvas program's interaction can otherwise remain active out of bounds.
    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        let Some(position) = cursor.position_in(bounds) else {
            return mouse::Interaction::default();
        };

        if self.fold_header_at_point(position).is_some() {
            mouse::Interaction::Pointer
        } else if position.x >= self.gutter_width() {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use iced::{Point, Size};

    use super::*;

    fn editor_mouse_interaction(
        editor: &CodeEditor,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        canvas::Program::<Message>::mouse_interaction(
            editor,
            &(),
            bounds,
            cursor,
        )
    }

    #[test]
    fn test_mouse_interaction_uses_text_cursor_in_editable_area() {
        let editor = CodeEditor::new("fn main() {}", "rs");
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(800.0, 600.0));
        let cursor = mouse::Cursor::Available(Point::new(
            editor.gutter_width() + 10.0,
            10.0,
        ));

        assert_eq!(
            editor_mouse_interaction(&editor, bounds, cursor),
            mouse::Interaction::Text
        );
    }

    #[test]
    fn test_mouse_interaction_keeps_default_cursor_in_gutter_and_outside() {
        let editor = CodeEditor::new("fn main() {}", "rs");
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(800.0, 600.0));

        assert_eq!(
            editor_mouse_interaction(
                &editor,
                bounds,
                mouse::Cursor::Available(Point::new(5.0, 10.0)),
            ),
            mouse::Interaction::default()
        );
        assert_eq!(
            editor_mouse_interaction(
                &editor,
                bounds,
                mouse::Cursor::Available(Point::new(900.0, 10.0)),
            ),
            mouse::Interaction::default()
        );
    }
}
