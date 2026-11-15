//! Search and replace dialog UI.
//!
//! This module provides the visual interface for the search/replace functionality.

use super::icons::{self, Symbol};
use super::shortcuts;
use super::{MAX_MATCHES, SearchState};
use crate::canvas_editor::Message;
use crate::i18n::Translations;
use iced::widget::{
    Space, Tooltip, button, column, container, row, text, text_editor,
    text_input, tooltip,
};
use iced::{Alignment, Border, Element, Length, Theme};

const FIELD_HEIGHT: f32 = 28.0;
const ACTION_WIDTH: f32 = 248.0;

fn control<'a>(
    content: Element<'a, Message>,
    label: String,
    message: Message,
    active: bool,
    enabled: bool,
    multiline: bool,
) -> Element<'a, Message> {
    let id = control_id(&message);
    let inline = matches!(
        message,
        Message::ToggleCaseSensitive
            | Message::ToggleWholeWord
            | Message::ToggleSearchRegex
    );
    let button_size = if inline { 20 } else { 24 };
    let hint = shortcuts::hint(&message, multiline, cfg!(target_os = "macos"));
    let label = match label.find(['(', '（']) {
        Some(index) => label[..index].trim().to_owned(),
        None => label,
    };
    let hint =
        container(text(hint).size(12).style(|theme: &Theme| text::Style {
            color: Some(
                theme.extended_palette().background.base.text.scale_alpha(0.65),
            ),
        }))
        .align_x(Alignment::End);
    let fill_active = !matches!(
        message,
        Message::ToggleSearchReplace | Message::ToggleSearchMultiline
    );
    Tooltip::new(
        container(
            button(container(content).center(Length::Fill))
                .on_press_maybe(enabled.then_some(message))
                .width(button_size)
                .height(button_size)
                .padding(if inline { 2 } else { 3 })
                .style(move |theme: &Theme, status| {
                    let palette = theme.extended_palette();
                    let mut style = button::text(theme, status);
                    style.border.radius = 4.into();
                    if active && fill_active {
                        style.background =
                            Some(palette.background.strong.color.into());
                        style.text_color = palette.background.base.text;
                    } else if matches!(status, button::Status::Hovered) {
                        style.background =
                            Some(palette.background.strong.color.into());
                    }
                    style
                }),
        )
        .id(id),
        row![text(label).size(12), hint].spacing(24).align_y(Alignment::Center),
        tooltip::Position::Bottom,
    )
    .gap(6)
    .padding(8)
    .style(|theme: &Theme| {
        let palette = theme.extended_palette();
        container::Style {
            background: Some(palette.background.weak.color.into()),
            text_color: Some(palette.background.base.text),
            border: Border {
                radius: 8.into(),
                width: 1.0,
                color: palette.background.strong.color,
            },
            shadow: iced::Shadow {
                color: iced::Color::BLACK.scale_alpha(0.14),
                offset: iced::Vector::new(0.0, 2.0),
                blur_radius: 8.0,
            },
            ..Default::default()
        }
    })
    .into()
}
fn control_id(message: &Message) -> &'static str {
    match message {
        Message::ToggleCaseSensitive => "search-case",
        Message::ToggleWholeWord => "search-word",
        Message::ToggleSearchRegex => "search-regex",
        Message::ToggleSearchReplace => "search-replace-toggle",
        Message::ToggleSearchMultiline => "search-multiline",
        Message::FindPrevious => "search-previous",
        Message::FindNext => "search-next",
        Message::CloseSearch => "search-close",
        Message::ReplaceNext => "search-replace-next",
        Message::ReplaceAll => "search-replace-all",
        Message::SelectAllSearchMatches => "search-select-all",
        _ => "search-control",
    }
}
fn icon<'a>(
    symbol: Symbol,
    label: String,
    message: Message,
    active: bool,
    enabled: bool,
    multiline: bool,
) -> Element<'a, Message> {
    control(
        icons::view(symbol, active, enabled),
        label,
        message,
        active,
        enabled,
        multiline,
    )
}

