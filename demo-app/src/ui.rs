use crate::app::{DemoApp, Message};
use crate::types::{EditorId, FontOption, LanguageOption, PaneType, Template};
use iced::widget::pane_grid::{Content, TitleBar};
use iced::widget::{
    PaneGrid, Space, button, center, checkbox, column, container, mouse_area,
    pick_list, row, scrollable, slider, stack, text, text_input,
};
use iced::{Color, Element, Length, Theme};

/// Renders the user interface.
pub fn view(app: &DemoApp) -> Element<'_, Message> {
    let palette = app.current_theme.extended_palette();
    let text_color = palette.background.base.text;

    // Toolbar
    let toolbar = row![
        button(text("Open")).on_press(Message::OpenFile),
        button(text("Save")).on_press(Message::SaveFile),
        button(text("Save As")).on_press(Message::SaveFileAs),
        button(text("Run")).on_press(Message::RunCode),
        text(file_status(app))
            .style(move |_| text::Style { color: Some(text_color) }),
        Space::new().width(Length::Fill),
        mouse_area(
            text_input("Input for testing focus ...", &app.text_input_value)
                .on_input(Message::TextInputChanged)
                .width(200)
        )
        .on_press(Message::TextInputClicked),
        Space::new().width(10),
        button(text("Settings")).on_press(Message::ToggleSettings),
    ]
    .spacing(10)
    .padding(10)
    .align_y(iced::Center);

    // Error message if any
    let error_bar = if let Some(err) = &app.error_message {
        container(text(format!("Error: {}", err)).style(|_| text::Style {
            color: Some(Color::from_rgb(1.0, 0.3, 0.3)),
        }))
        .padding(5)
        .width(Length::Fill)
    } else {
        container(text("")).height(0)
    };

    // PaneGrid with two editors (resizable horizontally)
    let editors_pane_grid =
        PaneGrid::new(&app.panes, |_id, pane, _is_maximized| {
            let title_bar_style = palette.background.weak.color;

            match pane {
                PaneType::EditorLeft => {
                    let title =
                        TitleBar::new(text("Editor (Left)").style(move |_| {
                            text::Style { color: Some(text_color) }
                        }))
                        .style(move |_| container::Style {
                            background: Some(iced::Background::Color(
                                title_bar_style,
                            )),
                            ..Default::default()
                        })
                        .padding(5);

                    Content::new(view_editor_pane(
                        app,
                        EditorId::Left,
                        text_color,
                    ))
                    .title_bar(title)
                    .style(move |_| container::Style {
                        background: Some(iced::Background::Color(
                            palette.background.base.color,
                        )),
                        ..Default::default()
                    })
                }
                PaneType::EditorRight => {
                    let title = TitleBar::new(text("Editor (Right)").style(
                        move |_| text::Style { color: Some(text_color) },
                    ))
                    .style(move |_| container::Style {
                        background: Some(iced::Background::Color(
                            title_bar_style,
                        )),
                        ..Default::default()
                    })
                    .padding(5);

                    Content::new(view_editor_pane(
                        app,
                        EditorId::Right,
                        text_color,
                    ))
                    .title_bar(title)
                    .style(move |_| container::Style {
                        background: Some(iced::Background::Color(
                            palette.background.base.color,
                        )),
                        ..Default::default()
                    })
                }
            }
        })
        .on_resize(10, Message::PaneResized)
        .spacing(2)
        .height(Length::FillPortion(7)); // 70% of available height

    // Output view (separate from PaneGrid)
    let title_bar_style = palette.background.weak.color;
    let output_title = container(
        text("Output").style(move |_| text::Style { color: Some(text_color) }),
    )
    .padding(5)
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(iced::Background::Color(title_bar_style)),
        ..Default::default()
    });

    let output_content = container(view_output_pane(app, text_color))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(
                palette.background.base.color,
            )),
            ..Default::default()
        });

    let output_view = column![output_title, output_content]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::FillPortion(3)); // 30% of available height

    // Main layout: column with toolbar, error_bar, editors, and output
    let content = container(
        column![toolbar, error_bar, editors_pane_grid, output_view]
            .spacing(2)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .style(move |_| container::Style {
        background: Some(iced::Background::Color(
            palette.background.base.color,
        )),
        ..Default::default()
    })
    .width(Length::Fill)
    .height(Length::Fill);

    if app.show_settings {
        let modal = center(
            container(
                column![
                    text("Settings").size(24),
                    row![
                        text("Font:").width(80),
                        pick_list(
                            FontOption::ALL,
                            Some(app.current_font),
                            Message::FontChanged
                        )
                    ]
                    .spacing(10)
                    .align_y(iced::Center),
                    row![
                        text(format!("Size: {:.0}", app.current_font_size))
                            .width(80),
                        slider(
                            10.0..=30.0,
                            app.current_font_size,
                            Message::FontSizeChanged
                        )
                        .width(150)
                    ]
                    .spacing(10)
                    .align_y(iced::Center),
                    checkbox(app.auto_adjust_line_height)
                        .label("Auto-adjust Line Height")
                        .on_toggle(Message::ToggleAutoLineHeight),
                    row![
                        text(format!("Height: {:.1}", app.current_line_height))
                            .width(80),
                        slider(
                            10.0..=50.0,
                            app.current_line_height,
                            Message::LineHeightChanged
                        )
                        .width(150)
                    ]
                    .spacing(10)
                    .align_y(iced::Center),
                    row![
                        text("Language:").width(80),
                        pick_list(
                            LanguageOption::ALL,
                            Some(LanguageOption::from(app.current_language)),
                            Message::LanguageChanged
                        )
                    ]
                    .spacing(10)
                    .align_y(iced::Center),
                    row![
                        text("Theme:").width(80),
                        pick_list(
                            Theme::ALL,
                            Some(app.current_theme.clone()),
                            Message::ThemeChanged
                        )
                    ]
                    .spacing(10)
                    .align_y(iced::Center),
                    button("Close").on_press(Message::ToggleSettings)
                ]
                .spacing(20)
                .padding(20),
            )
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(
                    palette.background.weak.color,
                )),
                border: iced::Border {
                    color: palette.primary.base.color,
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..Default::default()
            }),
        );

        stack![
            content,
            mouse_area(
                container(
                    Space::new().width(Length::Fill).height(Length::Fill)
                )
                .style(|_| container::Style {
                    background: Some(
                        Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()
                    ),
                    ..Default::default()
                })
            )
            .on_press(Message::ToggleSettings),
            modal
        ]
        .into()
    } else {
        content.into()
    }
}

