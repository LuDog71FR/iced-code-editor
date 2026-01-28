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

    /// Returns a mutable reference to the active editor.
    ///
    /// # Returns
    ///
    /// A mutable reference to the currently active `CodeEditor`.
    fn get_active_editor(&mut self) -> &mut CodeEditor {
        match self.active_editor {
            EditorId::Left => &mut self.editor_left,
            EditorId::Right => &mut self.editor_right,
        }
    }

    /// Returns a mutable reference to the active editor and its associated file path.
    ///
    /// # Returns
    ///
    /// A tuple containing:
    /// - A mutable reference to the currently active `CodeEditor`
    /// - A mutable reference to its associated file path `Option<PathBuf>`
    fn get_active_editor_and_file(
        &mut self,
    ) -> (&mut CodeEditor, &mut Option<PathBuf>) {
        match self.active_editor {
            EditorId::Left => {
                (&mut self.editor_left, &mut self.current_file_left)
            }
            EditorId::Right => {
                (&mut self.editor_right, &mut self.current_file_right)
            }
        }
    }

    /// Returns a mutable reference to the specified editor.
    ///
    /// # Arguments
    ///
    /// * `id` - The identifier of the editor to retrieve
    ///
    /// # Returns
    ///
    /// A mutable reference to the specified `CodeEditor`.
    fn get_editor(&mut self, id: EditorId) -> &mut CodeEditor {
        match id {
            EditorId::Left => &mut self.editor_left,
            EditorId::Right => &mut self.editor_right,
        }
    }

    /// Returns a mutable reference to the specified editor and its associated file path.
    ///
    /// # Arguments
    ///
    /// * `id` - The identifier of the editor to retrieve
    ///
    /// # Returns
    ///
    /// A tuple containing:
    /// - A mutable reference to the specified `CodeEditor`
    /// - A mutable reference to its associated file path `Option<PathBuf>`
    fn get_editor_and_file(
        &mut self,
        id: EditorId,
    ) -> (&mut CodeEditor, &mut Option<PathBuf>) {
        match id {
            EditorId::Left => {
                (&mut self.editor_left, &mut self.current_file_left)
            }
            EditorId::Right => {
                (&mut self.editor_right, &mut self.current_file_right)
            }
        }
    }

    /// Handles the file open request by displaying a file picker dialog.
    ///
    /// # Returns
    ///
    /// A `Task` that will produce a `Message::FileOpened` with the file contents.
    fn handle_file_open(&mut self) -> Task<Message> {
        self.log(
            "INFO",
            &format!("Opening file for {:?} editor...", self.active_editor),
        );
        Task::perform(file_ops::open_file_dialog(), Message::FileOpened)
    }

    /// Handles the result of a file open operation.
    ///
    /// # Arguments
    ///
    /// * `result` - Result containing the file path and contents, or an error message
    ///
    /// # Returns
    ///
    /// A `Task` that will reset the editor with the new content.
    fn handle_file_opened(
        &mut self,
        result: Result<(PathBuf, String), String>,
    ) -> Task<Message> {
        match result {
            Ok((path, content)) => {
                self.log(
                    "INFO",
                    &format!(
                        "Opened {} in {:?} editor",
                        path.display(),
                        self.active_editor
                    ),
                );

                let style = theme::from_iced_theme(&self.current_theme);
                let (editor, current_file) = self.get_active_editor_and_file();

                let task = editor.reset(&content);
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
        }
    }

    /// Handles saving the current file to disk.
    ///
    /// If the active editor has an associated file path, saves to that path.
    /// Otherwise, delegates to `handle_file_save_as` to prompt for a path.
    ///
    /// # Returns
    ///
    /// A `Task` that will perform the save operation.
    fn handle_file_save(&mut self) -> Task<Message> {
        let current_file = match self.active_editor {
            EditorId::Left => self.current_file_left.clone(),
            EditorId::Right => self.current_file_right.clone(),
        };
        if let Some(path) = current_file {
            self.log("INFO", &format!("Saving to: {}", path.display()));
            let editor = self.get_active_editor();
            let content = editor.content();
            Task::perform(
                file_ops::save_file(path, content),
                Message::FileSaved,
            )
        } else {
            self.update(Message::SaveFileAs)
        }
    }

    /// Handles the "Save As" operation by displaying a file save dialog.
    ///
    /// # Returns
    ///
    /// A `Task` that will perform the save operation with user-selected path.
    fn handle_file_save_as(&mut self) -> Task<Message> {
        self.log("INFO", "Opening save dialog...");
        let editor = self.get_active_editor();
        let content = editor.content();
        Task::perform(
            file_ops::save_file_as_dialog(content),
            Message::FileSaved,
        )
    }

    /// Handles the result of a file save operation.
    ///
    /// # Arguments
    ///
    /// * `result` - Result containing the saved file path, or an error message
    ///
    /// # Returns
    ///
    /// Always returns `Task::none()`.
    fn handle_file_saved(
        &mut self,
        result: Result<PathBuf, String>,
    ) -> Task<Message> {
        match result {
            Ok(path) => {
                self.log("INFO", &format!("Saved: {}", path.display()));
                let (editor, current_file) = self.get_active_editor_and_file();
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

    /// Handles font changes by updating both editors.
    ///
    /// # Arguments
    ///
    /// * `font_option` - The new font to apply
    ///
    /// # Returns
    ///
    /// Always returns `Task::none()`.
    fn handle_font_changed(
        &mut self,
        font_option: FontOption,
    ) -> Task<Message> {
        self.log("INFO", &format!("Font changed to: {}", font_option.name));
        self.current_font = font_option;

        let font = font_option.font();
        self.editor_left.set_font(font);
        self.editor_right.set_font(font);
        Task::none()
    }

    /// Handles font size changes by updating both editors.
    ///
    /// If auto-adjust line height is enabled, also updates the line height proportionally.
    ///
    /// # Arguments
    ///
    /// * `size` - The new font size in pixels
    ///
    /// # Returns
    ///
    /// Always returns `Task::none()`.
    fn handle_font_size_changed(&mut self, size: f32) -> Task<Message> {
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

    /// Handles line height changes by updating both editors.
    ///
    /// # Arguments
    ///
    /// * `height` - The new line height in pixels
    ///
    /// # Returns
    ///
    /// Always returns `Task::none()`.
    fn handle_line_height_changed(&mut self, height: f32) -> Task<Message> {
        self.current_line_height = height;
        self.editor_left.set_line_height(height);
        self.editor_right.set_line_height(height);
        Task::none()
    }

    /// Handles UI language changes by updating both editors.
    ///
    /// # Arguments
    ///
    /// * `lang_option` - The new language to apply
    ///
    /// # Returns
    ///
    /// Always returns `Task::none()`.
    fn handle_language_changed(
        &mut self,
        lang_option: LanguageOption,
    ) -> Task<Message> {
        let new_language = lang_option.inner();
        self.log("INFO", &format!("UI Language changed to: {}", lang_option));
        self.current_language = new_language;
        self.editor_left.set_language(new_language);
        self.editor_right.set_language(new_language);
        Task::none()
    }

    /// Handles theme changes by updating both editors.
    ///
    /// # Arguments
    ///
    /// * `new_theme` - The new theme to apply
    ///
    /// # Returns
    ///
    /// Always returns `Task::none()`.
    fn handle_theme_changed(&mut self, new_theme: Theme) -> Task<Message> {
        self.log("INFO", &format!("Theme changed to: {:?}", new_theme));
        let style = theme::from_iced_theme(&new_theme);
        self.current_theme = new_theme;
        self.editor_left.set_theme(style);
        self.editor_right.set_theme(style);
        Task::none()
    }

    /// Handles toggling line wrapping for a specific editor.
    ///
    /// # Arguments
    ///
    /// * `editor_id` - The editor to toggle line wrapping for
    /// * `enabled` - Whether line wrapping should be enabled or disabled
    ///
    /// # Returns
    ///
    /// Always returns `Task::none()`.
    fn handle_toggle_wrap(
        &mut self,
        editor_id: EditorId,
        enabled: bool,
    ) -> Task<Message> {
        self.log(
            "INFO",
            &format!(
                "Line wrapping {} in {:?} editor",
                if enabled { "enabled" } else { "disabled" },
                editor_id
            ),
        );

        let editor = self.get_editor(editor_id);
        editor.set_wrap_enabled(enabled);
        Task::none()
    }

    /// Handles toggling search/replace functionality for a specific editor.
    ///
    /// # Arguments
    ///
    /// * `editor_id` - The editor to toggle search/replace for
    /// * `enabled` - Whether search/replace should be enabled or disabled
    ///
    /// # Returns
    ///
    /// Always returns `Task::none()`.
    fn handle_toggle_search_replace(
        &mut self,
        editor_id: EditorId,
        enabled: bool,
    ) -> Task<Message> {
        self.log(
            "INFO",
            &format!(
                "Search/Replace {} in {:?} editor",
                if enabled { "enabled" } else { "disabled" },
                editor_id
            ),
        );

        match editor_id {
            EditorId::Left => self.search_replace_enabled_left = enabled,
            EditorId::Right => self.search_replace_enabled_right = enabled,
        }

        let editor = self.get_editor(editor_id);
        editor.set_search_replace_enabled(enabled);
        Task::none()
    }

    /// Handles toggling line numbers for a specific editor.
    ///
    /// # Arguments
    ///
    /// * `editor_id` - The editor to toggle line numbers for
    /// * `enabled` - Whether line numbers should be enabled or disabled
    ///
    /// # Returns
    ///
    /// Always returns `Task::none()`.
    fn handle_toggle_line_numbers(
        &mut self,
        editor_id: EditorId,
        enabled: bool,
    ) -> Task<Message> {
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
            EditorId::Right => self.line_numbers_enabled_right = enabled,
        }

        let editor = self.get_editor(editor_id);
        editor.set_line_numbers_enabled(enabled);
        Task::none()
    }

    /// Handles editor-specific events by forwarding them to the appropriate editor.
    ///
    /// # Arguments
    ///
    /// * `editor_id` - The editor that generated the event
    /// * `event` - The editor event to handle
    ///
    /// # Returns
    ///
    /// A `Task` that may produce additional `Message::EditorEvent` messages.
    fn handle_editor_event(
        &mut self,
        editor_id: EditorId,
        event: &EditorMessage,
    ) -> Task<Message> {
        let editor = self.get_editor(editor_id);
        editor.update(event).map(move |e| Message::EditorEvent(editor_id, e))
    }

    /// Handles periodic tick events for cursor blinking in both editors.
    ///
    /// # Returns
    ///
    /// A batched `Task` containing tick updates for both editors.
    fn handle_tick(&mut self) -> Task<Message> {
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

    /// Handles loading a code template into a specific editor.
    ///
    /// # Arguments
    ///
    /// * `editor_id` - The editor to load the template into
    /// * `template` - The template to load
    ///
    /// # Returns
    ///
    /// A `Task` that will reset the editor with the template content.
    fn handle_template_selected(
        &mut self,
        editor_id: EditorId,
        template: Template,
    ) -> Task<Message> {
        self.log(
            "INFO",
            &format!(
                "Template '{}' loaded in {:?} editor",
                template.name(),
                editor_id
            ),
        );

        let style = theme::from_iced_theme(&self.current_theme);
        let (editor, current_file) = self.get_editor_and_file(editor_id);

        let task = editor.reset(template.content());
        editor.set_theme(style);
        *current_file = None;

        task.map(move |e| Message::EditorEvent(editor_id, e))
    }

    /// Handles code execution simulation for the active editor.
    ///
    /// This is a simulated execution that counts lines and logs output.
    ///
    /// # Returns
    ///
    /// Always returns `Task::none()`.
    fn handle_run_code(&mut self) -> Task<Message> {
        self.log(
            "INFO",
            &format!("Running code from {:?} editor...", self.active_editor),
        );

        let editor = self.get_active_editor();
        let line_count = editor.content().lines().count();
        self.log("OUTPUT", &format!("Script has {} lines", line_count));
        self.log("OUTPUT", "Execution completed (simulated)");
        Task::none()
    }

    /// Handles changes to the text input field.
    ///
    /// When the text input changes, both editors lose focus to prevent
    /// race conditions with keyboard events.
    ///
    /// # Arguments
    ///
    /// * `value` - The new text input value
    ///
    /// # Returns
    ///
    /// Always returns `Task::none()`.
    fn handle_text_input_changed(&mut self, value: String) -> Task<Message> {
        self.text_input_value = value;
        // Immediately lose focus to prevent race condition with keyboard events
        self.editor_left.lose_focus();
        self.editor_right.lose_focus();
        Task::none()
    }

    /// Handles clicks on the text input field.
    ///
    /// When the text input is clicked, both editors lose focus.
    ///
    /// # Returns
    ///
    /// Always returns `Task::none()`.
    fn handle_text_input_clicked(&mut self) -> Task<Message> {
        self.editor_left.lose_focus();
        self.editor_right.lose_focus();
        Task::none()
    }

    /// Handles messages and updates the application state.
    ///
    /// This is the main message dispatcher that routes messages to appropriate handler methods.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to handle
    ///
    /// # Returns
    ///
    /// A `Task` that may produce additional messages.
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
            Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
                Task::none()
            }
            Message::ClearLog => {
                self.log_messages.clear();
                self.log("INFO", "Log cleared");
                Task::none()
            }
            // File operations
            Message::OpenFile => self.handle_file_open(),
            Message::FileOpened(result) => self.handle_file_opened(result),
            Message::SaveFile => self.handle_file_save(),
            Message::SaveFileAs => self.handle_file_save_as(),
            Message::FileSaved(result) => self.handle_file_saved(result),
            // Editor configuration
            Message::FontChanged(font_option) => {
                self.handle_font_changed(font_option)
            }
            Message::FontSizeChanged(size) => {
                self.handle_font_size_changed(size)
            }
            Message::LineHeightChanged(height) => {
                self.handle_line_height_changed(height)
            }
            Message::LanguageChanged(lang_option) => {
                self.handle_language_changed(lang_option)
            }
            Message::ThemeChanged(new_theme) => {
                self.handle_theme_changed(new_theme)
            }
            // Editor toggles
            Message::ToggleWrap(editor_id, enabled) => {
                self.handle_toggle_wrap(editor_id, enabled)
            }
            Message::ToggleSearchReplace(editor_id, enabled) => {
                self.handle_toggle_search_replace(editor_id, enabled)
            }
            Message::ToggleLineNumbers(editor_id, enabled) => {
                self.handle_toggle_line_numbers(editor_id, enabled)
            }
            // Editor events
            Message::EditorEvent(editor_id, event) => {
                self.handle_editor_event(editor_id, &event)
            }
            Message::Tick => self.handle_tick(),
            // Templates and execution
            Message::TemplateSelected(editor_id, template) => {
                self.handle_template_selected(editor_id, template)
            }
            Message::RunCode => self.handle_run_code(),
            // Text input
            Message::TextInputChanged(value) => {
                self.handle_text_input_changed(value)
            }
            Message::TextInputClicked => self.handle_text_input_clicked(),
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
