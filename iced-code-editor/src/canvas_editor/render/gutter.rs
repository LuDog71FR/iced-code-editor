//! Gutter rendering: line numbers, wrap indicators, and fold chevrons for
//! [`CodeEditor`].

use iced::Point;
use iced::widget::canvas;

use super::text::RenderContext;
use super::wrapping::VisualLine;
use crate::canvas_editor::features::folding;
use crate::canvas_editor::{CodeEditor, measure_text_width};

impl CodeEditor {
    /// Draws line numbers and wrap indicators in the gutter area.
    ///
    /// # Arguments
    ///
    /// * `frame` - The canvas frame to draw on
    /// * `ctx` - Rendering context containing visual lines and metrics
    /// * `visual_line` - The visual line to render
    /// * `y` - Y position for rendering
    pub(super) fn draw_line_numbers(
        &self,
        frame: &mut canvas::Frame,
        ctx: &RenderContext,
        visual_line: &VisualLine,
        y: f32,
    ) {
        // The line-number area is the left part of the gutter; the fold margin
        // (when folding is enabled) is the right strip adjacent to the text.
        let number_area_width = self.line_number_gutter_width();

        if self.line_numbers_enabled {
            if visual_line.is_first_segment() {
                // Draw line number for first segment, centered in the number area.
                let line_num = visual_line.logical_line + 1;
                let line_num_text = format!("{}", line_num);
                let text_width = measure_text_width(
                    &line_num_text,
                    ctx.full_char_width,
                    ctx.char_width,
                );
                let x_pos = (number_area_width - text_width) / 2.0;
                frame.fill_text(canvas::Text {
                    content: line_num_text,
                    position: Point::new(x_pos, y + 2.0),
                    color: self.style.line_number_color,
                    size: ctx.font_size.into(),
                    font: ctx.font,
                    ..canvas::Text::default()
                });
            } else {
                // Draw wrap indicator for continuation lines.
                frame.fill_text(canvas::Text {
                    content: "↪".to_string(),
                    position: Point::new(number_area_width - 20.0, y + 2.0),
                    color: self.style.line_number_color,
                    size: ctx.font_size.into(),
                    font: ctx.font,
                    ..canvas::Text::default()
                });
            }
        }

        self.draw_fold_chevron(frame, ctx, visual_line, y, number_area_width);
    }

    /// Draws the fold chevron in the fold margin for a foldable header line.
    ///
    /// Draws nothing when folding is disabled, on continuation (wrapped)
    /// segments, or on lines that are not fold headers.
    ///
    /// # Arguments
    ///
    /// * `frame` - The canvas frame to draw on
    /// * `ctx` - Rendering context containing metrics
    /// * `visual_line` - The visual line to render
    /// * `y` - Y position for rendering
    /// * `number_area_width` - Width of the line-number area (start of the fold margin)
    fn draw_fold_chevron(
        &self,
        frame: &mut canvas::Frame,
        ctx: &RenderContext,
        visual_line: &VisualLine,
        y: f32,
        number_area_width: f32,
    ) {
        if !self.folding_enabled || !visual_line.is_first_segment() {
            return;
        }

        if !folding::is_line_fold_header(&self.buffer, visual_line.logical_line)
        {
            return;
        }

        // `▶` when collapsed, `▼` when expanded.
        let chevron = if self.is_folded(visual_line.logical_line) {
            "▶"
        } else {
            "▼"
        };
        frame.fill_text(canvas::Text {
            content: chevron.to_string(),
            position: Point::new(number_area_width + 1.0, y + 2.0),
            color: self.style.line_number_color,
            size: ctx.font_size.into(),
            font: ctx.font,
            ..canvas::Text::default()
        });
    }

    /// Draws a `⋯` marker after the text of a collapsed fold header, signalling
    /// that lines are hidden below it (VS Code-style cue).
    ///
    /// Draws nothing unless folding is enabled and `visual_line` is the header
    /// of a currently collapsed region. Intended to be called inside the clipped
    /// code area so the marker cannot bleed into the gutter.
    pub(super) fn draw_fold_collapsed_marker(
        &self,
        frame: &mut canvas::Frame,
        ctx: &RenderContext,
        visual_line: &VisualLine,
        y: f32,
    ) {
        if !self.folding_enabled
            || !visual_line.is_first_segment()
            || !self.is_folded(visual_line.logical_line)
        {
            return;
        }

        let line_content = self.buffer.line(visual_line.logical_line);
        let line_width = measure_text_width(
            line_content,
            ctx.full_char_width,
            ctx.char_width,
        );
        let x = ctx.gutter_width + 5.0 - ctx.horizontal_scroll_offset
            + line_width
            + 6.0;
        frame.fill_text(canvas::Text {
            content: "⋯".to_string(),
            position: Point::new(x, y + 2.0),
            color: self.style.line_number_color,
            size: ctx.font_size.into(),
            font: ctx.font,
            ..canvas::Text::default()
        });
    }
}
