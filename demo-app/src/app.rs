//! Core application state and update logic for the demo app.
//!
//! Defines [`DemoApp`], the root Iced application state (open editor tabs,
//! LSP session state, UI preferences), and the top-level `update`/`view`
//! wiring that [`main`](crate) plugs into `iced::application`.
//!
//! The [`Message`] enum and most `update` handlers live in submodules
//! grouped by concern: `message` (the enum itself), `tabs` (tab/editor
//! creation, lookup, and event forwarding), `files` (open/save/reveal and
//! LSP jump-to-definition), `settings` (font/theme/language/per-editor
//! toggles), and `app_lsp` (LSP client lifecycle, hover, and completion).
//! `update` itself stays here as the single dispatch point: Iced's
//! `application` builder needs one `fn(&mut DemoApp, Message) ->
//! Task<Message>` regardless of how the handlers behind it are organized.

use crate::types::{EditorId, EditorToggle, FontOption, Template};
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
use iced_code_editor::{Language, theme};
#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

mod files;
mod message;
mod settings;
mod tabs;

pub use message::Message;
pub use tabs::EditorTab;

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
    /// Whether the editor options dropdown panel is expanded
    pub show_editor_options: bool,
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
            show_editor_options: false,
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

        // Auto-attach an LSP server for the initial demo buffer, if one is
        // available for its language. This spawns a real subprocess, so it
        // must run through the async Task/Message flow (like the manual LSP
        // toggle) rather than synchronously here: `new()` is also called by
        // unit tests, and a synchronous spawn would make their behavior
        // depend on whether a language server happens to be on `PATH`.
        #[cfg(not(target_arch = "wasm32"))]
        let startup_task = Task::done(Message::ToggleEditor(
            active_tab_id,
            EditorToggle::Lsp,
            true,
        ));
        #[cfg(target_arch = "wasm32")]
        let startup_task = Task::none();

        (app, startup_task)
    }

    /// Adds a log message.
    fn log(&mut self, level: &str, message: &str) {
        self.log_messages.push(format!("[{}] {}", level, message));
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
            Message::ToggleEditorOptions => {
                self.show_editor_options = !self.show_editor_options;
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
            Message::ToggleEditor(editor_id, toggle, enabled) => {
                self.handle_toggle_editor(editor_id, toggle, enabled)
            }
            Message::IndentStyleChanged(editor_id, style) => {
                self.handle_indent_style_changed(editor_id, style)
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
                // Always creates a fresh tab (unlike `open_content_in_tab`,
                // which may reuse an empty active tab) — an explicit "new
                // tab" action must always produce a new one.
                let new_id = EditorId(self.next_tab_id);
                self.next_tab_id += 1;

                let editor = self.configured_editor("");
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
