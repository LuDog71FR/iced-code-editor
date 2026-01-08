//! Iced UI view and rendering logic.

use iced::widget::canvas::Canvas;
use iced::widget::{Scrollable, container, scrollable};
use iced::{Border, Color, Element, Length, Shadow};

use super::{CodeEditor, LINE_HEIGHT, Message};

impl CodeEditor {
    /// Creates the view element with scrollable wrapper.
    pub fn view(&self) -> Element<'_, Message> {
        // Calculate total content height based on actual lines only
        let total_lines = self.buffer.line_count();
        let content_height = total_lines as f32 * LINE_HEIGHT;

        // Create canvas with height based on content only
        // The scrollable wrapper will handle the viewport constraints
        let canvas = Canvas::new(self)
            .width(Length::Fill)
            .height(Length::Fixed(content_height));

        // Capture style colors for the scrollbar style closure
        let scrollbar_bg = self.style.scrollbar_background;
        let scroller_color = self.style.scroller_color;

        // Wrap in scrollable for automatic scrollbar display with custom style
        // Use Length::Shrink to respect parent container constraints
        Scrollable::new(canvas)
            .id(self.scrollable_id.clone())
            .width(Length::Fill)
            .height(Length::Shrink)
            .on_scroll(Message::Scrolled)
            .style(move |_theme, _status| scrollable::Style {
                container: container::Style::default(),
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
            })
            .into()
    }
}
