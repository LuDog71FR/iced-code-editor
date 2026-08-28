//! Iced UI view and rendering logic.

use iced::Size;
use iced::advanced::input_method;
use iced::alignment;
use iced::mouse;
use iced::widget::canvas::Canvas;
use iced::widget::{
    Column, MouseArea, Row, Scrollable, Space, container, scrollable, text,
};
use iced::{Background, Border, Color, Element, Length, Rectangle, Shadow};
use iced_aw::ContextMenu;

use super::text::{expand_tabs, expand_tabs_visible};
use super::wrapping::{self, WrappingCalculator};
use crate::canvas_editor::features::command_palette::dialog as command_palette_dialog;
use crate::canvas_editor::features::context_menu;
use crate::canvas_editor::features::goto_line::dialog as goto_line_dialog;
use crate::canvas_editor::features::search::dialog as search_dialog;
use crate::canvas_editor::features::sticky_scroll;
use crate::canvas_editor::input::ime_requester::ImeRequester;
use crate::canvas_editor::{CodeEditor, GUTTER_WIDTH, Message, TAB_WIDTH};
use std::rc::Rc;

/// Horizontal gap in pixels between the gutter and the code text, matching the
/// offset the canvas text layer uses so pinned headers line up with the code.
const CODE_TEXT_LEFT_PADDING: f32 = 5.0;

/// Thickness in pixels of the separator drawn under the sticky-scroll headers.
const STICKY_SEPARATOR_WIDTH: f32 = 1.0;

/// Builds the transparent-container scrollable style shared by the canvas
/// editor's vertical and horizontal scrollbars.
fn canvas_scrollbar_style(
    scrollbar_bg: Color,
    scroller_color: Color,
) -> scrollable::Style {
    scrollable::Style {
        container: container::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            ..container::Style::default()
        },
        vertical_rail: scrollable_rail(scrollbar_bg, scroller_color, 4.0),
        horizontal_rail: scrollable_rail(scrollbar_bg, scroller_color, 4.0),
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: Color::TRANSPARENT.into(),
            border: Border::default(),
            shadow: Shadow::default(),
            icon: Color::TRANSPARENT,
        },
    }
}

impl CodeEditor {
    /// Calculates visual lines and canvas height for the editor.
    ///
    /// Returns a tuple of (visual_lines, canvas_height) where:
    /// - visual_lines: The visual line mapping with wrapping applied
    /// - canvas_height: The total height needed for the canvas
    fn calculate_canvas_height(&self) -> (Rc<Vec<wrapping::VisualLine>>, f32) {
        // Reuse memoized visual lines so view layout (canvas height + IME cursor rect)
        // does not trigger repeated wrapping computation.
        let visual_lines = self.visual_lines_cached(self.viewport_width);
        let total_visual_lines = visual_lines.len();
        let content_height = total_visual_lines as f32 * self.line_height;

        // Use max of content height and viewport height to ensure the canvas
        // always covers the visible area (prevents visual artifacts when
        // content is shorter than viewport after reset/file change)
        let canvas_height = content_height.max(self.viewport_height);

        (visual_lines, canvas_height)
    }

    /// Creates the scrollable style function with custom colors.
    ///
    /// Returns a style function that configures the scrollbar appearance.
    fn create_scrollable_style(
        &self,
    ) -> impl Fn(&iced::Theme, scrollable::Status) -> scrollable::Style {
        let scrollbar_bg = self.style.scrollbar_background;
        let scroller_color = self.style.scroller_color;

        move |_theme, _status| {
            canvas_scrollbar_style(scrollbar_bg, scroller_color)
        }
    }

    /// Creates the canvas widget wrapped in a scrollable container.
    ///
    /// # Arguments
    ///
    /// * `canvas_height` - The total height of the canvas
    ///
    /// # Returns
    ///
    /// A configured scrollable widget containing the canvas
    fn create_canvas_with_scrollable(
        &self,
        canvas_height: f32,
    ) -> Scrollable<'_, Message> {
        let canvas = Canvas::new(self)
            .width(Length::Fill)
            .height(Length::Fixed(canvas_height));

