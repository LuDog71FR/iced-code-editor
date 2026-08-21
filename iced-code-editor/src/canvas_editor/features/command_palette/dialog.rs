//! Command palette UI: filter input plus the filtered command list.

use iced::mouse;
use iced::widget::canvas::{self, Canvas};
use iced::widget::{
    Space, Stack, button, column, container, row, scrollable, text, text_input,
};
use iced::{
    Background, Border, Color, Element, Event, Length, Rectangle, Renderer,
    Shadow, Theme, Vector, keyboard,
};

use super::{CommandPaletteState, PaletteEntry};
use crate::canvas_editor::Message;
use crate::canvas_editor::render::view::scrollable_rail;
use crate::i18n::Translations;

/// Height of a single command row, in pixels.
///
/// Fixed rather than content-driven because the geometry that keeps the
/// highlighted row visible is computed from it (see [`rows_to_pixels`] and
/// `CodeEditor::scroll_command_palette_to_selection`).
const ROW_HEIGHT: f32 = 26.0;

/// Vertical gap inserted between two consecutive rows.
const ROW_SPACING: f32 = 1.0;

/// Distance from the top of one row to the top of the next.
///
/// This — not [`ROW_HEIGHT`] — is the unit every scroll computation works in:
/// the rows are laid out in a `column` with [`ROW_SPACING`] between them, so a
/// list scrolled by `n * ROW_HEIGHT` would drift by one pixel per row.
const ROW_PITCH: f32 = ROW_HEIGHT + ROW_SPACING;

/// Number of command rows shown before the list starts scrolling.
pub(crate) const MAX_VISIBLE_ROWS: usize = 10;

/// Converts a row count into the pixel distance it spans.
///
/// Used both for the list's fixed height and for the scroll offset, so the two
/// cannot disagree about how tall a row is.
///
/// # Arguments
///
/// * `rows` - Number of rows to measure
///
/// # Returns
///
/// The distance from the top of the first row to the top of row `rows`,
/// saturating on a row count no list could ever reach
pub(crate) fn rows_to_pixels(rows: usize) -> f32 {
    ROW_PITCH * f32::from(u16::try_from(rows).unwrap_or(u16::MAX))
}

/// Width of the palette, in pixels.
const PALETTE_WIDTH: f32 = 560.0;

/// Border radius shared by the palette frame and its scrollbar rail.
const BORDER_RADIUS: f32 = 6.0;

/// Transparent top layer that handles the keys the focused text input would
/// otherwise swallow: Escape (which would merely unfocus it) and the arrow
/// keys (which would move the caret inside the query instead of moving the
/// highlight through the results).
struct KeyListener;

impl canvas::Program<Message> for KeyListener {
    type State = ();

    fn update(
        &self,
        _state: &mut Self::State,
        event: &Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event
        else {
            return None;
        };

        let message = match key {
            keyboard::Key::Named(keyboard::key::Named::Escape) => {
                Message::CloseCommandPalette
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                Message::CommandPaletteNavigate(true)
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                Message::CommandPaletteNavigate(false)
            }
            _ => return None,
        };

        Some(canvas::Action::publish(message).and_capture())
    }

    fn draw(
        &self,
        _state: &Self::State,
        _renderer: &Renderer,
        _theme: &Theme,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        Vec::new()
    }
}

/// Builds the command palette shown over the editor.
///
/// # Arguments
///
/// * `state` - Current palette state
/// * `entries` - The commands matching the current query, in display order
/// * `translations` - Translations for the palette's own UI text
pub(crate) fn view<'a>(
    state: &CommandPaletteState,
    entries: &[PaletteEntry],
    translations: &Translations,
) -> Element<'a, Message> {
    if !state.is_open {
        return Space::new().into();
    }

    let query_input =
        text_input(&translations.command_palette_placeholder(), &state.query)
            .id(state.input_id.clone())
            .on_input(Message::CommandPaletteChanged)
            .on_submit(Message::SubmitCommandPalette)
            .padding(8)
            .size(14)
            .width(Length::Fill);

    let results: Element<'a, Message> = if entries.is_empty() {
        container(text(translations.command_palette_no_results()).size(13))
            .padding([6, 10])
            .width(Length::Fill)
            .into()
    } else {
        let rows = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                command_row(entry, index, index == state.selected)
            })
            .collect::<Vec<_>>();

        // The last row of the window is not followed by a gap, so the window
        // is one spacing shorter than the pitch it is made of. Getting this
        // exactly right is what lets the scroll offset land on a row boundary.
        let visible_rows = entries.len().min(MAX_VISIBLE_ROWS);
        let list_height = rows_to_pixels(visible_rows) - ROW_SPACING;

        scrollable(column(rows).spacing(ROW_SPACING))
            .id(state.scrollable_id.clone())
            .height(Length::Fixed(list_height))
            .width(Length::Fill)
            .style(|theme: &Theme, _status| {
                let palette = theme.extended_palette();
                scrollable::Style {
                    container: container::Style::default(),
                    vertical_rail: scrollable_rail(
                        palette.background.weak.color,
                        palette.background.strong.color,
                        BORDER_RADIUS,
                    ),
                    horizontal_rail: scrollable_rail(
                        palette.background.weak.color,
                        palette.background.strong.color,
                        BORDER_RADIUS,
                    ),
                    gap: None,
                    auto_scroll: scrollable::AutoScroll {
                        background: Color::TRANSPARENT.into(),
                        border: Border::default(),
                        shadow: Shadow::default(),
                        icon: Color::TRANSPARENT,
                    },
                }
            })
            .into()
    };

    let dialog =
        container(column![query_input, results].spacing(6).width(Length::Fill))
            .padding(8)
            .width(Length::Fixed(PALETTE_WIDTH))
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(Background::Color(
                        palette.background.weak.color,
                    )),
                    text_color: Some(palette.background.weak.text),
                    border: Border {
                        color: palette.background.strong.color,
                        width: 1.0,
                        radius: BORDER_RADIUS.into(),
                    },
                    shadow: Shadow {
                        color: Color::BLACK.scale_alpha(0.35),
                        offset: Vector::new(0.0, 4.0),
                        blur_radius: 14.0,
                    },
                    ..container::Style::default()
                }
            });

    Stack::new()
        .push(dialog)
        .push(Canvas::new(KeyListener).width(Length::Fill).height(Length::Fill))
        .into()
}

/// Builds one command row: the label on the left, its shortcut hint right.
fn command_row<'a>(
    entry: &PaletteEntry,
    index: usize,
    is_selected: bool,
) -> Element<'a, Message> {
    let content = row![
        text(entry.label.clone()).size(13),
        Space::new().width(Length::Fill),
        text(entry.shortcut.clone()).size(12),
    ]
    .align_y(iced::Alignment::Center);

    button(content)
        .width(Length::Fill)
        .height(Length::Fixed(ROW_HEIGHT))
        .padding([4, 8])
        .on_press(Message::CommandPaletteSelected(index))
        .style(move |theme: &Theme, status| {
            let palette = theme.extended_palette();
            let hovered = matches!(
                status,
                button::Status::Hovered | button::Status::Pressed
            );
            let background = if is_selected {
                Some(Background::Color(palette.primary.weak.color))
            } else if hovered {
                Some(Background::Color(palette.background.strong.color))
            } else {
                None
            };
            let text_color = if is_selected {
                palette.primary.weak.text
            } else {
                palette.background.weak.text
            };

            button::Style {
                background,
                text_color,
                border: Border { radius: 4.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        })
        .into()
}
