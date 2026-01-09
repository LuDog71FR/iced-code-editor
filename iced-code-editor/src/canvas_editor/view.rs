//! Iced UI view and rendering logic.

use iced::widget::canvas::Canvas;
use iced::widget::{Row, Scrollable, Space, container, scrollable};
use iced::{Background, Border, Color, Element, Length, Shadow};

use super::{
    CodeEditor, GUTTER_WIDTH, LINE_HEIGHT, Message,
    wrapping::WrappingCalculator,
};

impl CodeEditor {
    /// Creates the view element with scrollable wrapper.
    ///
    /// The backgrounds (editor and gutter) are handled by container styles
    /// to ensure proper clipping when the pane is resized.
    pub fn view(&self) -> Element<'_, Message> {
        // Calculate total content height based on actual lines
        // When wrapping is enabled, use visual line count
        let wrapping_calc =
            WrappingCalculator::new(self.wrap_enabled, self.wrap_column);

        // Use viewport width for calculating visual lines
        let visual_lines = wrapping_calc
            .calculate_visual_lines(&self.buffer, self.viewport_width);

        let total_visual_lines = visual_lines.len();
        let content_height = total_visual_lines as f32 * LINE_HEIGHT;

        // Use max of content height and viewport height to ensure the canvas
        // always covers the visible area (prevents visual artifacts when
        // content is shorter than viewport after reset/file change)
        let canvas_height = content_height.max(self.viewport_height);

        // Create canvas with height that covers at least the viewport
        let canvas = Canvas::new(self)
            .width(Length::Fill)
            .height(Length::Fixed(canvas_height));

        // Capture style colors for closures
        let scrollbar_bg = self.style.scrollbar_background;
        let scroller_color = self.style.scroller_color;
        let background_color = self.style.background;
        let gutter_background = self.style.gutter_background;

        // Wrap in scrollable for automatic scrollbar display with custom style
        // Use Length::Fill to respect parent container constraints and enable proper clipping
        // Background is TRANSPARENT here because it's handled by the Stack layer below
        let scrollable = Scrollable::new(canvas)
            .id(self.scrollable_id.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .on_scroll(Message::Scrolled)
            .style(move |_theme, _status| scrollable::Style {
                container: container::Style {
                    background: Some(Background::Color(Color::TRANSPARENT)),
                    ..container::Style::default()
                },
                vertical_rail: scrollable::Rail {
                    background: Some(scrollbar_bg.into()),
                    border: Border {
                        radius: 4.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    scroller: scrollable::Scroller {
                        background: scroller_color.into(),
                        border: Border {
                            radius: 4.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                    },
                },
                horizontal_rail: scrollable::Rail {
                    background: Some(scrollbar_bg.into()),
                    border: Border {
                        radius: 4.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    scroller: scrollable::Scroller {
                        background: scroller_color.into(),
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
            });

        // Gutter background container (fixed width, clipped by parent)
        let gutter_container =
            container(Space::new().width(Length::Fill).height(Length::Fill))
                .width(Length::Fixed(GUTTER_WIDTH))
                .height(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(gutter_background)),
                    ..container::Style::default()
                });

        // Code background container (fills remaining width)
        let code_background_container =
            container(Space::new().width(Length::Fill).height(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(background_color)),
                    ..container::Style::default()
                });

        // Main layout: use a Stack to layer the backgrounds behind the scrollable
        // The scrollable has a transparent background so the colors show through
        let editor_content = iced::widget::Stack::new()
            .push(
                // Background layer (bottom): gutter + code backgrounds
                Row::new()
                    .push(gutter_container)
                    .push(code_background_container),
            )
            .push(
                // Scrollable layer (top) - transparent, overlays the backgrounds
                scrollable,
            );

        // Wrap in a container with clip to ensure proper bounds
        container(editor_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .clip(true)
            .into()
    }
}