/// Renders the editor pane content.
pub fn view_editor_pane(
    app: &DemoApp,
    editor_id: EditorId,
    _text_color: Color,
) -> Element<'_, Message> {
    // Select data based on editor_id
    let (editor, search_replace_enabled, line_numbers_enabled) = match editor_id
    {
        EditorId::Left => (
            &app.editor_left,
            app.search_replace_enabled_left,
            app.line_numbers_enabled_left,
        ),
        EditorId::Right => (
            &app.editor_right,
            app.search_replace_enabled_right,
            app.line_numbers_enabled_right,
        ),
    };

    // Template picker using pick_list
    let template_picker =
        pick_list(Template::ALL, None::<Template>, move |template| {
            Message::TemplateSelected(editor_id, template)
        })
        .placeholder("Choose template...")
        .text_size(14);

    // Wrap checkbox
    let wrap_checkbox = checkbox(editor.wrap_enabled())
        .label("Line wrapping")
        .on_toggle(move |b| Message::ToggleWrap(editor_id, b))
        .text_size(14);

    // Search/replace checkbox
    let search_replace_checkbox = checkbox(search_replace_enabled)
        .label("Allow search/replace")
        .on_toggle(move |b| Message::ToggleSearchReplace(editor_id, b))
        .text_size(14);

    // Line numbers checkbox
    let line_numbers_checkbox = checkbox(line_numbers_enabled)
        .label("Show line numbers")
        .on_toggle(move |b| Message::ToggleLineNumbers(editor_id, b))
        .text_size(14);

    // Editor in a constrained container (400px height, clipped)
    let editor_view = container(
        editor.view().map(move |e| Message::EditorEvent(editor_id, e)),
    )
    .width(Length::Fill)
    .clip(true)
    .style(|_| container::Style {
        border: iced::Border {
            color: Color::from_rgb(0.3, 0.3, 0.35),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    });

    container(
        column![
            row![
                template_picker,
                Space::new().width(10),
                wrap_checkbox,
                Space::new().width(10),
                search_replace_checkbox,
                Space::new().width(10),
                line_numbers_checkbox
            ]
            .padding(10),
            editor_view,
        ]
        .spacing(5)
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .clip(true)
    .into()
}

/// Renders the output pane content.
pub fn view_output_pane(
    app: &DemoApp,
    text_color: Color,
) -> Element<'_, Message> {
    // Clear button with hover effect
    let clear_button = button(text("Clear").size(12))
        .on_press(Message::ClearLog)
        .padding(4)
        .style(|theme: &iced::Theme, status| {
            let palette = theme.extended_palette();
            match status {
                iced::widget::button::Status::Hovered => {
                    iced::widget::button::Style {
                        background: Some(iced::Background::Color(
                            palette.primary.weak.color,
                        )),
                        text_color: palette.primary.weak.text,
                        border: iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }
                _ => iced::widget::button::Style {
                    background: Some(iced::Background::Color(
                        palette.background.weak.color,
                    )),
                    text_color: palette.background.weak.text,
                    border: iced::Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            }
        });

    // Toolbar row with Clear button aligned to the right
    let toolbar = row![Space::new().width(Length::Fill), clear_button]
        .padding(5)
        .align_y(iced::Center);

    // Log messages content
    let log_content: Vec<Element<'_, Message>> = app
        .log_messages
        .iter()
        .map(|msg| {
            let color = if msg.contains("[ERROR]") {
                Color::from_rgb(1.0, 0.4, 0.4)
            } else if msg.contains("[OUTPUT]") {
                Color::from_rgb(0.4, 1.0, 0.4)
            } else {
                text_color
            };

            text(msg)
                .size(13)
                .style(move |_| text::Style { color: Some(color) })
                .into()
        })
        .collect();

    let log_scrollable = scrollable(
        column(log_content).spacing(2).padding(10).width(Length::Fill),
    )
    .height(Length::Fill)
    .width(Length::Fill);

    column![toolbar, log_scrollable]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Returns the file status string.
pub fn file_status(app: &DemoApp) -> String {
    let left_name = app
        .current_file_left
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("New file");
    let right_name = app
        .current_file_right
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("New file");

    let left_mod = if app.editor_left.is_modified() { "*" } else { "" };
    let right_mod = if app.editor_right.is_modified() { "*" } else { "" };

    let active_left =
        if app.active_editor == EditorId::Left { "● " } else { "" };
    let active_right =
        if app.active_editor == EditorId::Right { " ●" } else { "" };

    format!(
        "{}L: {}{} | R: {}{}{}",
        active_left, left_name, left_mod, right_name, right_mod, active_right
    )
}
