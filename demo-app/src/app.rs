use crate::file_ops;
use crate::types::{EditorId, FontOption, LanguageOption, Template};
#[cfg(not(target_arch = "wasm32"))]
use iced::mouse;
#[cfg(not(target_arch = "wasm32"))]
use iced::widget::Id;
#[cfg(not(target_arch = "wasm32"))]
use iced::widget::operation::focus;
use iced::{Event, Subscription, Task, Theme, event, window};
#[cfg(not(target_arch = "wasm32"))]
use iced_code_editor::LspEvent;
#[cfg(not(target_arch = "wasm32"))]
use iced_code_editor::LspOverlayState;
#[cfg(not(target_arch = "wasm32"))]
use iced_code_editor::LspPosition;
use iced_code_editor::Message as EditorMessage;
use iced_code_editor::{
    CodeEditor, ContextMenuEntry, IndentStyle, Language, theme,
};
#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashMap;
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

#[cfg(not(target_arch = "wasm32"))]
mod app_lsp;
#[cfg(not(target_arch = "wasm32"))]
use app_lsp::LspHoverPending;

/// Delay in milliseconds before hiding the hover tooltip when the cursor leaves the window.
#[cfg(not(target_arch = "wasm32"))]
const LSP_HOVER_CURSOR_LEFT_MS: u64 = 400;
/// Delay in milliseconds before hiding the hover tooltip after the mouse exits the tooltip.
#[cfg(not(target_arch = "wasm32"))]
const LSP_HOVER_TOOLTIP_EXIT_MS: u64 = 300;
/// Delay in milliseconds before hiding the hover tooltip after the cursor leaves the editor.
#[cfg(not(target_arch = "wasm32"))]
const LSP_HOVER_HIDE_DELAY_MS: u64 = 500;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
pub struct LspProgress {
    pub title: String,
    pub message: Option<String>,
    pub percentage: Option<u32>,
}

pub struct EditorTab {
    pub id: EditorId,
    pub editor: CodeEditor,
    pub file_path: Option<PathBuf>,
    pub is_dirty: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pub lsp_server_key: Option<&'static str>,
}

/// Demo application state.
pub struct DemoApp {
    /// Tabs
    pub tabs: Vec<EditorTab>,
    /// Active tab ID
    pub active_tab_id: EditorId,
    /// Next available tab ID
    pub next_tab_id: usize,
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
    /// Log messages for output pane
    pub log_messages: Vec<String>,
    /// Test text input value
    pub text_input_value: String,
    /// Whether to show the settings modal
    pub show_settings: bool,
    /// Whether to automatically adjust line height when font size changes
    pub auto_adjust_line_height: bool,
    #[cfg(not(target_arch = "wasm32"))]
    lsp_events: Option<mpsc::Receiver<LspEvent>>,
    #[cfg(not(target_arch = "wasm32"))]
    lsp_event_sender: Option<mpsc::Sender<LspEvent>>,
    /// Aggregated LSP overlay display state (hover + completion).
    #[cfg(not(target_arch = "wasm32"))]
    pub lsp_overlay: LspOverlayState,
    #[cfg(not(target_arch = "wasm32"))]
    lsp_applying_completion: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pub lsp_hover_anchor: Option<(EditorId, LspPosition)>,
    #[cfg(not(target_arch = "wasm32"))]
    pub lsp_overlay_editor: Option<EditorId>,
    #[cfg(not(target_arch = "wasm32"))]
    lsp_hover_pending: Option<LspHoverPending>,
    #[cfg(not(target_arch = "wasm32"))]
    pub lsp_hover_hide_deadline: Option<Instant>,
    #[cfg(not(target_arch = "wasm32"))]
    pub lsp_progress: HashMap<String, HashMap<String, LspProgress>>,
    /// Current window width
    pub window_width: f32,
    /// Whether tabs are overflowing the window width
    pub tabs_overflow: bool,
    /// Spinner animation frame (0-7)
    pub spinner_frame: usize,
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
    /// Editor mouse entered
    EditorMouseEntered(EditorId),
    /// Editor mouse exited
    EditorMouseExited(EditorId),
    /// Open file
    OpenFile,
    /// File opened
    FileOpened(Result<(PathBuf, String), String>),
    /// Save file
    SaveFile,
    /// Save file as
    SaveFileAs,
    /// File saved
    FileSaved(EditorId, Result<PathBuf, String>),
    /// File revealed in the platform file manager
    #[cfg(not(target_arch = "wasm32"))]
    FileRevealed(Result<PathBuf, String>),
    /// Cursor blink tick
    Tick,
    /// Window-level events
    WindowEvent(Event),
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
    /// Template selected
    TemplateSelected(EditorId, Template),
    /// Clear log
    ClearLog,
    /// Run code (simulated)
    RunCode,
    /// Toggle line wrapping
    ToggleWrap(EditorId, bool),
    /// Toggle code folding
    ToggleFolding(EditorId, bool),
    /// Toggle auto-indentation
    ToggleAutoIndent(EditorId, bool),
    /// Toggle auto-closing of brackets/quotes
    ToggleAutoCloseBrackets(EditorId, bool),
    /// Change indentation style
    IndentStyleChanged(EditorId, IndentStyle),
    /// Toggle search/replace
    ToggleSearchReplace(EditorId, bool),
    /// Toggle line numbers
    ToggleLineNumbers(EditorId, bool),
    /// Toggle visible whitespace rendering
    ToggleShowWhitespace(EditorId, bool),
    /// Toggle Vim behavior
    ToggleVim(EditorId, bool),
    /// Toggle LSP support
    ToggleLsp(EditorId, bool),
    /// Test text input changed
    TextInputChanged(String),
    /// Test text input clicked
    TextInputClicked,
    /// Close a tab
    CloseTab(EditorId),
    /// Select a tab
    SelectTab(EditorId),
    /// New empty tab
    NewTab,
    #[cfg(not(target_arch = "wasm32"))]
    LspOverlay(iced_code_editor::LspOverlayMessage),
    #[cfg(not(target_arch = "wasm32"))]
    JumpToFile(PathBuf, usize, usize),
    #[cfg(not(target_arch = "wasm32"))]
    FileOpenedAndJump(Result<(PathBuf, String, usize, usize), String>),
}

