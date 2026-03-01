//! LSP (Language Server Protocol) UI components.
//!
//! This module provides UI elements for displaying LSP features such as
//! hover tooltips and auto-completion menus as overlays on the code editor.

use crate::app::{DemoApp, Message};
use crate::types::EditorId;
use iced::widget::{
    Id, Space, button, column, container, mouse_area, row, scrollable, stack,
    text, text_editor,
};
use iced::{Background, Border, Color, Element, Length, Point, Shadow, Theme};
use iced_code_editor::CodeEditor;

/// Returns an empty LSP panel for native platforms.
/// Currently not implemented - returns a minimal placeholder container.
#[cfg(not(target_arch = "wasm32"))]
pub fn view_lsp_panel() -> Element<'static, Message> {
    container(Space::new()).width(Length::Shrink).height(Length::Shrink).into()
}

/// Returns an empty LSP panel for WebAssembly platforms.
/// LSP features are not available in the web version.
#[cfg(target_arch = "wasm32")]
pub fn view_lsp_panel() -> Element<'static, Message> {
    column![].into()
}

/// Measures the maximum width needed to display hover text.
/// Calculates the width of the longest line in the hover content.
#[cfg(not(target_arch = "wasm32"))]
fn measure_hover_width(editor: &CodeEditor, text: &str) -> f32 {
    text.lines().map(|line| editor.measure_text_width(line)).fold(0.0, f32::max)
}

