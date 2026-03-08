mod lsp;

use crate::app::{DemoApp, EditorTab, Message};
use crate::types::{FontOption, LanguageOption, Template};
use iced::widget::{
    Space, button, center, checkbox, column, container, mouse_area, pick_list,
    row, scrollable, slider, stack, text, text_input,
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
        button(text("+ New Tab")).on_press(Message::NewTab),
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

    // Tab Bar
    let tabs_list = row(app
        .tabs
        .iter()
        .map(|tab| view_tab_header(tab, tab.id == app.active_tab_id))
        .collect::<Vec<_>>())
    .spacing(2);

    let tab_height = 38.0;
    let tab_scrollbar_height = 12.0;
    let tab_bar_height = if app.tabs_overflow {
        tab_height + tab_scrollbar_height
    } else {
        tab_height
    };

    let tab_bar = scrollable(tabs_list)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new(),
        ))
        .height(tab_bar_height)
        .style(|theme: &Theme, _status| {
            let palette = theme.extended_palette();
            scrollable::Style {
                container: container::Style {
                    background: Some(palette.background.weak.color.into()),
                    ..Default::default()
                },
                vertical_rail: scrollable::Rail {
                    background: Some(palette.background.weak.color.into()),
                    border: iced::Border {
                        radius: 4.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    scroller: scrollable::Scroller {
                        background: palette.primary.weak.color.into(),
                        border: iced::Border {
                            radius: 4.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                    },
                },
                horizontal_rail: scrollable::Rail {
                    background: Some(palette.background.weak.color.into()),
                    border: iced::Border {
                        radius: 4.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    scroller: scrollable::Scroller {
                        background: palette.primary.weak.color.into(),
                        border: iced::Border {
                            radius: 4.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                    },
                },
                gap: None,
                auto_scroll: scrollable::AutoScroll {
                    background: Color::TRANSPARENT.into(),
                    border: iced::Border::default(),
                    shadow: iced::Shadow::default(),
                    icon: Color::TRANSPARENT,
                },
            }
        });

    // File path display below tabs
    let file_path_display: Element<'_, Message> = if let Some(tab) =
        app.tabs.iter().find(|t| t.id == app.active_tab_id)
    {
        let path_text = tab
            .file_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "Untitled".to_string());

        container(text(path_text).size(12).style(move |_| text::Style {
            color: Some(palette.background.weak.text),
        }))
        .padding([4, 10])
        .width(Length::Fill)
        .style(move |theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(palette.background.weak.color.into()),
                border: iced::Border {
                    width: 1.0,
                    color: palette.background.strong.color,
                    ..iced::Border::default()
                },
                ..Default::default()
            }
        })
        .into()
    } else {
        container(text("")).height(0).into()
    };

    // Editor Pane
    let editor_pane = if let Some(tab) =
        app.tabs.iter().find(|t| t.id == app.active_tab_id)
    {
        view_editor_pane(app, tab, text_color)
    } else {
        container(text("No open tabs").size(20))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    };

    // Output view
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

    // Main layout
    let content = container(
        column![
            toolbar,
            error_bar,
            tab_bar,
            file_path_display,
            editor_pane,
            output_view
        ]
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

fn view_tab_header(tab: &EditorTab, is_active: bool) -> Element<'_, Message> {
    let name = tab
        .file_path
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("Untitled");
    let modified = if tab.is_dirty { "*" } else { "" };
    let label = format!("{}{}", name, modified);

    let label_text = text(label).size(14).style(move |theme: &Theme| {
        let palette = theme.extended_palette();
        text::Style {
            color: Some(if is_active {
                palette.background.base.text
            } else {
                let mut color = palette.background.base.text;
                color.a = 0.6;
                color
            }),
        }
    });

    let close_btn = button(text("×").size(16))
        .on_press(Message::CloseTab(tab.id))
        .padding(0)
        .width(20)
        .style(button::text);

    // Active tab indicator (top line)
    let indicator: Element<'_, Message> = if is_active {
        container(Space::new())
            .width(Length::Fill)
            .height(2)
            .style(move |theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(palette.primary.base.color.into()),
                    ..Default::default()
                }
            })
            .into()
    } else {
        container(Space::new()).width(Length::Fill).height(2).into()
    };

    let content = column![
        indicator,
        container(
            row![
                button(label_text)
                    .on_press(Message::SelectTab(tab.id))
                    .style(button::text),
                close_btn
            ]
            .spacing(5)
            .align_y(iced::Center)
        )
        .padding([3, 10])
        .height(Length::Fill)
    ]
    .spacing(0);

    container(content)
        .height(38)
        .style(move |theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(
                    if is_active {
                        palette.background.base.color
                    } else {
                        palette.background.weak.color
                    }
                    .into(),
                ),
                border: iced::Border {
                    width: 1.0,
                    color: palette.background.strong.color,
                    ..iced::Border::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Renders the editor pane content.
pub fn view_editor_pane<'a>(
    app: &'a DemoApp,
    tab: &'a EditorTab,
    _text_color: Color,
) -> Element<'a, Message> {
    let editor_id = tab.id;
    let editor = &tab.editor;

    // We assume these settings are global for now, but could be per-tab
    let wrap_enabled = editor.wrap_enabled();
    let search_replace_enabled = editor.search_replace_enabled();
    let line_numbers_enabled = editor.line_numbers_enabled();

    // Template picker using pick_list
    let template_picker =
        pick_list(Template::ALL, None::<Template>, move |template| {
            Message::TemplateSelected(editor_id, template)
        })
        .placeholder("Choose template...")
        .text_size(14);

    // Wrap checkbox
    let wrap_checkbox = checkbox(wrap_enabled)
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

    // LSP Status
    #[cfg(not(target_arch = "wasm32"))]
    let lsp_status: Element<'_, Message> = if let Some(key) = tab.lsp_server_key
    {
        let (status_text, is_working, is_finishing) =
            if let Some(progress_map) = app.lsp_progress.get(key) {
                if let Some(progress) = progress_map.values().next() {
                    let percent_val = progress.percentage.unwrap_or(0);
                    if percent_val >= 100 {
                        (format!("LSP: {} (Finishing...)", key), true, true)
                    } else {
                        let percent = progress
                            .percentage
                            .map(|p| format!(" {}%", p))
                            .unwrap_or_default();
                        let msg = progress
                            .message
                            .as_ref()
                            .map(|m| format!(": {}", m))
                            .unwrap_or_default();
                        (
                            format!(
                                "LSP: {} ({}{}{})",
                                key, progress.title, msg, percent
                            ),
                            true,
                            false,
                        )
                    }
                } else {
                    (format!("LSP: {}", key), false, false)
                }
            } else {
                (format!("LSP: {}", key), false, false)
            };

        let spinner = if is_working {
            if is_finishing {
                text("✓").size(14).style(move |theme: &Theme| {
                    let palette = theme.extended_palette();
                    text::Style { color: Some(palette.success.base.color) }
                })
            } else {
                let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let frame = frames[app.spinner_frame % frames.len()];
                text(frame).size(14).font(iced::font::Font::MONOSPACE).style(
                    move |theme: &Theme| {
                        let palette = theme.extended_palette();
                        text::Style { color: Some(palette.primary.base.color) }
                    },
                )
            }
        } else {
            text("●").size(14).style(move |theme: &Theme| {
                let palette = theme.extended_palette();
                text::Style { color: Some(palette.success.base.color) }
            })
        };

        row![
            spinner,
            text(status_text).size(14).style(move |theme: &Theme| {
                let palette = theme.extended_palette();
                text::Style { color: Some(palette.success.base.color) }
            })
        ]
        .spacing(5)
        .align_y(iced::Center)
        .into()
    } else {
        text("LSP: Inactive")
            .size(14)
            .style(move |theme: &Theme| {
                let palette = theme.extended_palette();
                text::Style { color: Some(palette.danger.base.color) }
            })
            .into()
    };

    #[cfg(target_arch = "wasm32")]
    let lsp_status: Element<'_, Message> = text("LSP: N/A").size(14).into();

    // Editor in a constrained container
    let editor_view = container(
        editor.view().map(move |e| Message::EditorEvent(editor_id, e)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .clip(true)
    .style(|_| container::Style {
        border: iced::Border {
            color: Color::from_rgb(0.3, 0.3, 0.35),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    });

    let overlay = lsp::view_lsp_overlay(app, editor_id);
    let editor_stack: Element<'_, Message> =
        stack![editor_view, overlay].into();
    let editor_stack = mouse_area(editor_stack)
        .on_enter(Message::EditorMouseEntered(editor_id))
        .on_exit(Message::EditorMouseExited(editor_id));

    container(
        column![
            row![
                template_picker,
                Space::new().width(10),
                wrap_checkbox,
                Space::new().width(10),
                search_replace_checkbox,
                Space::new().width(10),
                line_numbers_checkbox,
                Space::new().width(10),
                lsp_status
            ]
            .padding(10),
            editor_stack,
        ]
        .spacing(5)
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::FillPortion(7))
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

    let lsp_panel = lsp::view_lsp_panel();

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

    column![toolbar, lsp_panel, log_scrollable]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