/// Builds the toolbar, reserving space above the document when open.
pub fn view<'a>(
    state: &'a SearchState,
    tr: &'a Translations,
    allow_multi_cursor: bool,
    shortcuts_enabled: bool,
) -> Element<'a, Message> {
    if !state.is_open {
        return Space::new().height(0).into();
    }
    let options = row![
        icon(
            Symbol::Case,
            tr.case_sensitive_label(),
            Message::ToggleCaseSensitive,
            state.case_sensitive,
            true,
            state.multiline
        ),
        icon(
            Symbol::Word,
            tr.whole_word_tooltip(),
            Message::ToggleWholeWord,
            state.whole_word,
            true,
            state.multiline
        ),
        icon(
            Symbol::Regex,
            tr.regex_tooltip(),
            Message::ToggleSearchRegex,
            state.use_regex,
            true,
            state.multiline
        ),
    ]
    .spacing(3)
    .align_y(Alignment::Center);
    let query: Element<'a, Message> = if state.multiline {
        text_editor(&state.search_content)
            .id(state.search_input_id.clone())
            .placeholder(tr.search_placeholder())
            .on_action(Message::SearchInputAction)
            .key_binding(editor_binding)
            .style(|theme, status| {
                let mut style = text_editor::default(theme, status);
                style.background = iced::Color::TRANSPARENT.into();
                style.border.width = 0.0;
                style.placeholder = theme
                    .extended_palette()
                    .background
                    .base
                    .text
                    .scale_alpha(0.55);
                style
            })
            .size(12)
            .padding(8)
            .height(76)
            .into()
    } else {
        text_input(&tr.search_placeholder(), &state.query)
            .id(state.search_input_id.clone())
            .on_input(Message::SearchQueryChanged)
            .on_submit(Message::FindNext)
            .size(12)
            .padding([4, 8])
            .style(|theme, status| {
                let mut style = text_input::default(theme, status);
                style.border.width = 0.0;
                style.placeholder = theme
                    .extended_palette()
                    .background
                    .base
                    .text
                    .scale_alpha(0.55);
                style.background = iced::Color::TRANSPARENT.into();
                style
            })
            .into()
    };
    let height = if state.multiline { 80.0 } else { FIELD_HEIGHT };
    let field = container(
        row![
            container(query)
                .width(Length::Fill)
                .height(height)
                .align_y(Alignment::Center),
            container(options)
                .padding(iced::Padding { right: 3.0, ..Default::default() })
                .height(FIELD_HEIGHT)
                .align_y(Alignment::Center),
        ]
        .spacing(4)
        .align_y(Alignment::Start),
    )
    .id("search-query-field")
    .padding([0, 3])
    .height(height)
    .width(Length::Fill)
    .style(field_style);
    let count = if state.match_count() >= MAX_MATCHES {
        format!("{MAX_MATCHES}+")
    } else {
        state.match_count().to_string()
    };
    let info = format!(
        "{}/{}",
        state.current_match_index.map_or(0, |index| index + 1),
        count
    );
    let has_matches = !state.matches.is_empty();
    let actions = row![
        icon(
            Symbol::ReplaceToggle,
            tr.toggle_replace_tooltip(),
            Message::ToggleSearchReplace,
            state.is_replace_mode,
            true,
            state.multiline
        ),
        icon(
            Symbol::Multiline,
            tr.multiline_tooltip(),
            Message::ToggleSearchMultiline,
            state.multiline,
            true,
            state.multiline
        ),
        icon(
            Symbol::SelectAll,
            tr.select_all_matches_tooltip(),
            Message::SelectAllSearchMatches,
            false,
            has_matches && allow_multi_cursor,
            state.multiline
        ),
        container(iced::widget::rule::vertical(1)).height(16),
        icon(
            Symbol::Previous,
            tr.previous_match_tooltip(),
            Message::FindPrevious,
            false,
            has_matches,
            state.multiline
        ),
        icon(
            Symbol::Next,
            tr.next_match_tooltip(),
            Message::FindNext,
            false,
            has_matches,
            state.multiline
        ),
        container(text(info).size(11))
            .width(Length::Fill)
            .center_y(FIELD_HEIGHT),
        icon(
            Symbol::Close,
            tr.close_search_tooltip(),
            Message::CloseSearch,
            false,
            true,
            state.multiline
        ),
    ]
    .spacing(4)
    .align_y(Alignment::Center)
    .width(ACTION_WIDTH)
    .height(FIELD_HEIGHT);
    let first = row![field, actions].spacing(6).align_y(Alignment::Start);
    let mut content = column![first].spacing(6);
    if state.is_replace_mode {
        let replacement: Element<'a, Message> = if state.multiline {
            text_editor(&state.replace_content)
                .id(state.replace_input_id.clone())
                .placeholder(tr.replace_placeholder())
                .on_action(Message::ReplaceInputAction)
                .key_binding(editor_binding)
                .style(|theme, status| {
                    let mut style = text_editor::default(theme, status);
                    style.background = iced::Color::TRANSPARENT.into();
                    style.border.width = 0.0;
                    style.placeholder = theme
                        .extended_palette()
                        .background
                        .base
                        .text
                        .scale_alpha(0.55);
                    style
                })
                .size(12)
                .padding(8)
                .height(76)
                .into()
        } else {
            text_input(&tr.replace_placeholder(), &state.replace_with)
                .id(state.replace_input_id.clone())
                .on_input(Message::ReplaceQueryChanged)
                .on_submit(Message::ReplaceNext)
                .size(12)
                .padding([4, 8])
                .style(|theme, status| {
                    let mut style = text_input::default(theme, status);
                    style.border.width = 0.0;
                    style.placeholder = theme
                        .extended_palette()
                        .background
                        .base
                        .text
                        .scale_alpha(0.55);
                    style.background = iced::Color::TRANSPARENT.into();
                    style
                })
                .into()
        };
        content = content.push(
            row![
                container(replacement)
                    .id("search-replacement-field")
                    .padding([0, 3])
                    .height(height)
                    .align_y(Alignment::Center)
                    .width(Length::Fill)
                    .style(field_style),
                row![
                    icon(
                        Symbol::Replace,
                        tr.replace_current_tooltip(),
                        Message::ReplaceNext,
                        false,
                        has_matches,
                        state.multiline
                    ),
                    icon(
                        Symbol::ReplaceAll,
                        tr.replace_all_tooltip(),
                        Message::ReplaceAll,
                        false,
                        has_matches,
                        state.multiline
                    ),
                    Space::new().width(Length::Fill),
                ]
                .spacing(4)
                .width(ACTION_WIDTH)
                .height(FIELD_HEIGHT)
                .align_y(Alignment::Center),
            ]
            .spacing(6)
            .align_y(Alignment::Start),
        );
    }
    if let Some(error) = &state.pattern_error {
        content = content.push(text(error).size(12).style(text::danger));
    }
    let toolbar = container(content).padding([6, 8]).width(Length::Fill).style(
        |theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(palette.background.base.color.into()),
                text_color: Some(palette.background.base.text),
                border: Border {
                    width: 1.0,
                    color: palette.background.strong.color,
                    radius: 0.into(),
                },
                ..Default::default()
            }
        },
    );
    iced::widget::Stack::new()
        .push(toolbar)
        .push(
            iced::widget::canvas::Canvas::new(SearchKeyListener {
                replace: state.is_replace_mode,
                enabled: shortcuts_enabled,
            })
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .into()
}

