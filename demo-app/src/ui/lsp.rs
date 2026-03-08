//! LSP (Language Server Protocol) UI components.
//!
//! This module provides UI elements for displaying LSP features such as
//! hover tooltips and auto-completion menus as overlays on the code editor.

use crate::app::{DemoApp, Message};
use crate::types::EditorId;
#[cfg(not(target_arch = "wasm32"))]
use iced::widget::{
    Id, Space, button, column, container, markdown, mouse_area, row,
    scrollable, stack, text,
};
#[cfg(target_arch = "wasm32")]
use iced::widget::{Space, column, container};
#[cfg(not(target_arch = "wasm32"))]
use iced::{Background, Border, Color, Element, Length, Point, Shadow, Theme};
#[cfg(target_arch = "wasm32")]
use iced::{Element, Length};
#[cfg(not(target_arch = "wasm32"))]
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
            let (editor, viewport_width) = if let Some(tab) =
                app.tabs.iter().find(|t| t.id == editor_id)
            {
                (&tab.editor, tab.editor.viewport_width())
            } else {
                return container(
                    Space::new().width(Length::Shrink).height(Length::Shrink),
                )
                .into();
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

            // Parse markdown content
            let palette = app.current_theme.palette();
            let markdown_settings = markdown::Settings::with_text_size(
                app.current_font_size,
                markdown::Style::from_palette(palette),
            );

            // Build the scrollable hover content with markdown view
            let hover_content = scrollable(
                container(
                    markdown::view(&app.lsp_hover_items, markdown_settings)
                        .map(|_| Message::LspHoverEntered),
                )
                .width(Length::Fixed(hover_width))
                .padding(hover_padding),
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
            let viewport_scroll = editor.viewport_scroll();
            let hover_pos = Point::new(
                hover_pos.x,
                (hover_pos.y - viewport_scroll).max(0.0),
            );

            // Get viewport dimensions for boundary checking
            let viewport_height = editor.viewport_height();

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
        && !app.lsp_completion_suppressed
    {
        // Limit the number of visible items to prevent oversized menus
        let max_visible_items = 8;
        let visible_count =
            app.lsp_last_completion.len().min(max_visible_items);
        let item_height = 20.0; // Approximate height per item
        let header_height = 24.0;
        let padding = 4.0;
        let menu_height = header_height
            + (visible_count as f32 * item_height)
            + (padding * 2.0);

        // Get cursor position from completion_position or use default
        let cursor_pos =
            app.lsp_completion_position.unwrap_or(Point::new(4.0, 4.0));
        let line_height = app.current_line_height;

        // Get editor viewport info
        let (viewport_width, viewport_height, viewport_scroll) =
            if let Some(tab) = app.tabs.iter().find(|t| t.id == editor_id) {
                (
                    tab.editor.viewport_width(),
                    tab.editor.viewport_height(),
                    tab.editor.viewport_scroll(),
                )
            } else {
                return container(
                    Space::new().width(Length::Shrink).height(Length::Shrink),
                )
                .into();
            };

        // Calculate menu width (limited)
        let menu_width = 250.0_f32.min(viewport_width - 8.0);

        // Adjust position for viewport scrolling
        let adjusted_y = (cursor_pos.y - viewport_scroll).max(0.0);

        // Determine if menu should appear above or below cursor
        let space_below = viewport_height - adjusted_y - line_height;
        let show_above =
            space_below < menu_height + 4.0 && adjusted_y >= menu_height + 4.0;

        // Calculate position
        let offset_x =
            cursor_pos.x.min(viewport_width - menu_width - 4.0).max(4.0);
        let offset_y = if show_above {
            (adjusted_y - menu_height - 4.0).max(0.0)
        } else {
            adjusted_y + line_height + 4.0
        };

        let mut completion_items: Vec<Element<'_, Message>> = Vec::new();

        // Render each completion item as a clickable button
        for (index, item) in app.lsp_last_completion.iter().enumerate() {
            let is_selected = index == app.lsp_completion_selected;

            completion_items.push(
                button(text(item.clone()).size(12).line_height(
                    iced::widget::text::LineHeight::Relative(1.5),
                ))
                .padding([2, 8])
                .width(Length::Fill)
                .on_press(Message::LspCompletionSelected(index))
                .style(move |theme: &Theme, _status| {
                    let palette = theme.extended_palette();
                    if is_selected {
                        button::Style {
                            background: Some(iced::Background::Color(
                                palette.primary.weak.color,
                            )),
                            text_color: Color::WHITE,
                            ..Default::default()
                        }
                    } else {
                        button::Style {
                            background: Some(iced::Background::Color(
                                palette.background.weak.color,
                            )),
                            text_color: Color::WHITE,
                            ..Default::default()
                        }
                    }
                })
                .into(),
            );
        }

        // Wrap completion items in a styled, scrollable container
        let completion_box = scrollable(column(completion_items).spacing(0))
            .height(Length::Fixed(menu_height))
            .width(Length::Fixed(menu_width))
            .id(Id::new("completion_scrollable"))
            .style(|theme: &Theme, _status| {
                let palette = theme.extended_palette();
                scrollable::Style {
                    container: container::Style {
                        background: Some(iced::Background::Color(
                            palette.background.weak.color,
                        )),
                        border: iced::Border {
                            color: palette.primary.weak.color,
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    },
                    vertical_rail: scrollable::Rail {
                        background: Some(palette.background.weak.color.into()),
                        border: iced::Border {
                            radius: 4.0.into(),
                            width: 0.0,
                            color: iced::Color::TRANSPARENT,
                        },
                        scroller: scrollable::Scroller {
                            background: palette.primary.weak.color.into(),
                            border: iced::Border {
                                radius: 4.0.into(),
                                width: 0.0,
                                color: iced::Color::TRANSPARENT,
                            },
                        },
                    },
                    horizontal_rail: scrollable::Rail {
                        background: Some(palette.background.weak.color.into()),
                        border: iced::Border {
                            radius: 4.0.into(),
                            width: 0.0,
                            color: iced::Color::TRANSPARENT,
                        },
                        scroller: scrollable::Scroller {
                            background: palette.primary.weak.color.into(),
                            border: iced::Border {
                                radius: 4.0.into(),
                                width: 0.0,
                                color: iced::Color::TRANSPARENT,
                            },
                        },
                    },
                    gap: None,
                    auto_scroll: scrollable::AutoScroll {
                        background: iced::Color::TRANSPARENT.into(),
                        border: iced::Border::default(),
                        shadow: iced::Shadow::default(),
                        icon: iced::Color::TRANSPARENT,
                    },
                }
            });

        has_overlay = true;

        // Create an invisible overlay to capture clicks outside the completion box
        let click_outside =
            button(Space::new().width(Length::Fill).height(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill)
                .on_press(Message::LspCompletionClosed)
                .style(|_theme: &Theme, _status| button::Style {
                    background: Some(Background::Color(Color::TRANSPARENT)),
                    ..Default::default()
                });

        // Position the completion menu at cursor position
        let completion_content = container(
            column![
                Space::new().height(Length::Fixed(offset_y)),
                row![
                    Space::new().width(Length::Fixed(offset_x)),
                    completion_box
                ]
            ]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill);

        // Stack click-outside handler below completion box so clicks on completion still work
        stack![click_outside, completion_content].into()
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