        Scrollable::new(canvas)
            .id(self.scrollable_id.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .on_scroll(Message::Scrolled)
            .style(self.create_scrollable_style())
    }

    /// Creates the horizontal scrollbar element when wrap is disabled and content overflows.
    ///
    /// # Arguments
    ///
    /// * `max_content_width` - The total pixel width of the widest line
    ///
    /// # Returns
    ///
    /// `Some(element)` if a horizontal scrollbar is needed, `None` otherwise
    fn create_horizontal_scrollbar(
        &self,
        max_content_width: f32,
    ) -> Option<Element<'_, Message>> {
        if self.wrap_enabled || max_content_width <= self.viewport_width {
            return None;
        }

        let scrollbar_bg = self.style.scrollbar_background;
        let scroller_color = self.style.scroller_color;

        let h_scrollable = Scrollable::new(
            Space::new().width(Length::Fixed(max_content_width)).height(0.0),
        )
        .id(self.horizontal_scrollable_id.clone())
        .width(Length::Fill)
        .height(Length::Fixed(12.0))
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new(),
        ))
        .on_scroll(Message::HorizontalScrolled)
        .style(move |_theme, _status| {
            canvas_scrollbar_style(scrollbar_bg, scroller_color)
        });

        Some(h_scrollable.into())
    }

    /// Creates the gutter background container if line numbers are enabled.
    ///
    /// # Returns
    ///
    /// Some(container) if line numbers are enabled, None otherwise
    fn create_gutter_container(
        &self,
    ) -> Option<container::Container<'_, Message>> {
        if self.line_numbers_enabled {
            let gutter_background = self.style.gutter_background;
            Some(
                container(
                    Space::new().width(Length::Fill).height(Length::Fill),
                )
                .width(Length::Fixed(GUTTER_WIDTH))
                .height(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(gutter_background)),
                    ..container::Style::default()
                }),
            )
        } else {
            None
        }
    }

    /// Creates the code area background container.
    ///
    /// # Returns
    ///
    /// The code background container widget
    fn create_code_background_container(
        &self,
    ) -> container::Container<'_, Message> {
        let background_color = self.style.background;
        container(Space::new().width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(background_color)),
                ..container::Style::default()
            })
    }

    /// Creates the fixed Vim status and command line shown below the editor.
    fn create_vim_status_bar(&self) -> Element<'_, Message> {
        let (left_text, right_text) = self.vim_state.status_line_text();
        let background = self.style.gutter_background;
        let text_color = self.style.text_color;

        container(
            Row::new()
                .push(
                    text(left_text).size(self.font_size).style(move |_| {
                        text::Style { color: Some(text_color) }
                    }),
                )
                .push(Space::new().width(Length::Fill))
                .push(
                    text(right_text).size(self.font_size).style(move |_| {
                        text::Style { color: Some(text_color) }
                    }),
                ),
        )
        .padding([2, 8])
        .width(Length::Fill)
        .height(Length::Fixed(self.line_height.max(20.0)))
        .style(move |_| container::Style {
            background: Some(Background::Color(background)),
            ..container::Style::default()
        })
        .into()
    }

    /// Creates the background layer combining gutter and code backgrounds.
    ///
    /// # Returns
    ///
    /// A row containing the background elements
    fn create_background_layer(&self) -> Row<'_, Message> {
        let gutter_container = self.create_gutter_container();
        let code_background_container = self.create_code_background_container();

        if let Some(gutter) = gutter_container {
            Row::new().push(gutter).push(code_background_container)
        } else {
            Row::new().push(code_background_container)
        }
    }

    /// Calculates the IME cursor rectangle for the current cursor position.
    ///
    /// # Arguments
    ///
    /// * `visual_lines` - The visual line mapping
    ///
    /// # Returns
    ///
    /// A rectangle representing the cursor position for IME
    fn calculate_ime_cursor_rect(
        &self,
        visual_lines: &[wrapping::VisualLine],
    ) -> Rectangle {
        let ime_enabled = self.is_focused() && self.has_canvas_focus;

        if !ime_enabled {
            return Rectangle::new(
                iced::Point::new(0.0, 0.0),
                Size::new(0.0, 0.0),
            );
        }

        if let Some(cursor_visual) = WrappingCalculator::logical_to_visual(
            visual_lines,
            self.cursors.primary_position().0,
            self.cursors.primary_position().1,
        ) {
            let vl = &visual_lines[cursor_visual];
            let line_content = self.buffer.line(vl.logical_line);
            let prefix_len =
                self.cursors.primary_position().1.saturating_sub(vl.start_col);
            let prefix_text: String = line_content
                .chars()
                .skip(vl.start_col)
                .take(prefix_len)
                .collect();
            let cursor_x = self.gutter_width()
                + 5.0
                + crate::canvas_editor::measure_text_width(
                    &prefix_text,
                    self.full_char_width,
                    self.char_width,
                )
                - self.horizontal_scroll_offset;

            // Calculate visual Y position relative to the viewport
            // We subtract viewport_scroll because the content is scrolled up/down
            // but the cursor position sent to IME must be relative to the visible area
            let cursor_y = (cursor_visual as f32 * self.line_height)
                - self.viewport_scroll;

            Rectangle::new(
                iced::Point::new(cursor_x, cursor_y + 2.0),
                Size::new(2.0, self.line_height - 4.0),
            )
        } else {
            Rectangle::new(iced::Point::new(0.0, 0.0), Size::new(0.0, 0.0))
        }
    }

    /// Builds one pinned header row: its line number, then its colored text.
    ///
    /// The row reuses the memoized per-line highlight spans, so pinning a header
    /// costs no extra syntect work. Unlike the canvas text layer, it ignores
    /// `horizontal_scroll_offset`: a pinned header exists to be read, and would
    /// be useless if it scrolled out of view sideways.
    ///
    /// # Arguments
    ///
    /// * `line` - Index of the logical header line to render
    /// * `syntax_set` - The syntax set `syntax` belongs to
    /// * `syntax` - The syntect syntax definition to tokenize with
    /// * `theme` - The syntect theme providing token colors
    fn create_sticky_header_row(
        &self,
        line: usize,
        syntax_set: &syntect::parsing::SyntaxSet,
        syntax: &syntect::parsing::SyntaxReference,
        theme: &syntect::highlighting::Theme,
    ) -> Element<'_, Message> {
        let font = self.font;
        let font_size = self.font_size;
        let line_number_color = self.style.line_number_color;

        let mut row = Row::new();

        if self.line_numbers_enabled {
            row = row.push(
                container(
                    text(format!("{}", line + 1))
                        .size(font_size)
                        .font(font)
                        .style(move |_| text::Style {
                            color: Some(line_number_color),
                        }),
                )
                .width(Length::Fixed(self.line_number_gutter_width()))
                .align_x(alignment::Horizontal::Center),
            );
        }

        // Keep the fold margin empty: a pinned header is not where the user
        // folds, and a chevron there would invite a click that does nothing.
        row = row
            .push(Space::new().width(Length::Fixed(self.fold_margin_width())));
        row =
            row.push(Space::new().width(Length::Fixed(CODE_TEXT_LEFT_PADDING)));

        let spans =
            self.highlighted_line_cached(line, syntax, theme, syntax_set);
        let mut code = Row::new();
        for (color, content) in spans.iter() {
            let color = *color;
            // Match the canvas text layer, so a pinned header shows the same
            // whitespace glyphs as the line it mirrors.
            let content = if self.show_whitespace {
                expand_tabs_visible(content, TAB_WIDTH)
            } else {
                expand_tabs(content, TAB_WIDTH).into_owned()
            };
            code = code.push(
                text(content)
                    .size(font_size)
                    .font(font)
                    .wrapping(text::Wrapping::None)
                    .style(move |_| text::Style { color: Some(color) }),
            );
        }
        row = row.push(code);

        MouseArea::new(
            container(row)
                .width(Length::Fill)
                .height(Length::Fixed(self.line_height))
                .clip(true),
        )
        .interaction(mouse::Interaction::Pointer)
        .on_press(Message::StickyScrollJump(line))
        .into()
    }

    /// Creates the sticky-scroll layer: the headers of the blocks enclosing the
    /// topmost visible line, pinned above the viewport.
    ///
    /// # Arguments
    ///
    /// * `visual_lines` - The visual line mapping, used to resolve the topmost
    ///   visible visual line back to its logical line
    ///
    /// # Returns
    ///
    /// `Some(layer)` when at least one header must be pinned, `None` when the
    /// feature is disabled or the topmost visible line sits at the top level
    fn create_sticky_scroll_layer(
        &self,
        visual_lines: &[wrapping::VisualLine],
    ) -> Option<Element<'_, Message>> {
        if !self.sticky_scroll_enabled || self.line_height <= 0.0 {
            return None;
        }

        // Same first-visible-line formula the canvas layer uses. The topmost
        // entry is a *visual* line, so it must be mapped back to a logical one:
        // wrapping and collapsed folds make the two indices diverge.
        let first_visible =
            (self.viewport_scroll / self.line_height).floor() as usize;
        let top_line = visual_lines.get(first_visible)?.logical_line;

        // `block_regions`, not `foldable_regions`: the blocks a line sits in
        // are a property of the buffer, not of whether the user may collapse
        // them. Sticky scroll keeps working with code folding turned off.
        let regions = self.block_regions();
        let headers = sticky_scroll::sticky_headers(
            &regions,
            top_line,
            sticky_scroll::DEFAULT_MAX_STICKY_LINES,
        );
        if headers.is_empty() {
            return None;
        }

        let (syntax_set, syntax, theme) = self.resolve_syntax();
        let (Some(syntax), Some(theme)) = (syntax, theme) else {
            return None;
        };

        let mut column = Column::new();
        for line in headers {
            column = column.push(
                self.create_sticky_header_row(line, syntax_set, syntax, theme),
            );
        }

        let background = self.style.gutter_background;
        let border_color = self.style.gutter_border;

        // An explicit rule rather than a container shadow: it must sit exactly
        // one pixel under the last header, on top of the code scrolling beneath.
        let column = column.push(
            container(Space::new().width(Length::Fill).height(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fixed(STICKY_SEPARATOR_WIDTH))
                .style(move |_| container::Style {
                    background: Some(Background::Color(border_color)),
                    ..container::Style::default()
                }),
        );

        Some(
            container(column)
                .width(Length::Fill)
                .height(Length::Shrink)
                .style(move |_| container::Style {
                    background: Some(Background::Color(background)),
                    ..container::Style::default()
                })
                .into(),
        )
    }

    /// Creates the IME (Input Method Editor) layer widget.
    ///
    /// # Arguments
    ///
    /// * `cursor_rect` - The rectangle representing the cursor position
    ///
    /// # Returns
    ///
    /// An element containing the IME requester widget
    fn create_ime_layer(&self, cursor_rect: Rectangle) -> Element<'_, Message> {
        let ime_enabled = self.is_focused() && self.has_canvas_focus;

        let preedit =
            self.ime_preedit.as_ref().map(|p| input_method::Preedit {
                content: p.content.clone(),
                selection: p.selection.clone(),
                text_size: None,
            });

        let ime_layer = ImeRequester::new(ime_enabled, cursor_rect, preedit);
        iced::Element::new(ime_layer)
    }

    /// Creates the view element with scrollable wrapper.
    ///
    /// The backgrounds (editor and gutter) are handled by container styles
    /// to ensure proper clipping when the pane is resized.
    ///
    /// Call this from the host application's `view`, mapping the editor's
    /// [`Message`] into the host's own message type.
    ///
    /// # Returns
    ///
    /// An `Element` rendering the editor, its gutter, and any open overlay
    ///
    /// # Example
    ///
    /// ```
    /// use iced::Element;
    /// use iced_code_editor::{CodeEditor, Message};
    ///
    /// /// The host application's own message type.
    /// #[derive(Debug, Clone)]
    /// enum AppMessage {
    ///     Editor(Message),
    /// }
    ///
    /// fn view(editor: &CodeEditor) -> Element<'_, AppMessage> {
    ///     editor.view().map(AppMessage::Editor)
    /// }
    /// ```
    pub fn view(&self) -> Element<'_, Message> {
        // Calculate canvas height and visual lines
        let (visual_lines, canvas_height) = self.calculate_canvas_height();

        // Create scrollable containing the canvas
        let scrollable = self.create_canvas_with_scrollable(canvas_height);

        // Create background layer with gutter and code backgrounds
        let background_row = self.create_background_layer();

        // Build editor stack: backgrounds + scrollable
        let mut editor_stack =
            iced::widget::Stack::new().push(background_row).push(scrollable);

        // Pin the enclosing block headers above the viewport. This goes right
        // above the scrollable so the dialogs pushed below stay on top of it.
        if let Some(sticky_layer) =
            self.create_sticky_scroll_layer(visual_lines.as_ref())
        {
            editor_stack = editor_stack.push(sticky_layer);
        }

        // Add IME layer for input method support.
        // The IME requester needs the cursor rect in viewport coordinates, which
        // depends on the current logical↔visual mapping.
        let cursor_rect = self.calculate_ime_cursor_rect(visual_lines.as_ref());
        let ime_layer = self.create_ime_layer(cursor_rect);
        editor_stack = editor_stack.push(ime_layer);

        // Add search dialog overlay if open
        if self.search_state.is_open {
            let search_dialog =
                search_dialog::view(&self.search_state, &self.translations);

            // Position the dialog in top-right corner with 20px margin
            let positioned_dialog = container(
                Row::new()
                    .push(Space::new().width(Length::Fill))
                    .push(search_dialog),
            )
            .padding(20)
            .width(Length::Fill)
            .height(Length::Shrink);

            editor_stack = editor_stack.push(positioned_dialog);
        }

        // Add the compact go-to-line dialog in the top center.
        if self.goto_line_state.is_open {
            let goto_line_dialog = goto_line_dialog::view(
                &self.goto_line_state,
                self.buffer.line_count(),
            );
            let positioned_dialog = container(
                Row::new()
                    .push(Space::new().width(Length::Fill))
                    .push(goto_line_dialog)
                    .push(Space::new().width(Length::Fill)),
            )
            .padding(20)
            .width(Length::Fill)
            .height(Length::Shrink);

            editor_stack = editor_stack.push(positioned_dialog);
        }

        // Add the command palette in the top center, above every other
        // dialog: opening it closes the others, so it is always innermost.
        if self.command_palette_state.is_open {
            let entries = self.command_palette_entries();
            let palette = command_palette_dialog::view(
                &self.command_palette_state,
                &entries,
                &self.translations,
            );
            let positioned_palette = container(
                Row::new()
                    .push(Space::new().width(Length::Fill))
                    .push(palette)
                    .push(Space::new().width(Length::Fill)),
            )
            .padding(20)
            .width(Length::Fill)
            .height(Length::Shrink);

            editor_stack = editor_stack.push(positioned_palette);
        }

        // Wrap the editor stack in a container with clip
        let editor_container = container(editor_stack)
            .width(Length::Fill)
            .height(Length::Fill)
            .clip(true);

        // The context menu owns its transient open/close state and positions
        // itself at the right-click location. The canvas still receives the
        // right-click event so it can preserve or reposition the selection.
        let action_context = self.action_context();
        let custom_context_menu_entries =
            self.custom_context_menu_entries().to_vec();
        let default_context_menu_enabled = self.default_context_menu_enabled();
        let translations = self.translations;
        let editor_container = ContextMenu::new(editor_container, move || {
            context_menu::view(
                &custom_context_menu_entries,
                default_context_menu_enabled,
                action_context,
                translations,
            )
        });

        // When wrap is disabled, add a horizontal scrollbar below the editor.
        let editor_body: Element<'_, Message> = if self.wrap_enabled {
            editor_container.into()
        } else {
            // Measuring the widest line scans the entire buffer. It is only
            // needed for the horizontal scrollbar, so never do that work while
            // wrapping is enabled (the default), especially after every edit in
            // a large file.
            let max_content_width = self.max_content_width();
            if let Some(h_scrollbar) =
                self.create_horizontal_scrollbar(max_content_width)
            {
                Column::new().push(editor_container).push(h_scrollbar).into()
            } else {
                editor_container.into()
            }
        };

        if self.vim_enabled {
            Column::new()
                .push(editor_body)
                .push(self.create_vim_status_bar())
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            editor_body
        }
    }
}