impl DemoApp {
    /// Creates a new instance of the application.
    pub fn new() -> (Self, Task<Message>) {
        let default_content = r#"-- Lua code editor demo
-- This demo tests tabs with CodeEditor

function greet(name)
    print("Hello, " .. name .. "!")
end

greet("World")
"#;

        let log_messages = vec!["[INFO] Application started".to_string()];

        let current_font = if cfg!(target_arch = "wasm32") {
            FontOption::JETBRAINS_MONO
        } else {
            FontOption::MONOSPACE
        };

        let mut editor = Self::new_editor(default_content);
        let font = current_font.font();
        editor.set_font(font);

        // Initial tab
        let tab_id = EditorId(0);
        let tab = EditorTab {
            id: tab_id,
            editor,
            file_path: None,
            is_dirty: false,
            #[cfg(not(target_arch = "wasm32"))]
            lsp_server_key: None,
        };

        let tabs = vec![tab];
        let active_tab_id = tab_id;
        let next_tab_id = 1;

        let startup_task = Task::none();
        #[cfg(not(target_arch = "wasm32"))]
        let (lsp_event_sender, lsp_events) = {
            let (event_tx, event_rx) = mpsc::channel();
            (Some(event_tx), Some(event_rx))
        };

        let app = Self {
            tabs,
            active_tab_id,
            next_tab_id,
            error_message: None,
            current_theme: Theme::TokyoNightStorm,
            current_language: Language::English,
            current_font,
            current_font_size: 14.0,
            current_line_height: 20.0,
            log_messages,
            text_input_value: String::new(),
            show_settings: false,
            auto_adjust_line_height: true,
            #[cfg(not(target_arch = "wasm32"))]
            lsp_events,
            #[cfg(not(target_arch = "wasm32"))]
            lsp_event_sender,
            #[cfg(not(target_arch = "wasm32"))]
            lsp_overlay: LspOverlayState::new(),
            #[cfg(not(target_arch = "wasm32"))]
            lsp_applying_completion: false,
            #[cfg(not(target_arch = "wasm32"))]
            lsp_hover_anchor: None,
            #[cfg(not(target_arch = "wasm32"))]
            lsp_overlay_editor: None,
            #[cfg(not(target_arch = "wasm32"))]
            lsp_hover_pending: None,
            #[cfg(not(target_arch = "wasm32"))]
            lsp_hover_hide_deadline: None,
            #[cfg(not(target_arch = "wasm32"))]
            lsp_progress: HashMap::new(),
            window_width: 1024.0,
            tabs_overflow: false,
            spinner_frame: 0,
        };

        #[cfg(not(target_arch = "wasm32"))]
        let mut app = app;
        #[cfg(target_arch = "wasm32")]
        let app = app;

        #[cfg(not(target_arch = "wasm32"))]
        {
            let root_dir = std::env::current_dir().ok();
            if let Some(root_dir) = root_dir {
                let path = root_dir.join("demo.lua");
                app.sync_lsp_for_path(active_tab_id, &path);
            } else {
                app.log("ERROR", "LSP failed: cwd unavailable");
            }
        }

        (app, startup_task)
    }

    /// Adds a log message.
    fn log(&mut self, level: &str, message: &str) {
        self.log_messages.push(format!("[{}] {}", level, message));
    }

    /// Creates an editor with the demo context-menu entries.
    fn new_editor(content: &str) -> CodeEditor {
        CodeEditor::new(content, "lua")
            .with_custom_context_menu_entries(vec![
                ContextMenuEntry::item(
                    "app.format_document",
                    "Format document",
                )
                .with_shortcut("Ctrl+Shift+F"),
                ContextMenuEntry::separator(),
                ContextMenuEntry::item("app.rename_symbol", "Rename symbol")
                    .with_enabled(false),
            ])
            .with_default_context_menu_enabled(true)
    }