struct SearchKeyListener {
    replace: bool,
    enabled: bool,
}
impl iced::widget::canvas::Program<Message> for SearchKeyListener {
    type State = ();
    fn update(
        &self,
        _: &mut (),
        event: &iced::Event,
        _: iced::Rectangle,
        _: iced::mouse::Cursor,
    ) -> Option<iced::widget::canvas::Action<Message>> {
        if !self.enabled {
            return None;
        }
        if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key,
            modified_key,
            modifiers,
            ..
        }) = event
            && let Some(message) = shortcuts::resolve(
                key,
                modified_key,
                *modifiers,
                self.replace,
                cfg!(target_os = "macos"),
            )
        {
            return Some(
                iced::widget::canvas::Action::publish(message).and_capture(),
            );
        }
        None
    }
    fn draw(
        &self,
        _: &(),
        _: &iced::Renderer,
        _: &Theme,
        _: iced::Rectangle,
        _: iced::mouse::Cursor,
    ) -> Vec<iced::widget::canvas::Geometry> {
        Vec::new()
    }
}

fn field_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.base.color.into()),
        border: Border {
            color: palette.background.strong.color,
            width: 1.0,
            radius: 5.into(),
        },
        ..Default::default()
    }
}
fn editor_binding(
    event: text_editor::KeyPress,
) -> Option<text_editor::Binding<Message>> {
    use iced::keyboard::{Key, key::Named};
    match event.key {
        Key::Named(Named::Escape) => {
            Some(text_editor::Binding::Custom(Message::CloseSearch))
        }
        Key::Named(Named::Tab) => {
            Some(text_editor::Binding::Custom(if event.modifiers.shift() {
                Message::SearchDialogShiftTab
            } else {
                Message::SearchDialogTab
            }))
        }
        Key::Named(Named::F3) => {
            Some(text_editor::Binding::Custom(if event.modifiers.shift() {
                Message::FindPrevious
            } else {
                Message::FindNext
            }))
        }
        _ => text_editor::Binding::from_key_press(event),
    }
}
