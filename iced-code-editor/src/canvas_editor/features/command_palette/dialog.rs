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

/// Thickness of the line drawn under the recently-used block.
const SEPARATOR_HEIGHT: f32 = 1.0;

/// Distance the separator adds between the recent block and the row below it:
/// its own thickness plus the extra [`ROW_SPACING`] the column inserts around
/// it. Every scroll computation has to account for it, or the list would drift
/// by two pixels as soon as a command has been run.
const SEPARATOR_PITCH: f32 = SEPARATOR_HEIGHT + ROW_SPACING;

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

/// Returns the number of rows the separator is drawn under, or `0` when the
/// list has no separator.
///
/// The recently-used commands are the leading rows of `entries`, flagged by
/// `promote_recent`. A list that is *only* recent commands gets no separator:
/// there is nothing under it to separate them from.
///
/// # Arguments
///
/// * `entries` - The rows about to be displayed, in display order
///
/// # Returns
///
/// The index of the first non-recent row, or `0` when no line should be drawn
pub(crate) fn separator_after(entries: &[PaletteEntry]) -> usize {
    let recent = entries.iter().take_while(|entry| entry.is_recent).count();
    if recent == entries.len() { 0 } else { recent }
}

/// Returns the vertical offset of the top of row `row`.
///
/// This is [`rows_to_pixels`] corrected for the separator: every row below it
/// sits [`SEPARATOR_PITCH`] lower than its index alone would suggest.
///
/// # Arguments
///
/// * `row` - Index of the row to locate
/// * `separator_after` - Result of [`separator_after`] for the same list
///
/// # Returns
///
/// The distance from the top of the list to the top of `row`
pub(crate) fn row_top(row: usize, separator_after: usize) -> f32 {
    let base = rows_to_pixels(row);
    if separator_after > 0 && row >= separator_after {
        base + SEPARATOR_PITCH
    } else {
        base
    }
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
        let separator_row = separator_after(entries);
        let mut rows: Vec<Element<'a, Message>> =
            Vec::with_capacity(entries.len() + 1);
        for (index, entry) in entries.iter().enumerate() {
            if separator_row > 0 && index == separator_row {
                rows.push(recent_separator());
            }
            rows.push(command_row(
                entry,
                index,
                index == state.selected,
                translations,
            ));
        }

        // The last row of the window is not followed by a gap, so the window
        // is one spacing shorter than the pitch it is made of. Getting this
        // exactly right is what lets the scroll offset land on a row boundary.
        // The separator only counts once it is strictly inside the window —
        // a window ending exactly on it stops above the line.
        let visible_rows = entries.len().min(MAX_VISIBLE_ROWS);
        let separator_extra =
            if separator_row > 0 && visible_rows > separator_row {
                SEPARATOR_PITCH
            } else {
                0.0
            };
        let list_height =
            rows_to_pixels(visible_rows) - ROW_SPACING + separator_extra;

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

/// Builds the line closing the recently-used block.
fn recent_separator<'a>() -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(SEPARATOR_HEIGHT))
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(Background::Color(
                    palette.background.strong.color,
                )),
                ..container::Style::default()
            }
        })
        .into()
}

/// Builds one command row: the label on the left, then the toggle badge and
/// the shortcut hint on the right.
fn command_row<'a>(
    entry: &PaletteEntry,
    index: usize,
    is_selected: bool,
    translations: &Translations,
) -> Element<'a, Message> {
    // A toggle shows what it is switching *from*, so the user knows what
    // running it does without having to run it first.
    let status: Element<'a, Message> = match entry.status {
        Some(true) => {
            text(translations.command_palette_status_on()).size(12).into()
        }
        Some(false) => {
            text(translations.command_palette_status_off()).size(12).into()
        }
        None => Space::new().into(),
    };

    let content = row![
        text(entry.label.clone()).size(13),
        Space::new().width(Length::Fill),
        status,
        text(entry.shortcut.clone()).size(12),
    ]
    .spacing(10)
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

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::super::PaletteAction;
    use super::*;
    use crate::canvas_editor::compare_floats;

    /// Builds a row that is recent or not, with no other distinguishing state.
    fn entry(is_recent: bool) -> PaletteEntry {
        PaletteEntry {
            label: String::new(),
            shortcut: String::new(),
            status: None,
            is_recent,
            action: PaletteAction::Custom(String::new()),
        }
    }

    /// Asserts two offsets are equal, since `float_cmp` is denied crate-wide.
    fn assert_same_offset(measured: f32, expected: f32) {
        assert_eq!(
            compare_floats(measured, expected),
            Ordering::Equal,
            "expected {expected}, measured {measured}"
        );
    }

    #[test]
    fn test_separator_sits_under_the_leading_recent_rows() {
        assert_eq!(separator_after(&[]), 0);
        assert_eq!(separator_after(&[entry(false), entry(false)]), 0);
        assert_eq!(
            separator_after(&[entry(true), entry(true), entry(false)]),
            2
        );
    }

    #[test]
    fn test_a_list_of_only_recent_rows_gets_no_separator() {
        assert_eq!(separator_after(&[entry(true), entry(true)]), 0);
    }

    #[test]
    fn test_row_top_shifts_every_row_below_the_separator() {
        // Without a separator the offset is the plain row pitch.
        assert_same_offset(row_top(3, 0), rows_to_pixels(3));

        // With one under row 2, the rows above it are unaffected...
        assert_same_offset(row_top(0, 2), rows_to_pixels(0));
        assert_same_offset(row_top(1, 2), rows_to_pixels(1));

        // ...and the first row under it, like every row after, moves down.
        assert_same_offset(row_top(2, 2), rows_to_pixels(2) + SEPARATOR_PITCH);
        assert_same_offset(row_top(5, 2), rows_to_pixels(5) + SEPARATOR_PITCH);
    }
}