/// Builds a plain-color scrollable rail: a solid background track with a
/// solid-color scroller, square corners of `radius`, and no border.
///
/// Shared by the canvas editor's own scrollbars ([`view`]) and the LSP
/// overlay panels ([`lsp::process::overlay`]), which derive their colors from
/// a theme palette instead of plain [`Color`]s.
///
/// # Examples
///
/// ```text
/// let rail = scrollable_rail(Color::BLACK, Color::WHITE, 4.0);
/// ```
pub(crate) fn scrollable_rail(
    background: Color,
    scroller: Color,
    radius: f32,
) -> iced::widget::scrollable::Rail {
    iced::widget::scrollable::Rail {
        background: Some(background.into()),
        border: iced::Border {
            radius: radius.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        scroller: iced::widget::scrollable::Scroller {
            background: scroller.into(),
            border: iced::Border {
                radius: radius.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrollable_rail_sets_background_scroller_and_radius() {
        let bg = Color::from_rgb(0.1, 0.2, 0.3);
        let scroller = Color::from_rgb(0.4, 0.5, 0.6);
        let rail = scrollable_rail(bg, scroller, 4.0);

        assert_eq!(rail.background, Some(iced::Background::Color(bg)));
        assert_eq!(rail.scroller.background, iced::Background::Color(scroller));
        assert_eq!(rail.border.radius, iced::border::Radius::from(4.0));
        assert!(rail.border.width.abs() < f32::EPSILON);
        assert_eq!(
            rail.scroller.border.radius,
            iced::border::Radius::from(4.0)
        );
    }

    #[test]
    fn test_sticky_layer_absent_when_disabled() {
        let mut editor =
            CodeEditor::new("fn main() {\n    let x = 1;\n}", "rs");
        editor.set_sticky_scroll_enabled(false);
        let visual_lines = editor.visual_lines_cached(editor.viewport_width);

        assert!(
            editor.create_sticky_scroll_layer(visual_lines.as_ref()).is_none()
        );
    }

    #[test]
    fn test_sticky_layer_absent_without_enclosing_block() {
        // A flat buffer has no fold region, so nothing can be pinned.
        let editor = CodeEditor::new("a\nb\nc", "rs");
        let visual_lines = editor.visual_lines_cached(editor.viewport_width);

        assert!(
            editor.create_sticky_scroll_layer(visual_lines.as_ref()).is_none()
        );
    }

    #[test]
    fn test_sticky_layer_absent_at_top_of_file() {
        // Scrolled to the top, the header is on screen and must not be pinned.
        let editor = CodeEditor::new(
            "fn main() {\n    let x = 1;\n    let y = 2;\n}",
            "rs",
        );
        let visual_lines = editor.visual_lines_cached(editor.viewport_width);

        assert!(
            editor.create_sticky_scroll_layer(visual_lines.as_ref()).is_none()
        );
    }

    #[test]
    fn test_sticky_layer_survives_code_folding_being_disabled() {
        // End-to-end counterpart of
        // `sticky_scroll::tests::test_headroom_survives_code_folding_being_disabled`:
        // the layer is built from `block_regions`, so turning folding off
        // removes the chevrons and leaves the pinned headers alone.
        let mut editor = CodeEditor::new(
            "fn main() {\n    let x = 1;\n    let y = 2;\n}",
            "rs",
        );
        editor.set_folding_enabled(false);
        editor.viewport_scroll = editor.line_height();
        let visual_lines = editor.visual_lines_cached(editor.viewport_width);

        assert!(
            editor.create_sticky_scroll_layer(visual_lines.as_ref()).is_some()
        );
    }

    #[test]
    fn test_sticky_layer_maps_a_wrapped_segment_back_to_its_logical_line() {
        // The subtlest line in the feature: the topmost entry of the viewport
        // is a *visual* line, and soft wrapping makes visual and logical
        // indices diverge. Here the body of the `if` wraps into several
        // segments, so scrolling onto one of its continuations puts the
        // topmost visual index well past any logical line index.
        //
        // The fixture discriminates: reading the visual index as a logical one
        // would land on `}` or past the end of the buffer, where nothing
        // encloses anything and the layer would be `None`.
        let mut editor = CodeEditor::new(
            "fn main() {\n    if a {\n        let x = 1; let y = 2; let z = 3; let w = 4;\n    }\n}",
            "rs",
        )
        .with_wrap_column(Some(20));

        let visual_lines = editor.visual_lines_cached(editor.viewport_width);
        let continuation = visual_lines
            .iter()
            .position(|line| line.logical_line == 2 && line.segment_index == 1)
            .unwrap_or_default();
        assert!(
            continuation > 2,
            "the fixture must wrap far enough for the two indices to diverge"
        );

        editor.viewport_scroll = continuation as f32 * editor.line_height();
        let visual_lines = editor.visual_lines_cached(editor.viewport_width);

        assert!(
            editor.create_sticky_scroll_layer(visual_lines.as_ref()).is_some(),
            "a continuation segment is still inside both enclosing blocks"
        );
        // Both headers, so the layer really resolved logical line 2 and not
        // whatever line the raw visual index points at.
        assert_eq!(editor.sticky_headroom(2), 2);
    }

    #[test]
    fn test_sticky_layer_present_when_scrolled_into_block() {
        let mut editor = CodeEditor::new(
            "fn main() {\n    let x = 1;\n    let y = 2;\n}",
            "rs",
        );
        // Scroll so the second line is the topmost visible one, putting the
        // viewport inside the `fn` block.
        editor.viewport_scroll = editor.line_height();
        let visual_lines = editor.visual_lines_cached(editor.viewport_width);

        assert!(
            editor.create_sticky_scroll_layer(visual_lines.as_ref()).is_some()
        );
    }

    #[test]
    fn test_canvas_scrollbar_style_uses_transparent_container() {
        let bg = Color::from_rgb(0.1, 0.2, 0.3);
        let scroller = Color::from_rgb(0.4, 0.5, 0.6);
        let style = canvas_scrollbar_style(bg, scroller);

        assert_eq!(
            style.container.background,
            Some(Background::Color(Color::TRANSPARENT))
        );
        let expected_rail = scrollable_rail(bg, scroller, 4.0);
        assert_eq!(style.vertical_rail.background, expected_rail.background);
        assert_eq!(style.horizontal_rail.background, expected_rail.background);
    }
}
