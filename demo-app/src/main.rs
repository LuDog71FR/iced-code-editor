//! Demo application for iced-code-editor with pane_grid layout.
//!
//! This demo reproduces a typical IDE layout with:
//! - A toolbar at the top
//! - A vertical pane_grid with:
//!   - Top pane: DropDown menu + CodeEditor (height constrained to 400px)
//!   - Bottom pane: Output/Log area
//!
//! Remarks:
//! This layout is designed to test overflow and z-index issues.

use iced::widget::pane_grid::{Content, TitleBar};
use iced::widget::{
    PaneGrid, Space, button, center, checkbox, column, container, mouse_area,
    pane_grid, pick_list, row, scrollable, slider, stack, text, text_input,
};
use iced::{Color, Element, Length, Subscription, Task, Theme, window};
use iced_code_editor::Message as EditorMessage;
use iced_code_editor::{CodeEditor, Language, theme};
use std::path::PathBuf;

/// Main entry point for the demo application.
fn main() -> iced::Result {
    iced::application(DemoApp::new, DemoApp::update, DemoApp::view)
        .subscription(DemoApp::subscription)
        .theme(DemoApp::theme)
        .font(include_bytes!("../../fonts/JetBrainsMono-Regular.ttf").as_slice())
        .run()
}

/// Identifier for which editor is being referenced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorId {
    Left,
    Right,
}

/// Wrapper for Font to implement Display trait for pick_list.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FontOption {
    name: &'static str,
    font: iced::Font,
}

impl FontOption {
    const MONOSPACE: FontOption =
        FontOption { name: "Monospace (Default)", font: iced::Font::MONOSPACE };

    const SERIF: FontOption = FontOption {
        name: "Serif",
        font: iced::Font {
            family: iced::font::Family::Serif,
            ..iced::Font::DEFAULT
        },
    };

    const SANS_SERIF: FontOption = FontOption {
        name: "Sans Serif",
        font: iced::Font {
            family: iced::font::Family::SansSerif,
            ..iced::Font::DEFAULT
        },
    };

    const JETBRAINS_MONO: FontOption = FontOption {
        name: "JetBrains Mono",
        font: iced::Font {
            family: iced::font::Family::Name("JetBrains Mono"),
            ..iced::Font::DEFAULT
        },
    };

    const ALL: [FontOption; 4] = [
        FontOption::MONOSPACE,
        FontOption::SERIF,
        FontOption::SANS_SERIF,
        FontOption::JETBRAINS_MONO,
    ];

    fn font(&self) -> iced::Font {
        self.font
    }
}

impl std::fmt::Display for FontOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Wrapper for Language to implement Display trait for pick_list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LanguageOption(Language);

impl LanguageOption {
    const ALL: [LanguageOption; 8] = [
        LanguageOption(Language::German),
        LanguageOption(Language::English),
        LanguageOption(Language::Spanish),
        LanguageOption(Language::French),
        LanguageOption(Language::Italian),
        LanguageOption(Language::PortugueseBR),
        LanguageOption(Language::PortuguesePT),
        LanguageOption(Language::ChineseSimplified),
    ];

    fn inner(&self) -> Language {
        self.0
    }
}

impl From<Language> for LanguageOption {
    fn from(lang: Language) -> Self {
        LanguageOption(lang)
    }
}

impl std::fmt::Display for LanguageOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Language::English => write!(f, "English"),
            Language::French => write!(f, "Français"),
            Language::Spanish => write!(f, "Español"),
            Language::German => write!(f, "Deutsch"),
            Language::Italian => write!(f, "Italiano"),
            Language::PortugueseBR => write!(f, "Português (BR)"),
            Language::PortuguesePT => write!(f, "Português (PT)"),
            Language::ChineseSimplified => write!(f, "简体中文"),
        }
    }
}

/// Code templates available in the dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Template {
    Empty,
    HelloWorld,
    Fibonacci,
    Factorial,
}

impl Template {
    const ALL: [Template; 4] = [
        Template::Empty,
        Template::HelloWorld,
        Template::Fibonacci,
        Template::Factorial,
    ];