/// Creates overlay UI elements for LSP features (hover tooltips and completion menus).
///
/// This function renders two types of overlays:
/// 1. Hover tooltips - Display type information and documentation when hovering over code
/// 2. Completion menus - Show auto-completion suggestions as users type
///
/// The overlays are positioned intelligently to avoid clipping:
/// - Hover tooltips appear above or below the cursor based on available space
/// - Completion menus appear in the top-left corner of the editor
///
/// Returns an empty container if no overlays are currently visible.
#[cfg(not(target_arch = "wasm32"))]
pub fn view_lsp_overlay(
    app: &DemoApp,
    editor_id: EditorId,
) -> Element<'_, Message> {
    // Early return if this editor doesn't have the LSP overlay focus
    if app.lsp_overlay_editor != Some(editor_id) {
        return container(
            Space::new().width(Length::Shrink).height(Length::Shrink),
        )
        .into();
    }

    let mut has_overlay = false;

    // Build the hover tooltip layer
    let hover_layer: Element<'_, Message> = if app.lsp_hover_visible {
        // Only show hover if there's non-empty content
        if let Some(hover) =
            app.lsp_last_hover.as_ref().filter(|text| !text.trim().is_empty())
        {
            // Calculate hover tooltip dimensions
            let line_height = app.current_line_height;
            let line_count = hover.lines().count().max(1);
            let max_lines = 10usize; // Limit to 10 lines to prevent oversized tooltips
            let visible_lines = line_count.min(max_lines);
            let hover_padding = 8.0;

            // Calculate scroll height including padding and scrollbar space
            let scroll_height = line_height * visible_lines as f32
                + (line_height * 0.75).max(10.0)
                + hover_padding * 2.0;

            // Get the editor and its viewport width based on which editor is active
            let (editor, viewport_width) = match editor_id {
                EditorId::Left => {
                    (&app.editor_left, app.editor_left.viewport_width())
                }
                EditorId::Right => {
                    (&app.editor_right, app.editor_right.viewport_width())
                }
            };

            // Calculate content width, respecting viewport boundaries
            let max_line_width = measure_hover_width(editor, hover);
            let max_width = (viewport_width - 24.0).max(0.0);
            let content_max_width = if max_width > hover_padding * 2.0 {
                max_width - hover_padding * 2.0
            } else {
                max_width
            };
            let content_width = if content_max_width > 0.0 {
                max_line_width.min(content_max_width)
            } else {
                max_line_width
            };
            let hover_width = content_width + hover_padding * 2.0;

            // Build the scrollable hover content with a text editor
            // Using text_editor allows users to copy text from the hover tooltip
            let hover_content = scrollable(
                container(
                    text_editor(&app.lsp_hover_content)
                        .id(Id::new("lsp_hover_text_editor"))
                        .on_action(Message::LspHoverContentAction)
                        .padding(hover_padding)
                        .width(hover_width)
                        // Style the text editor to blend with the hover container
                        .style(|theme: &Theme, _status| {
                            let palette = theme.extended_palette();
                            text_editor::Style {
                                background: Background::Color(
                                    Color::TRANSPARENT,
                                ),
                                border: Border {
                                    radius: 0.0.into(),
                                    width: 0.0,
                                    color: Color::TRANSPARENT,
                                },
                                placeholder: palette.background.weak.text,
                                value: palette.background.weak.text,
                                selection: palette.primary.weak.color,
                            }
                        }),
                )
                .width(Length::Fixed(hover_width)),
            )
            .height(Length::Fixed(scroll_height))
            .width(Length::Fixed(hover_width))
            // Style the scrollbar to match the theme
            .style(|theme: &Theme, _status| {
                let palette = theme.extended_palette();
                scrollable::Style {
                    container: container::Style {
                        background: Some(Background::Color(Color::TRANSPARENT)),
                        ..container::Style::default()
                    },
                    vertical_rail: scrollable::Rail {
                        background: Some(palette.background.weak.color.into()),
                        border: Border {
                            radius: 4.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        scroller: scrollable::Scroller {
                            background: palette.primary.weak.color.into(),
                            border: Border {
                                radius: 4.0.into(),
                                width: 0.0,
                                color: Color::TRANSPARENT,
                            },
                        },
                    },
                    horizontal_rail: scrollable::Rail {
                        background: Some(palette.background.weak.color.into()),
                        border: Border {
                            radius: 4.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        scroller: scrollable::Scroller {
                            background: palette.primary.weak.color.into(),
                            border: Border {
                                radius: 4.0.into(),
                                width: 0.0,
                                color: Color::TRANSPARENT,
                            },
                        },
                    },
                    gap: None,
                    auto_scroll: scrollable::AutoScroll {
                        background: Color::TRANSPARENT.into(),
                        border: Border::default(),
                        shadow: Shadow::default(),
                        icon: Color::TRANSPARENT,
                    },
                }
            });

            // Wrap the hover content in a styled container
            // Wrap the hover content in a styled container
            let hover_box = container(column![hover_content])
                .width(Length::Shrink)
                // Apply theme-aware styling with border and background
                .style(|theme: &Theme| {
                    let palette = theme.extended_palette();
                    container::Style {
                        background: Some(iced::Background::Color(
                            palette.background.weak.color,
                        )),
                        border: iced::Border {
                            color: palette.primary.weak.color,
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        ..Default::default()
                    }
                });

            // Add mouse interaction to keep the hover visible when mouse enters it
            let hover_box: Element<'_, Message> = mouse_area(hover_box)
                .on_enter(Message::LspHoverEntered)
                .on_move(|_| Message::LspHoverEntered)
                .on_exit(Message::LspHoverExited)
                .into();

            // Calculate hover tooltip position
            let hover_pos =
                app.lsp_hover_position.unwrap_or(Point::new(4.0, 4.0));

            // Adjust position for viewport scrolling
            let viewport_scroll = match editor_id {
                EditorId::Left => app.editor_left.viewport_scroll(),
                EditorId::Right => app.editor_right.viewport_scroll(),
            };
            let hover_pos = Point::new(
                hover_pos.x,
                (hover_pos.y - viewport_scroll).max(0.0),
            );

            // Get viewport dimensions for boundary checking
            let viewport_height = match editor_id {
                EditorId::Left => app.editor_left.viewport_height(),
                EditorId::Right => app.editor_right.viewport_height(),
            };

            // Determine optimal position to avoid clipping
            let hover_total_height = scroll_height;
            let gap = 1.0; // Small gap between cursor and tooltip

            // Check if tooltip should appear above the cursor
            let show_above = hover_pos.y >= hover_total_height + gap;

            // Calculate horizontal position
            let gap_x = (editor.char_width() * 0.5).max(2.0);
            let right_x = hover_pos.x + gap_x;
            let left_x = hover_pos.x - hover_width - gap_x;
            let max_x = (viewport_width - hover_width - 4.0).max(0.0);

            // Prefer right side, fall back to left if no space, otherwise clamp
            let offset_x = if right_x <= max_x {
                right_x
            } else if left_x >= 0.0 {
                left_x
            } else {
                right_x.clamp(0.0, max_x)
            };

            // Calculate vertical position (above or below cursor)
            let offset_y = if show_above {
                (hover_pos.y - hover_total_height - gap).max(0.0)
            } else {
                (hover_pos.y + line_height + gap).max(0.0).min(viewport_height)
            };

            has_overlay = true;

            // Position the hover tooltip using offset calculations
            container(
                column![
                    Space::new().height(Length::Fixed(offset_y)),
                    row![
                        Space::new().width(Length::Fixed(offset_x)),
                        hover_box
                    ]
                ]
                .spacing(0)
                .width(Length::Fill)
                .height(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            container(Space::new().width(Length::Shrink).height(Length::Shrink))
                .into()
        }
    } else {
        container(Space::new().width(Length::Shrink).height(Length::Shrink))
            .into()
    };

    // Build the auto-completion menu layer
    let completion_layer: Element<'_, Message> = if app.lsp_completion_visible
        && !app.lsp_last_completion.is_empty()
    {
        let mut completion_items: Vec<Element<'_, Message>> = Vec::new();

        // Create header with title and close button
        let header = row![
            text("Completion").size(12),
            Space::new().width(Length::Fill),
            button(text("×").size(12))
                .on_press(Message::LspCompletionClosed)
                .padding(2)
        ]
        .align_y(iced::Center);
        completion_items.push(header.into());

        // Render each completion item
        for (index, item) in app.lsp_last_completion.iter().enumerate() {
            // Highlight the currently selected item with a marker
            let label = if index == app.lsp_completion_selected {
                format!("› {}", item)
            } else {
                item.clone()
            };
            completion_items.push(
                button(text(label).size(12))
                    .on_press(Message::LspCompletionSelected(index))
                    .padding(4)
                    .into(),
            );
        }

        // Wrap completion items in a styled container
        // Wrap completion items in a styled container
        let completion_box = container(column(completion_items).spacing(2))
            .padding(8)
            // Apply theme-aware styling
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(iced::Background::Color(
                        palette.background.weak.color,
                    )),
                    border: iced::Border {
                        color: palette.primary.weak.color,
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                }
            });

        has_overlay = true;

        // Position the completion menu in the top-left corner with padding
        container(
            column![
                Space::new().height(Length::Fixed(12.0)),
                row![Space::new().width(Length::Fixed(12.0)), completion_box]
            ]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
        container(Space::new().width(Length::Shrink).height(Length::Shrink))
            .into()
    };

    // Return empty container if no overlays are visible
    if !has_overlay {
        return container(
            Space::new().width(Length::Shrink).height(Length::Shrink),
        )
        .into();
    }

    // Create a base layer for the stack
    let base = container(Space::new().width(Length::Fill).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill);

    // Stack all overlay layers: base -> completion -> hover
    // The order ensures hover appears on top of completion if both are visible
    stack![base, completion_layer, hover_layer].into()
}

/// Returns an empty overlay for WebAssembly platforms.
/// LSP features are not available in the web version.
#[cfg(target_arch = "wasm32")]
pub fn view_lsp_overlay(
    _app: &DemoApp,
    _editor_id: EditorId,
) -> Element<'_, Message> {
    container(Space::new().width(Length::Shrink).height(Length::Shrink)).into()
}