    pub fn get_active_tab(&mut self) -> Option<&mut EditorTab> {
        let id = self.active_tab_id;
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub fn get_tab(&mut self, id: EditorId) -> Option<&mut EditorTab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    /// Returns a mutable reference to the active editor.
    fn get_active_editor(&mut self) -> Option<&mut CodeEditor> {
        self.get_active_tab().map(|tab| &mut tab.editor)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn get_editor(&mut self, id: EditorId) -> Option<&mut CodeEditor> {
        self.get_tab(id).map(|tab| &mut tab.editor)
    }

    /// Returns a mutable reference to the specified editor and its associated file path.
    fn get_editor_and_file(
        &mut self,
        id: EditorId,
    ) -> Option<(&mut CodeEditor, &mut Option<PathBuf>)> {
        self.get_tab(id).map(|tab| (&mut tab.editor, &mut tab.file_path))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn reveal_path_for_editor(
        &self,
        editor_id: EditorId,
    ) -> Result<PathBuf, String> {
        let tab = self.tabs.iter().find(|tab| tab.id == editor_id).ok_or_else(
            || "Editor tab not found for reveal request".to_string(),
        )?;

        tab.file_path.clone().ok_or_else(|| {
            "This tab does not have a file path to reveal".to_string()
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn handle_reveal_in_file_manager(
        &mut self,
        editor_id: EditorId,
    ) -> Task<Message> {
        match self.reveal_path_for_editor(editor_id) {
            Ok(path) => {
                self.log(
                    "INFO",
                    &format!("Revealing in file manager: {}", path.display()),
                );
                Task::perform(
                    file_ops::reveal_in_file_manager(path),
                    Message::FileRevealed,
                )
            }
            Err(error) => self.handle_file_revealed(Err(error)),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn handle_file_revealed(
        &mut self,
        result: Result<PathBuf, String>,
    ) -> Task<Message> {
        match result {
            Ok(path) => {
                self.log(
                    "INFO",
                    &format!("Revealed in file manager: {}", path.display()),
                );
                self.error_message = None;
            }
            Err(error) => {
                self.log("ERROR", &error);
                self.error_message = Some(error);
            }
        }
        Task::none()
    }

    /// Handles the file open request by displaying a file picker dialog.
    fn handle_file_open(&mut self) -> Task<Message> {
        self.log(
            "INFO",
            &format!("Opening file for {:?} editor...", self.active_tab_id),
        );
        Task::perform(file_ops::open_file_dialog(), Message::FileOpened)
    }

    /// Handles the result of a file open operation.
    fn handle_file_opened(
        &mut self,
        result: Result<(PathBuf, String), String>,
    ) -> Task<Message> {
        match result {
            Ok((path, content)) => {
                // Check if file is already open
                if let Some(tab) = self
                    .tabs
                    .iter()
                    .find(|t| t.file_path.as_ref() == Some(&path))
                {
                    self.active_tab_id = tab.id;
                    self.log(
                        "INFO",
                        &format!(
                            "Switched to existing tab for {}",
                            path.display()
                        ),
                    );
                    return Task::none();
                }

                // If current tab is empty (no file, no content), reuse it.
                // Otherwise create new tab.
                let active_tab_id = self.active_tab_id;
                let reuse_tab = self.get_active_tab().is_some_and(|tab| {
                    tab.file_path.is_none()
                        && tab.editor.content().trim().is_empty()
                        && !tab.is_dirty
                });

                let target_tab_id = if reuse_tab {
                    active_tab_id
                } else {
                    let new_id = EditorId(self.next_tab_id);
                    self.next_tab_id += 1;

                    let mut editor = Self::new_editor(&content); // Default language, will update
                    let font = self.current_font.font();
                    editor.set_font(font);
                    editor.set_font_size(
                        self.current_font_size,
                        self.auto_adjust_line_height,
                    );
                    editor.set_line_height(self.current_line_height);
                    editor
                        .set_theme(theme::from_iced_theme(&self.current_theme));
                    editor.set_language(self.current_language);
                    editor.set_reveal_in_file_manager_enabled(!cfg!(
                        target_arch = "wasm32"
                    ));

                    let tab = EditorTab {
                        id: new_id,
                        editor,
                        file_path: Some(path.clone()),
                        is_dirty: false,
                        #[cfg(not(target_arch = "wasm32"))]
                        lsp_server_key: None,
                    };
                    self.tabs.push(tab);
                    self.active_tab_id = new_id;
                    new_id
                };

                self.log(
                    "INFO",
                    &format!(
                        "Opened {} in {:?} editor",
                        path.display(),
                        target_tab_id
                    ),
                );

                let style = theme::from_iced_theme(&self.current_theme);
                let Some((editor, current_file)) =
                    self.get_editor_and_file(target_tab_id)
                else {
                    self.log("ERROR", "Target tab not found for opened file");
                    self.error_message = Some(
                        "Target tab not found for opened file".to_string(),
                    );
                    return Task::none();
                };

                let task = editor.reset(&content);
                editor.set_theme(style);
                editor.mark_saved();
                editor.set_reveal_in_file_manager_enabled(!cfg!(
                    target_arch = "wasm32"
                ));
                #[cfg(not(target_arch = "wasm32"))]
                let path_for_lsp = path.clone();
                *current_file = Some(path);

                // Update tab dirty state
                if let Some(tab) = self.get_tab(target_tab_id) {
                    tab.is_dirty = false;
                }

                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.sync_lsp_for_path(target_tab_id, &path_for_lsp);
                }
                self.error_message = None;

                self.check_tabs_overflow();
                task.map(move |e| Message::EditorEvent(target_tab_id, e))
            }
            Err(err) => {
                self.log("ERROR", &err);
                self.error_message = Some(err);
                Task::none()
            }
        }
    }

    /// Handles saving the current file to disk.
    fn handle_file_save(&mut self, editor_id: EditorId) -> Task<Message> {
        let tab_snapshot = self
            .tabs
            .iter()
            .find(|t| t.id == editor_id)
            .map(|tab| (tab.file_path.clone(), tab.editor.content()));
        let Some((file_path, content)) = tab_snapshot else {
            self.log("ERROR", "Editor tab not found for save");
            return Task::none();
        };

        if let Some(path) = file_path {
            self.log("INFO", &format!("Saving to: {}", path.display()));
            Task::perform(file_ops::save_file(path, content), move |result| {
                Message::FileSaved(editor_id, result)
            })
        } else {
            self.handle_file_save_as(editor_id)
        }
    }

    /// Handles the "Save As" operation by displaying a file save dialog.
    fn handle_file_save_as(&mut self, editor_id: EditorId) -> Task<Message> {
        self.log("INFO", "Opening save dialog...");
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == editor_id) else {
            self.log("ERROR", "Editor tab not found for save as");
            return Task::none();
        };
        let content = tab.editor.content();
        Task::perform(file_ops::save_file_as_dialog(content), move |result| {
            Message::FileSaved(editor_id, result)
        })
    }

    /// Handles the result of a file save operation.
    fn handle_file_saved(
        &mut self,
        editor_id: EditorId,
        result: Result<PathBuf, String>,
    ) -> Task<Message> {
        match result {
            Ok(path) => {
                self.log("INFO", &format!("Saved: {}", path.display()));
                let Some((editor, current_file)) =
                    self.get_editor_and_file(editor_id)
                else {
                    self.log("ERROR", "Editor tab missing on save");
                    self.error_message =
                        Some("Editor tab missing on save".to_string());
                    return Task::none();
                };
                *current_file = Some(path);
                editor.mark_saved();
                editor.set_reveal_in_file_manager_enabled(!cfg!(
                    target_arch = "wasm32"
                ));

                if let Some(tab) = self.get_tab(editor_id) {
                    tab.is_dirty = false;
                }

                self.error_message = None;
                self.check_tabs_overflow();
            }
            Err(err) => {
                self.log("ERROR", &err);
                self.error_message = Some(err);
            }
        }
        Task::none()
    }

    /// Handles font changes by updating all editors.
    fn handle_font_changed(
        &mut self,
        font_option: FontOption,
    ) -> Task<Message> {
        self.log("INFO", &format!("Font changed to: {}", font_option.name));
        self.current_font = font_option;

        let font = font_option.font();
        for tab in &mut self.tabs {
            tab.editor.set_font(font);
        }
        Task::none()
    }

    /// Handles font size changes by updating all editors.
    fn handle_font_size_changed(&mut self, size: f32) -> Task<Message> {
        self.current_font_size = size;

        if self.auto_adjust_line_height {
            let new_line_height = size * (20.0 / 14.0);
            self.current_line_height = new_line_height;
        }

        for tab in &mut self.tabs {
            tab.editor.set_font_size(size, self.auto_adjust_line_height);
        }
        Task::none()
    }

    /// Handles line height changes by updating all editors.
    fn handle_line_height_changed(&mut self, height: f32) -> Task<Message> {
        self.current_line_height = height;
        for tab in &mut self.tabs {
            tab.editor.set_line_height(height);
        }
        Task::none()
    }

    /// Handles UI language changes by updating all editors.
    fn handle_language_changed(
        &mut self,
        lang_option: LanguageOption,
    ) -> Task<Message> {
        let new_language = lang_option.inner();
        self.log("INFO", &format!("UI Language changed to: {}", lang_option));
        self.current_language = new_language;
        for tab in &mut self.tabs {
            tab.editor.set_language(new_language);
        }
        Task::none()
    }

    /// Handles theme changes by updating all editors.
    fn handle_theme_changed(&mut self, new_theme: Theme) -> Task<Message> {
        self.log("INFO", &format!("Theme changed to: {:?}", new_theme));
        let style = theme::from_iced_theme(&new_theme);
        self.current_theme = new_theme;
        for tab in &mut self.tabs {
            tab.editor.set_theme(style);
        }
        Task::none()
    }

    /// Handles changing the indentation style for a specific editor.
    fn handle_indent_style_changed(
        &mut self,
        editor_id: EditorId,
        style: IndentStyle,
    ) -> Task<Message> {
        self.log(
            "INFO",
            &format!(
                "Indent style changed to \"{style}\" in {editor_id:?} editor"
            ),
        );

        if let Some(tab) = self.get_tab(editor_id) {
            tab.editor.set_indent_style(style);
        }
        Task::none()
    }

    /// Handles toggling auto-indentation for a specific editor.
    fn handle_toggle_auto_indent(
        &mut self,
        editor_id: EditorId,
        enabled: bool,
    ) -> Task<Message> {
        self.log(
            "INFO",
            &format!(
                "Auto-indentation {} in {:?} editor",
                if enabled { "enabled" } else { "disabled" },
                editor_id
            ),
        );

        if let Some(tab) = self.get_tab(editor_id) {
            tab.editor.set_auto_indent_enabled(enabled);
        }
        Task::none()
    }

    /// Handles toggling auto-closing of brackets/quotes for a specific editor.
    fn handle_toggle_auto_close_brackets(
        &mut self,
        editor_id: EditorId,
        enabled: bool,
    ) -> Task<Message> {
        self.log(
            "INFO",
            &format!(
                "Auto-close brackets {} in {:?} editor",
                if enabled { "enabled" } else { "disabled" },
                editor_id
            ),
        );

        if let Some(tab) = self.get_tab(editor_id) {
            tab.editor.set_auto_close_brackets(enabled);
        }
        Task::none()
    }

    /// Handles toggling line wrapping for a specific editor.
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

        if let Some(tab) = self.get_tab(editor_id) {
            tab.editor.set_wrap_enabled(enabled);
        }
        Task::none()
    }

    /// Handles toggling code folding for a specific editor.
    fn handle_toggle_folding(
        &mut self,
        editor_id: EditorId,
        enabled: bool,
    ) -> Task<Message> {
        self.log(
            "INFO",
            &format!(
                "Code folding {} in {:?} editor",
                if enabled { "enabled" } else { "disabled" },
                editor_id
            ),
        );

        if let Some(tab) = self.get_tab(editor_id) {
            tab.editor.set_folding_enabled(enabled);
        }
        Task::none()
    }

    /// Handles toggling search/replace functionality for a specific editor.
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

        if let Some(tab) = self.get_tab(editor_id) {
            tab.editor.set_search_replace_enabled(enabled);
        }
        Task::none()
    }

    /// Handles toggling line numbers for a specific editor.
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

        if let Some(tab) = self.get_tab(editor_id) {
            tab.editor.set_line_numbers_enabled(enabled);
        }
        Task::none()
    }

    /// Handles toggling visible whitespace rendering for a specific editor.
    fn handle_toggle_show_whitespace(
        &mut self,
        editor_id: EditorId,
        enabled: bool,
    ) -> Task<Message> {
        if let Some(tab) = self.get_tab(editor_id) {
            tab.editor.set_show_whitespace(enabled);
        }
        Task::none()
    }

    /// Handles toggling Vim behavior for a specific editor.
    fn handle_toggle_vim(
        &mut self,
        editor_id: EditorId,
        enabled: bool,
    ) -> Task<Message> {
        if let Some(tab) = self.get_tab(editor_id) {
            tab.editor.set_vim_enabled(enabled);
        }
        Task::none()
    }

    /// Handles toggling LSP support for a specific editor.
    fn handle_toggle_lsp(
        &mut self,
        editor_id: EditorId,
        enabled: bool,
    ) -> Task<Message> {
        self.log(
            "INFO",
            &format!(
                "LSP {} in {:?} editor",
                if enabled { "enabled" } else { "disabled" },
                editor_id
            ),
        );

        if enabled {
            if let Some(tab) = self.get_tab(editor_id) {
                tab.editor.set_lsp_enabled(true);
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.sync_lsp_for_editor(editor_id);
            }
        } else {
            if let Some(tab) = self.get_tab(editor_id) {
                tab.editor.set_lsp_enabled(false);
            }
            #[cfg(not(target_arch = "wasm32"))]
            self.set_lsp_server_for_editor(editor_id, None);
        }
        Task::none()
    }

    /// Handles editor-specific events by forwarding them to the appropriate editor.
    fn handle_editor_event(
        &mut self,
        editor_id: EditorId,
        event: &EditorMessage,
    ) -> Task<Message> {
        if let EditorMessage::CustomContextMenuAction(id) = event {
            match id.as_str() {
                "app.format_document" => {
                    self.log("INFO", "Format document requested");
                }
                "app.rename_symbol" => {
                    self.log("INFO", "Rename symbol requested");
                }
                unknown => {
                    self.log(
                        "WARN",
                        &format!(
                            "Ignoring unknown context-menu action: {unknown}"
                        ),
                    );
                }
            }
            return Task::none();
        }

        if matches!(event, EditorMessage::RevealInFileManager) {
            #[cfg(not(target_arch = "wasm32"))]
            return self.handle_reveal_in_file_manager(editor_id);
            #[cfg(target_arch = "wasm32")]
            return Task::none();
        }

        if matches!(event, EditorMessage::WriteRequested) {
            return self.handle_file_save(editor_id);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Intercept Escape to close completion menu
            if matches!(event, EditorMessage::CloseSearch)
                && self.lsp_overlay.completion_visible
            {
                self.lsp_overlay.clear_completions();
                if !self.lsp_overlay.hover_visible {
                    self.lsp_overlay_editor = None;
                }
                return Task::none();
            }

            // Intercept keyboard events when completion menu is visible and should show
            if self.lsp_overlay.completion_visible
                && !self.lsp_overlay.completion_suppressed
                && !self.lsp_overlay.completion_items.is_empty()
            {
                match event {
                    EditorMessage::ArrowKey(direction, false) => {
                        use iced_code_editor::ArrowDirection;
                        match direction {
                            ArrowDirection::Up => {
                                return Task::done(Message::LspOverlay(
                                    iced_code_editor::LspOverlayMessage::CompletionNavigateUp,
                                ));
                            }
                            ArrowDirection::Down => {
                                return Task::done(Message::LspOverlay(
                                    iced_code_editor::LspOverlayMessage::CompletionNavigateDown,
                                ));
                            }
                            ArrowDirection::Left | ArrowDirection::Right => {
                                // Clear completion when navigating left/right away from word
                                self.lsp_overlay.clear_completions();
                                if !self.lsp_overlay.hover_visible {
                                    self.lsp_overlay_editor = None;
                                }
                            }
                        }
                    }
                    EditorMessage::Enter => {
                        return Task::done(Message::LspOverlay(
                            iced_code_editor::LspOverlayMessage::CompletionConfirm,
                        ));
                    }
                    _ => {}
                }
            }
        }

        let task = if let Some(tab) = self.get_tab(editor_id) {
            let task = tab
                .editor
                .update(event)
                .map(move |e| Message::EditorEvent(editor_id, e));

            tab.is_dirty = tab.editor.is_modified();
            // Check overflow if dirty state changed (adds/removes '*')
            // We can't easily know if it changed here without checking previous state,
            // but is_dirty is cheap to check.
            // For now, let's call it. It's not too expensive.
            self.check_tabs_overflow();
            task
        } else {
            self.log("ERROR", "Editor tab not found for event");
            Task::none()
        };
        #[cfg(not(target_arch = "wasm32"))]
        if let EditorMessage::MouseHover(point) = event {
            self.handle_lsp_hover_from_mouse(editor_id, *point);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let EditorMessage::JumpClick(point) = event
            && let Some(tab) = self.get_tab(editor_id)
        {
            tab.editor.lsp_request_definition_at(*point);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let EditorMessage::CharacterInput(ch) = event
            && !self.lsp_applying_completion
        {
            // If input is not a word character, clear completion state
            if !ch.is_alphanumeric() && *ch != '_' {
                self.lsp_overlay.clear_completions();
                if !self.lsp_overlay.hover_visible {
                    self.lsp_overlay_editor = None;
                }
            } else {
                self.lsp_overlay.completion_suppressed = false;
                if !self.lsp_overlay.all_completions.is_empty()
                    && let Some(tab) =
                        self.tabs.iter().find(|t| t.id == editor_id)
                {
                    let content = tab.editor.content();
                    let (line, col) = tab.editor.cursor_position();
                    if let Some(line_content) = content.lines().nth(line) {
                        let word_start =
                            Self::find_word_start(line_content, col);
                        let current_word = &line_content[word_start..col];
                        self.lsp_overlay.completion_filter =
                            current_word.to_string();
                        self.lsp_overlay.filter_completions();
                    }
                }
            }
        }
        task
    }

    /// Handles periodic tick events for cursor blinking in all editors.
    fn handle_tick(&mut self) -> Task<Message> {
        self.spinner_frame = (self.spinner_frame + 1) % 8;

        #[cfg(not(target_arch = "wasm32"))]
        let lsp_task = {
            self.process_lsp_hover_timers();
            self.drain_lsp_events()
        };
        #[cfg(target_arch = "wasm32")]
        let lsp_task = Task::none();

        let mut tasks = Vec::new();
        tasks.push(lsp_task);

        for tab in &mut self.tabs {
            let id = tab.id;
            tasks.push(
                tab.editor
                    .update(&EditorMessage::Tick)
                    .map(move |e| Message::EditorEvent(id, e)),
            );
        }
        Task::batch(tasks)
    }

    /// Handles loading a code template into a specific editor.
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
        let Some((editor, current_file)) = self.get_editor_and_file(editor_id)
        else {
            self.log("ERROR", "Editor tab not found for template");
            return Task::none();
        };

        let task = editor.reset(template.content());
        editor.set_theme(style);
        editor.set_reveal_in_file_manager_enabled(false);
        *current_file = None;

        if let Some(tab) = self.get_tab(editor_id) {
            tab.is_dirty = false;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.sync_lsp_for_template(editor_id, template);
        }

        task.map(move |e| Message::EditorEvent(editor_id, e))
    }

    /// Handles code execution simulation for the active editor.
    fn handle_run_code(&mut self) -> Task<Message> {
        self.log(
            "INFO",
            &format!("Running code from {:?} editor...", self.active_tab_id),
        );
        let Some(editor) = self.get_active_editor() else {
            self.log("ERROR", "No active tab to run code");
            return Task::none();
        };
        let line_count = editor.content().lines().count();
        self.log("OUTPUT", &format!("Script has {} lines", line_count));
        self.log("OUTPUT", "Execution completed (simulated)");
        Task::none()
    }

    /// Handles changes to the text input field.
    fn handle_text_input_changed(&mut self, value: String) -> Task<Message> {
        self.text_input_value = value;
        for tab in &mut self.tabs {
            tab.editor.lose_focus();
        }
        Task::none()
    }

    /// Handles clicks on the text input field.
    fn handle_text_input_clicked(&mut self) -> Task<Message> {
        for tab in &mut self.tabs {
            tab.editor.lose_focus();
        }
        Task::none()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn handle_jump_to_file(
        &mut self,
        path: PathBuf,
        line: usize,
        col: usize,
    ) -> Task<Message> {
        // Check if file is already open
        if let Some(tab) =
            self.tabs.iter().find(|t| t.file_path.as_ref() == Some(&path))
        {
            let editor_id = tab.id;
            self.active_tab_id = editor_id;
            if let Some(tab) = self.get_tab(editor_id) {
                return tab
                    .editor
                    .set_cursor(line, col)
                    .map(move |e| Message::EditorEvent(editor_id, e));
            }
            self.log("ERROR", "Editor tab not found for jump");
            return Task::none();
        }

        // Open file in new tab (or reuse empty one)
        Task::perform(file_ops::read_file(path), move |result| {
            Message::FileOpenedAndJump(result.map(|(p, c)| (p, c, line, col)))
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn handle_file_opened_and_jump(
        &mut self,
        result: Result<(PathBuf, String, usize, usize), String>,
    ) -> Task<Message> {
        match result {
            Ok((path, content, line, col)) => {
                // Check if file is already open (double check)
                if let Some(tab) = self
                    .tabs
                    .iter()
                    .find(|t| t.file_path.as_ref() == Some(&path))
                {
                    let editor_id = tab.id;
                    self.active_tab_id = editor_id;
                    if let Some(tab) = self.get_tab(editor_id) {
                        return tab
                            .editor
                            .set_cursor(line, col)
                            .map(move |e| Message::EditorEvent(editor_id, e));
                    }
                    self.log("ERROR", "Editor tab not found for jump");
                    return Task::none();
                }

                // New tab logic similar to handle_file_opened
                let active_tab_id = self.active_tab_id;
                let reuse_tab = self.get_active_tab().is_some_and(|tab| {
                    tab.file_path.is_none()
                        && tab.editor.content().trim().is_empty()
                        && !tab.is_dirty
                });

                let target_tab_id = if reuse_tab {
                    active_tab_id
                } else {
                    let new_id = EditorId(self.next_tab_id);
                    self.next_tab_id += 1;

                    let mut editor = Self::new_editor(&content);
                    let font = self.current_font.font();
                    editor.set_font(font);
                    editor.set_font_size(
                        self.current_font_size,
                        self.auto_adjust_line_height,
                    );
                    editor.set_line_height(self.current_line_height);
                    editor
                        .set_theme(theme::from_iced_theme(&self.current_theme));
                    editor.set_language(self.current_language);
                    editor.set_reveal_in_file_manager_enabled(true);

                    let tab = EditorTab {
                        id: new_id,
                        editor,
                        file_path: Some(path.clone()),
                        is_dirty: false,
                        #[cfg(not(target_arch = "wasm32"))]
                        lsp_server_key: None,
                    };
                    self.tabs.push(tab);
                    self.active_tab_id = new_id;
                    new_id
                };

                let Some((editor, current_file)) =
                    self.get_editor_and_file(target_tab_id)
                else {
                    self.log("ERROR", "Target tab not found for opened file");
                    self.error_message = Some(
                        "Target tab not found for opened file".to_string(),
                    );
                    return Task::none();
                };
                *current_file = Some(path.clone());
                editor.set_reveal_in_file_manager_enabled(true);
                let t1 = editor
                    .reset(&content)
                    .map(move |e| Message::EditorEvent(target_tab_id, e));
                let t2 = editor
                    .set_cursor(line, col)
                    .map(move |e| Message::EditorEvent(target_tab_id, e));
                editor.mark_saved();
                self.error_message = None;

                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.sync_lsp_for_path(target_tab_id, &path);
                }
                self.check_tabs_overflow();
                Task::batch([t1, t2])
            }
            Err(err) => {
                self.log("ERROR", &err);
                self.error_message = Some(err);
                Task::none()
            }
        }
    }

    /// Checks if the total width of tabs overflows the window width
    pub fn check_tabs_overflow(&mut self) {
        let total_tabs_width: f32 = self
            .tabs
            .iter()
            .map(|tab| {
                let name = tab
                    .file_path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("Untitled");
                let modified = if tab.is_dirty { "*" } else { "" };
                let label = format!("{}{}", name, modified);

                // Approximate width:
                // - Padding: 10 * 2 = 20
                // - Close button: 20
                // - Spacing inside tab: 5
                // - Text: len * 9 (approximate char width for size 14)
                // - Extra space for indicator/border: 2
                let text_width = label.len() as f32 * 9.0;
                text_width + 45.0
            })
            .sum();

        let spacing_width = (self.tabs.len().saturating_sub(1) as f32) * 2.0;
        let total_width = total_tabs_width + spacing_width + 20.0; // +20 padding

        self.tabs_overflow = total_width > self.window_width;
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
            Message::ClearLog => {
                self.log_messages.clear();
                self.log("INFO", "Log cleared");
                Task::none()
            }
            // File operations
            Message::OpenFile => self.handle_file_open(),
            Message::FileOpened(result) => self.handle_file_opened(result),
            Message::SaveFile => self.handle_file_save(self.active_tab_id),
            Message::SaveFileAs => self.handle_file_save_as(self.active_tab_id),
            Message::FileSaved(editor_id, result) => {
                self.handle_file_saved(editor_id, result)
            }
            #[cfg(not(target_arch = "wasm32"))]
            Message::FileRevealed(result) => self.handle_file_revealed(result),
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
            Message::ToggleFolding(editor_id, enabled) => {
                self.handle_toggle_folding(editor_id, enabled)
            }
            Message::ToggleAutoIndent(editor_id, enabled) => {
                self.handle_toggle_auto_indent(editor_id, enabled)
            }
            Message::ToggleAutoCloseBrackets(editor_id, enabled) => {
                self.handle_toggle_auto_close_brackets(editor_id, enabled)
            }
            Message::IndentStyleChanged(editor_id, style) => {
                self.handle_indent_style_changed(editor_id, style)
            }
            Message::ToggleSearchReplace(editor_id, enabled) => {
                self.handle_toggle_search_replace(editor_id, enabled)
            }
            Message::ToggleLineNumbers(editor_id, enabled) => {
                self.handle_toggle_line_numbers(editor_id, enabled)
            }
            Message::ToggleShowWhitespace(editor_id, enabled) => {
                self.handle_toggle_show_whitespace(editor_id, enabled)
            }
            Message::ToggleVim(editor_id, enabled) => {
                self.handle_toggle_vim(editor_id, enabled)
            }
            Message::ToggleLsp(editor_id, enabled) => {
                self.handle_toggle_lsp(editor_id, enabled)
            }
            // Editor events
            Message::EditorEvent(editor_id, event) => {
                self.handle_editor_event(editor_id, &event)
            }
            Message::EditorMouseEntered(_editor_id) => {
                #[cfg(not(target_arch = "wasm32"))]
                if self.lsp_overlay_editor == Some(_editor_id) {
                    self.lsp_hover_hide_deadline = None;
                }
                Task::none()
            }
            Message::EditorMouseExited(_editor_id) => {
                #[cfg(not(target_arch = "wasm32"))]
                if self.lsp_overlay_editor == Some(_editor_id)
                    && self.lsp_overlay.hover_visible
                    && !self.lsp_overlay.hover_interactive
                {
                    self.lsp_hover_hide_deadline = Some(
                        Instant::now()
                            + Duration::from_millis(LSP_HOVER_HIDE_DELAY_MS),
                    );
                }
                Task::none()
            }
            Message::Tick => self.handle_tick(),
            Message::WindowEvent(event) => {
                if let Event::Window(window_event) = &event
                    && let window::Event::Resized(size) = window_event
                {
                    self.window_width = size.width;
                    self.check_tabs_overflow();
                }

                #[cfg(not(target_arch = "wasm32"))]
                {
                    if matches!(event, Event::Mouse(mouse::Event::CursorLeft))
                        && self.lsp_overlay.hover_visible
                    {
                        self.lsp_overlay.hover_interactive = false;
                        self.lsp_hover_hide_deadline = Some(
                            Instant::now()
                                + Duration::from_millis(
                                    LSP_HOVER_CURSOR_LEFT_MS,
                                ),
                        );
                    }

                    // Handle Escape key to close completion
                    if let Event::Keyboard(iced::keyboard::Event::KeyPressed {
                        key:
                            iced::keyboard::Key::Named(
                                iced::keyboard::key::Named::Escape,
                            ),
                        ..
                    }) = &event
                        && self.lsp_overlay.completion_visible
                    {
                        self.lsp_overlay.clear_completions();
                        self.clear_overlay_editor_if_no_hover();
                    }
                }
                Task::none()
            }
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
            #[cfg(not(target_arch = "wasm32"))]
            Message::JumpToFile(path, line, col) => {
                self.handle_jump_to_file(path, line, col)
            }
            #[cfg(not(target_arch = "wasm32"))]
            Message::FileOpenedAndJump(result) => {
                self.handle_file_opened_and_jump(result)
            }
            #[cfg(not(target_arch = "wasm32"))]
            Message::LspOverlay(msg) => {
                use iced_code_editor::LspOverlayMessage;
                match msg {
                    LspOverlayMessage::HoverEntered => {
                        self.lsp_overlay.hover_interactive = true;
                        self.lsp_hover_hide_deadline = None;
                        for tab in &mut self.tabs {
                            tab.editor.lose_focus();
                        }
                        focus(Id::new("lsp_hover_text_editor"))
                    }
                    LspOverlayMessage::HoverExited => {
                        self.lsp_overlay.hover_interactive = false;
                        self.lsp_hover_hide_deadline = Some(
                            Instant::now()
                                + Duration::from_millis(
                                    LSP_HOVER_TOOLTIP_EXIT_MS,
                                ),
                        );
                        Task::none()
                    }
                    LspOverlayMessage::CompletionClosed => {
                        self.lsp_overlay.completion_visible = false;
                        self.lsp_overlay.completion_suppressed = false;
                        self.clear_overlay_editor_if_no_hover();
                        Task::none()
                    }
                    LspOverlayMessage::CompletionSelected(index) => {
                        self.lsp_applying_completion = true;
                        let completion = self
                            .lsp_overlay
                            .completion_items
                            .get(index)
                            .cloned();
                        if let Some(item) = completion {
                            self.apply_completion(&item);
                        }
                        self.lsp_applying_completion = false;
                        self.lsp_overlay.completion_visible = false;
                        self.lsp_overlay.completion_suppressed = true;
                        self.clear_overlay_editor_if_no_hover();
                        Task::none()
                    }
                    LspOverlayMessage::CompletionNavigateUp => {
                        self.navigate_completion(-1)
                    }
                    LspOverlayMessage::CompletionNavigateDown => {
                        self.navigate_completion(1)
                    }
                    LspOverlayMessage::CompletionConfirm => {
                        if self.lsp_overlay.completion_visible {
                            self.lsp_applying_completion = true;
                            let completion = self
                                .lsp_overlay
                                .selected_item()
                                .map(str::to_owned);
                            if let Some(item) = completion {
                                self.apply_completion(&item);
                            }
                            self.lsp_applying_completion = false;
                            self.lsp_overlay.completion_visible = false;
                            self.lsp_overlay.completion_suppressed = true;
                            self.clear_overlay_editor_if_no_hover();
                        }
                        Task::none()
                    }
                }
            }
            // Tab management
            Message::CloseTab(id) => {
                if self.tabs.len() > 1 {
                    if let Some(index) =
                        self.tabs.iter().position(|t| t.id == id)
                    {
                        self.tabs.remove(index);
                        if self.active_tab_id == id {
                            // Select the last tab or the one before the removed one
                            let new_index = if index >= self.tabs.len() {
                                self.tabs.len() - 1
                            } else {
                                index
                            };
                            self.active_tab_id = self.tabs[new_index].id;
                        }
                        self.check_tabs_overflow();
                    }
                } else {
                    // Don't close the last tab, just clear it?
                    // Or close app? User said "can close file".
                    // If it's the last tab, maybe just reset it to empty?
                    if let Some(tab) = self.tabs.first_mut() {
                        let default_content = "";
                        let _ = tab.editor.reset(default_content);
                        tab.file_path = None;
                        tab.editor.set_reveal_in_file_manager_enabled(false);
                        tab.is_dirty = false;
                    }
                    self.check_tabs_overflow();
                }
                Task::none()
            }
            Message::SelectTab(id) => {
                self.active_tab_id = id;
                Task::none()
            }
            Message::NewTab => {
                let new_id = EditorId(self.next_tab_id);
                self.next_tab_id += 1;

                let mut editor = Self::new_editor("");
                let font = self.current_font.font();
                editor.set_font(font);
                editor.set_font_size(
                    self.current_font_size,
                    self.auto_adjust_line_height,
                );
                editor.set_line_height(self.current_line_height);
                editor.set_theme(theme::from_iced_theme(&self.current_theme));
                editor.set_language(self.current_language);

                let tab = EditorTab {
                    id: new_id,
                    editor,
                    file_path: None,
                    is_dirty: false,
                    #[cfg(not(target_arch = "wasm32"))]
                    lsp_server_key: None,
                };
                self.tabs.push(tab);
                self.active_tab_id = new_id;
                self.check_tabs_overflow();
                Task::none()
            }
        }
    }

    /// Subscription for periodic updates.
    pub fn subscription(_state: &Self) -> Subscription<Message> {
        // Cursor blink
        Subscription::batch([
            window::frames().map(|_| Message::Tick),
            event::listen().map(Message::WindowEvent),
        ])
    }

    /// Returns the current theme for the application.
    pub fn theme(&self) -> Theme {
        self.current_theme.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toggle_vim_updates_editor_vim_enabled() {
        let (mut app, _) = DemoApp::new();
        let tab_id = app.active_tab_id;

        assert!(!app.get_active_editor().is_some_and(|e| e.vim_enabled()));

        let _ = app.handle_toggle_vim(tab_id, true);
        assert!(app.get_active_editor().is_some_and(|e| e.vim_enabled()));

        let _ = app.handle_toggle_vim(tab_id, false);
        assert!(!app.get_active_editor().is_some_and(|e| e.vim_enabled()));
    }

    #[test]
    fn test_toggle_auto_close_brackets_updates_editor_setting() {
        let (mut app, _) = DemoApp::new();
        let tab_id = app.active_tab_id;

        assert!(
            app.get_active_editor().is_some_and(|e| e.auto_close_brackets())
        );

        let _ = app.handle_toggle_auto_close_brackets(tab_id, false);
        assert!(
            !app.get_active_editor().is_some_and(|e| e.auto_close_brackets())
        );

        let _ = app.handle_toggle_auto_close_brackets(tab_id, true);
        assert!(
            app.get_active_editor().is_some_and(|e| e.auto_close_brackets())
        );
    }

    #[test]
    fn test_reveal_request_forwards_tab_path() {
        let (mut app, _) = DemoApp::new();
        let path = PathBuf::from("/tmp/iced-code-editor/reveal.lua");

        let _ = app.handle_file_opened(Ok((
            path.clone(),
            "print('reveal')".to_string(),
        )));

        assert_eq!(app.reveal_path_for_editor(app.active_tab_id), Ok(path));
        assert!(
            app.get_active_editor().is_some_and(|editor| {
                editor.reveal_in_file_manager_enabled()
            })
        );
    }

    #[test]
    fn test_untitled_tab_does_not_enable_reveal() {
        let (mut app, _) = DemoApp::new();

        assert_eq!(
            app.reveal_path_for_editor(app.active_tab_id),
            Err("This tab does not have a file path to reveal".to_string())
        );
        assert!(
            app.get_active_editor()
                .is_some_and(|editor| !editor.reveal_in_file_manager_enabled())
        );
    }

    #[test]
    fn test_reveal_error_is_reported() {
        let (mut app, _) = DemoApp::new();
        let error = "Unable to reveal /tmp/missing.lua: test failure";

        let _ = app.handle_file_revealed(Err(error.to_string()));
        let expected_log = format!("[ERROR] {error}");

        assert_eq!(app.error_message.as_deref(), Some(error));
        assert_eq!(
            app.log_messages.last().map(String::as_str),
            Some(expected_log.as_str())
        );
    }

    #[test]
    fn test_save_result_updates_originating_tab() {
        let (mut app, _) = DemoApp::new();
        let saved_editor_id = app.active_tab_id;
        let _ = app.update(Message::NewTab);
        let active_editor_id = app.active_tab_id;
        let path = PathBuf::from("/tmp/iced-code-editor/vim-write.lua");

        let _ = app.handle_file_saved(saved_editor_id, Ok(path.clone()));

        assert_eq!(
            app.tabs
                .iter()
                .find(|tab| tab.id == saved_editor_id)
                .and_then(|tab| tab.file_path.as_ref()),
            Some(&path)
        );
        assert_eq!(
            app.tabs
                .iter()
                .find(|tab| tab.id == active_editor_id)
                .and_then(|tab| tab.file_path.as_ref()),
            None
        );
    }
}
