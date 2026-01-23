use crate::file_ops;
use crate::types::{EditorId, FontOption, LanguageOption, PaneType, Template};
use iced::widget::pane_grid;
use iced::{Subscription, Task, Theme, window};
use iced_code_editor::Message as EditorMessage;
use iced_code_editor::{CodeEditor, Language, theme};
use std::path::PathBuf;

/// Demo application state.
pub struct DemoApp {
    /// Left code editor
    pub editor_left: CodeEditor,
    /// Right code editor
    pub editor_right: CodeEditor,
    /// Current file path for left editor
    pub current_file_left: Option<PathBuf>,
    /// Current file path for right editor
    pub current_file_right: Option<PathBuf>,
    /// Error message
    pub error_message: Option<String>,
    /// Current theme
    pub current_theme: Theme,
    /// Current UI language
    pub current_language: Language,
    /// Current font
    pub current_font: FontOption,
    /// Current font size
    pub current_font_size: f32,
    /// Current line height
    pub current_line_height: f32,
    /// Pane grid state
    pub panes: pane_grid::State<PaneType>,
    /// Log messages for output pane
    pub log_messages: Vec<String>,
    /// Search/replace enabled flag for left editor
    pub search_replace_enabled_left: bool,
    /// Search/replace enabled flag for right editor
    pub search_replace_enabled_right: bool,
    /// Line numbers enabled flag for left editor
    pub line_numbers_enabled_left: bool,
    /// Line numbers enabled flag for right editor
    pub line_numbers_enabled_right: bool,
    /// Active editor (receives Open/Save/Run commands)
    pub active_editor: EditorId,
    /// Test text input value
    pub text_input_value: String,
    /// Whether to show the settings modal
    pub show_settings: bool,
    /// Whether to automatically adjust line height when font size changes
    pub auto_adjust_line_height: bool,
}

/// Application messages.
#[derive(Debug, Clone)]
pub enum Message {
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
    pub fn new() -> (Self, Task<Message>) {
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
    pub fn update(&mut self, message: Message) -> Task<Message> {
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
                Task::perform(file_ops::open_file_dialog(), Message::FileOpened)
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
                    Task::perform(
                        file_ops::save_file(path, content),
                        Message::FileSaved,
                    )
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
                Task::perform(
                    file_ops::save_file_as_dialog(content),
                    Message::FileSaved,
                )
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

                self.editor_left
                    .set_font_size(size, self.auto_adjust_line_height);
                self.editor_right
                    .set_font_size(size, self.auto_adjust_line_height);
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
    pub fn subscription(_state: &Self) -> Subscription<Message> {
        // Cursor blink
        window::frames().map(|_| Message::Tick)
    }

    /// Returns the current theme for the application.
    pub fn theme(&self) -> Theme {
        self.current_theme.clone()
    }
}