    fn name(&self) -> &'static str {
        match self {
            Template::Empty => "Empty",
            Template::HelloWorld => "Hello World",
            Template::Fibonacci => "Fibonacci",
            Template::Factorial => "Factorial",
        }
    }

    fn content(&self) -> &'static str {
        match self {
            Template::Empty => "",
            Template::HelloWorld => {
                r#"-- Hello World in Lua
print("Hello, World!")
"#
            }
            Template::Fibonacci => {
                r#"-- Fibonacci sequence in Lua
function fibonacci(n)
    if n <= 1 then
        return n
    end
    return fibonacci(n - 1) + fibonacci(n - 2)
end

-- Print first 10 Fibonacci numbers
for i = 0, 10 do
    print("fib(" .. i .. ") = " .. fibonacci(i))
end
"#
            }
            Template::Factorial => {
                r#"-- Factorial function in Lua
function factorial(n)
    if n <= 1 then
        return 1
    end
    return n * factorial(n - 1)
end

-- Calculate factorials
for i = 1, 10 do
    print(i .. "! = " .. factorial(i))
end
"#
            }
        }
    }
}

impl std::fmt::Display for Template {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Pane content types for the editor PaneGrid.
#[derive(Debug, Clone, Copy)]
enum PaneType {
    EditorLeft,
    EditorRight,
}

/// Demo application state.
struct DemoApp {
    /// Left code editor
    editor_left: CodeEditor,
    /// Right code editor
    editor_right: CodeEditor,
    /// Current file path for left editor
    current_file_left: Option<PathBuf>,
    /// Current file path for right editor
    current_file_right: Option<PathBuf>,
    /// Error message
    error_message: Option<String>,
    /// Current theme
    current_theme: Theme,
    /// Current UI language
    current_language: Language,
    /// Current font
    current_font: FontOption,
    /// Current font size
    current_font_size: f32,
    /// Current line height
    current_line_height: f32,
    /// Pane grid state
    panes: pane_grid::State<PaneType>,
    /// Log messages for output pane
    log_messages: Vec<String>,
    /// Search/replace enabled flag for left editor
    search_replace_enabled_left: bool,
    /// Search/replace enabled flag for right editor
    search_replace_enabled_right: bool,
    /// Line numbers enabled flag for left editor
    line_numbers_enabled_left: bool,
    /// Line numbers enabled flag for right editor
    line_numbers_enabled_right: bool,
    /// Active editor (receives Open/Save/Run commands)
    active_editor: EditorId,
    /// Test text input value
    text_input_value: String,
    /// Whether to show the settings modal
    show_settings: bool,
    /// Whether to automatically adjust line height when font size changes
    auto_adjust_line_height: bool,
}

/// Application messages.
#[derive(Debug, Clone)]
enum Message {
    /// Toggle settings modal
    ToggleSettings,
    /// Toggle auto adjust line height
    ToggleAutoLineHeight(bool),
    /// Editor event
    EditorEvent(EditorId, EditorMessage),
    /// Open file
    OpenFile,
    /// File opened
    FileOpened(Result<(PathBuf, String), String>),
    /// Save file
    SaveFile,
    /// Save file as
    SaveFileAs,
    /// File saved
    FileSaved(Result<PathBuf, String>),
    /// Cursor blink tick
    Tick,
    /// Font changed
    FontChanged(FontOption),
    /// Font size changed
    FontSizeChanged(f32),
    /// Line height changed
    LineHeightChanged(f32),
    /// UI Language changed
    LanguageChanged(LanguageOption),
    /// Theme changed
    ThemeChanged(Theme),
    /// Pane resized
    PaneResized(pane_grid::ResizeEvent),
    /// Template selected
    TemplateSelected(EditorId, Template),
    /// Clear log
    ClearLog,
    /// Run code (simulated)
    RunCode,
    /// Toggle line wrapping
    ToggleWrap(EditorId, bool),
    /// Toggle search/replace
    ToggleSearchReplace(EditorId, bool),
    /// Toggle line numbers
    ToggleLineNumbers(EditorId, bool),
    /// Test text input changed
    TextInputChanged(String),
    /// Test text input clicked
    TextInputClicked,
}

impl DemoApp {
    /// Creates a new instance of the application.
    fn new() -> (Self, Task<Message>) {
        let default_content = r#"-- Lua code editor demo
-- This demo tests pane_grid layout with CodeEditor

function greet(name)
    print("Hello, " .. name .. "!")
end

greet("World")
"#;
        // Create PaneGrid with two editors side by side
        let (mut panes, left_pane) =
            pane_grid::State::new(PaneType::EditorLeft);

        // Split vertical to create EditorRight beside EditorLeft
        if let Some((_right_pane, split_v)) = panes.split(
            pane_grid::Axis::Vertical,
            left_pane,
            PaneType::EditorRight,
        ) {
            panes.resize(split_v, 0.5); // 50/50 between left and right editors
        }

        let log_messages = vec![
            "[INFO] Application started".to_string(),
            "[INFO] Two editors initialized side by side".to_string(),
        ];

        let current_font = FontOption::MONOSPACE;

        (
            Self {
                editor_left: CodeEditor::new(default_content, "lua"),
                editor_right: CodeEditor::new(default_content, "lua"),
                current_file_left: None,
                current_file_right: None,
                error_message: None,
                current_theme: Theme::TokyoNightStorm,
                current_language: Language::English,
                current_font,
                current_font_size: 14.0,
                current_line_height: 20.0,
                panes,
                log_messages,
                search_replace_enabled_left: true,
                search_replace_enabled_right: true,
                line_numbers_enabled_left: true,
                line_numbers_enabled_right: true,
                active_editor: EditorId::Left,
                text_input_value: String::new(),
                show_settings: false,
                auto_adjust_line_height: true,
            },
            Task::none(),
        )
    }

    /// Adds a log message.
    fn log(&mut self, level: &str, message: &str) {
        self.log_messages.push(format!("[{}] {}", level, message));
    }

    /// Handles messages and updates the application state.
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToggleSettings => {
                self.show_settings = !self.show_settings;
                Task::none()
            }
            Message::ToggleAutoLineHeight(enabled) => {
                self.auto_adjust_line_height = enabled;
                Task::none()
            }
            Message::EditorEvent(editor_id, event) => {
                let editor = match editor_id {
                    EditorId::Left => &mut self.editor_left,
                    EditorId::Right => &mut self.editor_right,
                };
                editor
                    .update(&event)
                    .map(move |e| Message::EditorEvent(editor_id, e))
            }
            Message::OpenFile => {
                self.log(
                    "INFO",
                    &format!(
                        "Opening file for {:?} editor...",
                        self.active_editor
                    ),
                );
                Task::perform(open_file_dialog(), Message::FileOpened)
            }
            Message::FileOpened(result) => match result {
                Ok((path, content)) => {
                    self.log(
                        "INFO",
                        &format!(
                            "Opened {} in {:?} editor",
                            path.display(),
                            self.active_editor
                        ),
                    );

                    let (editor, current_file) = match self.active_editor {
                        EditorId::Left => {
                            (&mut self.editor_left, &mut self.current_file_left)
                        }
                        EditorId::Right => (
                            &mut self.editor_right,
                            &mut self.current_file_right,
                        ),
                    };

                    let task = editor.reset(&content);
                    let style = theme::from_iced_theme(&self.current_theme);
                    editor.set_theme(style);
                    editor.mark_saved();
                    *current_file = Some(path);
                    self.error_message = None;

                    let active_editor = self.active_editor;
                    task.map(move |e| Message::EditorEvent(active_editor, e))
                }
                Err(err) => {
                    self.log("ERROR", &err);
                    self.error_message = Some(err);
                    Task::none()
                }
            },
            Message::SaveFile => {
                let current_file = match self.active_editor {
                    EditorId::Left => self.current_file_left.clone(),
                    EditorId::Right => self.current_file_right.clone(),
                };
                if let Some(path) = current_file {
                    self.log("INFO", &format!("Saving to: {}", path.display()));
                    let editor = match self.active_editor {
                        EditorId::Left => &self.editor_left,
                        EditorId::Right => &self.editor_right,
                    };
                    let content = editor.content();
                    Task::perform(save_file(path, content), Message::FileSaved)
                } else {
                    self.update(Message::SaveFileAs)
                }
            }
            Message::SaveFileAs => {
                self.log("INFO", "Opening save dialog...");
                let editor = match self.active_editor {
                    EditorId::Left => &self.editor_left,
                    EditorId::Right => &self.editor_right,
                };
                let content = editor.content();
                Task::perform(save_file_as_dialog(content), Message::FileSaved)
            }
            Message::FileSaved(result) => {
                match result {
                    Ok(path) => {
                        self.log("INFO", &format!("Saved: {}", path.display()));
                        let (editor, current_file) = match self.active_editor {
                            EditorId::Left => (
                                &mut self.editor_left,
                                &mut self.current_file_left,
                            ),
                            EditorId::Right => (
                                &mut self.editor_right,
                                &mut self.current_file_right,
                            ),
                        };
                        *current_file = Some(path);
                        editor.mark_saved();
                        self.error_message = None;
                    }
                    Err(err) => {
                        self.log("ERROR", &err);
                        self.error_message = Some(err);
                    }
                }
                Task::none()
            }
            Message::Tick => {
                // Handle cursor blinking only if editor has focus
                let task_left = self
                    .editor_left
                    .update(&EditorMessage::Tick)
                    .map(|e| Message::EditorEvent(EditorId::Left, e));
                let task_right = self
                    .editor_right
                    .update(&EditorMessage::Tick)
                    .map(|e| Message::EditorEvent(EditorId::Right, e));
                Task::batch([task_left, task_right])
            }
            Message::FontChanged(font_option) => {
                self.log(
                    "INFO",
                    &format!("Font changed to: {}", font_option.name),
                );
                self.current_font = font_option;

                let font = font_option.font();
                self.editor_left.set_font(font);
                self.editor_right.set_font(font);
                Task::none()
            }
            Message::FontSizeChanged(size) => {
                self.current_font_size = size;
                
                if self.auto_adjust_line_height {
                    // Auto-adjust line height ratio is 20/14 ~ 1.428
                    let new_line_height = size * (20.0 / 14.0);
                    self.current_line_height = new_line_height;
                }
                
                self.editor_left.set_font_size(size, self.auto_adjust_line_height);
                self.editor_right.set_font_size(size, self.auto_adjust_line_height);
                Task::none()
            }
            Message::LineHeightChanged(height) => {
                self.current_line_height = height;
                self.editor_left.set_line_height(height);
                self.editor_right.set_line_height(height);
                Task::none()
            }
            Message::LanguageChanged(lang_option) => {
                let new_language = lang_option.inner();
                self.log(
                    "INFO",
                    &format!("UI Language changed to: {}", lang_option),
                );
                self.current_language = new_language;
                self.editor_left.set_language(new_language);
                self.editor_right.set_language(new_language);
                Task::none()
            }
            Message::ThemeChanged(new_theme) => {
                self.log("INFO", &format!("Theme changed to: {:?}", new_theme));
                let style = theme::from_iced_theme(&new_theme);
                self.current_theme = new_theme;
                self.editor_left.set_theme(style);
                self.editor_right.set_theme(style);
                Task::none()
            }
            Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
                Task::none()
            }
            Message::TemplateSelected(editor_id, template) => {
                self.log(
                    "INFO",
                    &format!(
                        "Template '{}' loaded in {:?} editor",
                        template.name(),
                        editor_id
                    ),
                );

                let editor = match editor_id {
                    EditorId::Left => &mut self.editor_left,
                    EditorId::Right => &mut self.editor_right,
                };

                let task = editor.reset(template.content());
                let style = theme::from_iced_theme(&self.current_theme);
                editor.set_theme(style);

                match editor_id {
                    EditorId::Left => {
                        self.current_file_left = None;
                    }
                    EditorId::Right => {
                        self.current_file_right = None;
                    }
                }
                task.map(move |e| Message::EditorEvent(editor_id, e))
            }
            Message::ClearLog => {
                self.log_messages.clear();
                self.log("INFO", "Log cleared");
                Task::none()
            }
            Message::RunCode => {
                self.log(
                    "INFO",
                    &format!(
                        "Running code from {:?} editor...",
                        self.active_editor
                    ),
                );

                let editor = match self.active_editor {
                    EditorId::Left => &self.editor_left,
                    EditorId::Right => &self.editor_right,
                };

                let line_count = editor.content().lines().count();
                self.log("OUTPUT", &format!("Script has {} lines", line_count));
                self.log("OUTPUT", "Execution completed (simulated)");
                Task::none()
            }
            Message::ToggleWrap(editor_id, enabled) => {
                self.log(
                    "INFO",
                    &format!(
                        "Line wrapping {} in {:?} editor",
                        if enabled { "enabled" } else { "disabled" },
                        editor_id
                    ),
                );

                let editor = match editor_id {
                    EditorId::Left => &mut self.editor_left,
                    EditorId::Right => &mut self.editor_right,
                };

                editor.set_wrap_enabled(enabled);
                Task::none()
            }
            Message::ToggleSearchReplace(editor_id, enabled) => {
                self.log(
                    "INFO",
                    &format!(
                        "Search/Replace {} in {:?} editor",
                        if enabled { "enabled" } else { "disabled" },
                        editor_id
                    ),
                );

                match editor_id {
                    EditorId::Left => {
                        self.search_replace_enabled_left = enabled
                    }
                    EditorId::Right => {
                        self.search_replace_enabled_right = enabled
                    }
                }

                let editor = match editor_id {
                    EditorId::Left => &mut self.editor_left,
                    EditorId::Right => &mut self.editor_right,
                };

                editor.set_search_replace_enabled(enabled);
                Task::none()
            }
            Message::ToggleLineNumbers(editor_id, enabled) => {
                self.log(
                    "INFO",
                    &format!(
                        "Line numbers {} in {:?} editor",
                        if enabled { "enabled" } else { "disabled" },
                        editor_id
                    ),
                );

                match editor_id {
                    EditorId::Left => self.line_numbers_enabled_left = enabled,
                    EditorId::Right => {
                        self.line_numbers_enabled_right = enabled
                    }
                }

                let editor = match editor_id {
                    EditorId::Left => &mut self.editor_left,
                    EditorId::Right => &mut self.editor_right,
                };

                editor.set_line_numbers_enabled(enabled);
                Task::none()
            }
            Message::TextInputChanged(value) => {
                self.text_input_value = value;
                // Immediately lose focus to prevent race condition with keyboard events
                self.editor_left.lose_focus();
                self.editor_right.lose_focus();
                Task::none()
            }
            Message::TextInputClicked => {
                self.editor_left.lose_focus();
                self.editor_right.lose_focus();
                Task::none()
            }
        }
    }

    /// Subscription for periodic updates.
    fn subscription(_state: &Self) -> Subscription<Message> {
        // Cursor blink
        window::frames().map(|_| Message::Tick)
    }

    /// Returns the current theme for the application.
    fn theme(&self) -> Theme {
        self.current_theme.clone()
    }

    /// Renders the user interface.
    fn view(&self) -> Element<'_, Message> {
        let palette = self.current_theme.extended_palette();
        let text_color = palette.background.base.text;

        // Toolbar
        let toolbar = row![
            button(text("Open")).on_press(Message::OpenFile),
            button(text("Save")).on_press(Message::SaveFile),
            button(text("Save As")).on_press(Message::SaveFileAs),
            button(text("Run")).on_press(Message::RunCode),
            text(self.file_status())
                .style(move |_| text::Style { color: Some(text_color) }),
            Space::new().width(Length::Fill),
            mouse_area(
                text_input(
                    "Input for testing focus ...",
                    &self.text_input_value
                )
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
        let error_bar = if let Some(err) = &self.error_message {
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
            PaneGrid::new(&self.panes, |_id, pane, _is_maximized| {
                let title_bar_style = palette.background.weak.color;

                match pane {
                    PaneType::EditorLeft => {
                        let title = TitleBar::new(text("Editor (Left)").style(
                            move |_| text::Style { color: Some(text_color) },
                        ))
                        .style(move |_| container::Style {
                            background: Some(iced::Background::Color(
                                title_bar_style,
                            )),
                            ..Default::default()
                        })
                        .padding(5);

                        Content::new(
                            self.view_editor_pane(EditorId::Left, text_color),
                        )
                        .title_bar(title)
                        .style(move |_| {
                            container::Style {
                                background: Some(iced::Background::Color(
                                    palette.background.base.color,
                                )),
                                ..Default::default()
                            }
                        })
                    }
                    PaneType::EditorRight => {
                        let title = TitleBar::new(
                            text("Editor (Right)").style(move |_| {
                                text::Style { color: Some(text_color) }
                            }),
                        )
                        .style(move |_| container::Style {
                            background: Some(iced::Background::Color(
                                title_bar_style,
                            )),
                            ..Default::default()
                        })
                        .padding(5);

                        Content::new(
                            self.view_editor_pane(EditorId::Right, text_color),
                        )
                        .title_bar(title)
                        .style(move |_| {
                            container::Style {
                                background: Some(iced::Background::Color(
                                    palette.background.base.color,
                                )),
                                ..Default::default()
                            }
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
            text("Output")
                .style(move |_| text::Style { color: Some(text_color) }),
        )
        .padding(5)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(title_bar_style)),
            ..Default::default()
        });

        let output_content = container(self.view_output_pane(text_color))
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

        if self.show_settings {
            let modal = center(
                container(
                    column![
                        text("Settings").size(24),
                        row![
                            text("Font:").width(80),
                            pick_list(
                                FontOption::ALL,
                                Some(self.current_font),
                                Message::FontChanged
                            )
                        ]
                        .spacing(10)
                        .align_y(iced::Center),
                        row![
                            text(format!("Size: {:.0}", self.current_font_size)).width(80),
                            slider(
                                10.0..=30.0,
                                self.current_font_size,
                                Message::FontSizeChanged
                            )
                            .width(150)
                        ]
                        .spacing(10)
                        .align_y(iced::Center),
                        checkbox(self.auto_adjust_line_height)
                            .label("Auto-adjust Line Height")
                            .on_toggle(Message::ToggleAutoLineHeight),
                        row![
                            text(format!("Height: {:.1}", self.current_line_height)).width(80),
                            slider(
                                10.0..=50.0,
                                self.current_line_height,
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
                                Some(LanguageOption::from(self.current_language)),
                                Message::LanguageChanged
                            )
                        ]
                        .spacing(10)
                        .align_y(iced::Center),
                        row![
                            text("Theme:").width(80),
                            pick_list(
                                Theme::ALL,
                                Some(self.current_theme.clone()),
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
                    background: Some(iced::Background::Color(palette.background.weak.color)),
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
                    container(Space::new().width(Length::Fill).height(Length::Fill)).style(
                        |_| container::Style {
                            background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
                            ..Default::default()
                        }
                    )
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
    fn view_editor_pane(
        &self,
        editor_id: EditorId,
        _text_color: Color,
    ) -> Element<'_, Message> {
        // Select data based on editor_id
        let (editor, search_replace_enabled, line_numbers_enabled) =
            match editor_id {
                EditorId::Left => (
                    &self.editor_left,
                    self.search_replace_enabled_left,
                    self.line_numbers_enabled_left,
                ),
                EditorId::Right => (
                    &self.editor_right,
                    self.search_replace_enabled_right,
                    self.line_numbers_enabled_right,
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
    fn view_output_pane(&self, text_color: Color) -> Element<'_, Message> {
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
        let log_content: Vec<Element<'_, Message>> = self
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
    fn file_status(&self) -> String {
        let left_name = self
            .current_file_left
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("New file");
        let right_name = self
            .current_file_right
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("New file");

        let left_mod = if self.editor_left.is_modified() { "*" } else { "" };
        let right_mod = if self.editor_right.is_modified() { "*" } else { "" };

        let active_left =
            if self.active_editor == EditorId::Left { "● " } else { "" };
        let active_right =
            if self.active_editor == EditorId::Right { " ●" } else { "" };

        format!(
            "{}L: {}{} | R: {}{}{}",
            active_left,
            left_name,
            left_mod,
            right_name,
            right_mod,
            active_right
        )
    }
}

/// Opens a file dialog.
async fn open_file_dialog() -> Result<(PathBuf, String), String> {
    let file = rfd::AsyncFileDialog::new()
        .add_filter("Lua Files", &["lua"])
        .add_filter("All Files", &["*"])
        .set_title("Open Lua File")
        .pick_file()
        .await;

    if let Some(file) = file {
        let path = file.path().to_path_buf();
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Unable to read file: {}", e))?;
        Ok((path, content))
    } else {
        Err("No file selected".to_string())
    }
}

/// Saves content to a file.
async fn save_file(path: PathBuf, content: String) -> Result<PathBuf, String> {
    std::fs::write(&path, content)
        .map_err(|e| format!("Unable to write file: {}", e))?;
    Ok(path)
}

/// Opens a save-as dialog.
async fn save_file_as_dialog(content: String) -> Result<PathBuf, String> {
    let file = rfd::AsyncFileDialog::new()
        .add_filter("Lua Files", &["lua"])
        .set_title("Save As")
        .save_file()
        .await;

    if let Some(file) = file {
        let path = file.path().to_path_buf();
        std::fs::write(&path, content)
            .map_err(|e| format!("Unable to write file: {}", e))?;
        Ok(path)
    } else {
        Err("Save cancelled".to_string())
    }
}