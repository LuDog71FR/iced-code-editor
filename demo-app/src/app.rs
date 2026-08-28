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
use iced::widget::text_editor;
use iced::widget::text_editor::{Action, Edit, Motion};
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
use std::sync::Arc;

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

/// Greatest number of lines the output log keeps.
///
/// Well past what the pane can show, so the bound is invisible in normal use;
/// its job is to stop a chatty language server growing the log until the
/// process dies (see [`DemoApp::trim_log`]).
const MAX_LOG_LINES: usize = 2_000;

/// Number of lines dropped each time the log is trimmed.
///
/// Trimming costs a rebuild of the whole `text_editor::Content`, so lines go in
/// batches: one rebuild per this many messages instead of one per message.
const LOG_TRIM_BATCH: usize = 200;

/// Moves a saved cursor up by `lines`, as if that many lines had been removed
/// from the top of the buffer.
///
/// Used after the log is trimmed, so a reader ends up back on the text they
/// were looking at rather than wherever its old line number now points. A
/// position whose line was itself trimmed away collapses to the very start:
/// the closest surviving place, and the only column guaranteed to exist on the
/// line it lands on.
///
/// # Arguments
///
/// * `cursor` - The cursor as it stood before the lines were removed
/// * `lines` - Number of lines removed from the top
///
/// # Returns
/// The equivalent cursor in the trimmed buffer, selection included.
fn shift_cursor_up(
    cursor: text_editor::Cursor,
    lines: usize,
) -> text_editor::Cursor {
    fn shift(
        position: text_editor::Position,
        lines: usize,
    ) -> text_editor::Position {
        position
            .line
            .checked_sub(lines)
            .map_or(text_editor::Position { line: 0, column: 0 }, |line| {
                text_editor::Position { line, column: position.column }
            })
    }

    text_editor::Cursor {
        position: shift(cursor.position, lines),
        selection: cursor.selection.map(|position| shift(position, lines)),
    }
}

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
    /// Read-only mirror of [`log_messages`](Self::log_messages) backing the
    /// output pane's `text_editor`, so log lines can be selected and copied.
    ///
    /// Appended to by [`DemoApp::log`] and rebuilt only by
    /// [`DemoApp::refresh_log_content`], which the initial build and **Clear**
    /// use; never edited by the user — [`Message::LogAction`] drops editing
    /// actions. `log_content.text()` always equals
    /// `log_messages.join("\n")`.
    pub log_content: text_editor::Content,
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
        // Cursor at the end, so an untouched pane follows the newest output
        // (see `DemoApp::log`).
        let mut log_content =
            text_editor::Content::with_text(&log_messages.join("\n"));
        log_content.perform(Action::Move(Motion::DocumentEnd));

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
            log_content,
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
    ///
    /// The line is appended to [`log_content`](Self::log_content) in place.
    /// Rebuilding the whole `Content` instead — as this used to — re-parsed the
    /// entire log for every message, which is quadratic over a session, and
    /// threw away the reader's selection, which is the one thing the output
    /// pane's `text_editor` exists to provide.
    ///
    /// The pane follows the newest output only while nobody is reading it: a
    /// cursor still sitting at the end means it has not been moved, so it is
    /// left there and the view scrolls to the new line. A reader who has
    /// clicked or selected somewhere keeps their place and their selection.
    fn log(&mut self, level: &str, message: &str) {
        let line = format!("[{level}] {message}");

        // Ask the editor itself where the end is rather than deriving it from
        // line lengths: `column` is a byte index, and the two notions must not
        // be allowed to disagree.
        let before = self.log_content.cursor();
        self.log_content.perform(Action::Move(Motion::DocumentEnd));
        let was_following = self.log_cursor_was_following(before);

        // The newline separates this line from the previous one, so the very
        // first line must not carry it -- otherwise the log opens on a blank
        // row and `log_content` stops matching `log_messages.join("\n")`.
        let addition = if self.log_messages.is_empty() {
            line.clone()
        } else {
            format!("\n{line}")
        };
        self.log_content.perform(Action::Edit(Edit::Paste(Arc::new(addition))));

        if !was_following {
            self.log_content.move_to(before);
        }

        self.log_messages.push(line);
        self.trim_log();
    }

    /// Returns whether the pane is still following the newest output.
    ///
    /// True while the cursor sits at the very end of the log with nothing
    /// selected, which means nobody has moved it. The end is obtained from the
    /// editor rather than derived from line lengths:
    /// [`Position::column`](text_editor::Position::column) is a byte index, and
    /// the two notions must not be allowed to disagree.
    ///
    /// # Arguments
    ///
    /// * `before` - The cursor as it stood before the caller moved it to the
    ///   end of the document
    fn log_cursor_was_following(&self, before: text_editor::Cursor) -> bool {
        before.selection.is_none()
            && before.position == self.log_content.cursor().position
    }

    /// Drops the oldest lines once the log grows past [`MAX_LOG_LINES`].
    ///
    /// Without this the log is bounded by nothing, and the LSP client feeds it
    /// directly — up to `MAX_LSP_EVENTS_PER_TICK` messages a tick, from a
    /// server this application does not control.
    ///
    /// Trimming removes [`LOG_TRIM_BATCH`] lines at a time rather than one per
    /// message. Dropping the front of a `text_editor::Content` means rebuilding
    /// it, and rebuilding once per message is exactly the quadratic cost
    /// [`DemoApp::log`] exists to avoid; one rebuild per `LOG_TRIM_BATCH`
    /// messages, over a log that can no longer grow, is linear again.
    fn trim_log(&mut self) {
        if self.log_messages.len() <= MAX_LOG_LINES {
            return;
        }

        let removed = self.log_messages.len()
            - MAX_LOG_LINES.saturating_sub(LOG_TRIM_BATCH);
        self.log_messages.drain(..removed);

        // A reader is `removed` lines further down the log than they were, so
        // put them back where they were looking rather than at the end. One
        // who was following the tail stays on it.
        let before = self.log_content.cursor();
        self.log_content.perform(Action::Move(Motion::DocumentEnd));
        let was_following = self.log_cursor_was_following(before);

        self.refresh_log_content();

        if !was_following {
            self.log_content.move_to(shift_cursor_up(before, removed));
        }
    }

    /// Rebuilds [`log_content`](Self::log_content) from
    /// [`log_messages`](Self::log_messages).
    ///
    /// Only for the two places where appending cannot express the change: the
    /// initial build, and **Clear**, which replaces the whole log. Every other
    /// change goes through [`DemoApp::log`], which appends. Rebuilding drops
    /// any selection in progress, which is why it is not the general path.
    ///
    /// The cursor is left at the end so a pane nobody has touched follows the
    /// newest output — see [`DemoApp::log`].
    fn refresh_log_content(&mut self) {
        self.log_content =
            text_editor::Content::with_text(&self.log_messages.join("\n"));
        self.log_content.perform(Action::Move(Motion::DocumentEnd));
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
        // The tab may have been holding a file: templates are Lua, and the tab
        // becomes untitled again.
        editor.set_syntax(Self::UNTITLED_SYNTAX);
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
                // `log` appends, so the emptied list has to reach `log_content`
                // some other way: this is one of the two places a rebuild is
                // the only thing that can express the change.
                self.refresh_log_content();
                self.log("INFO", "Log cleared");
                Task::none()
            }
            Message::LogAction(action) => {
                // The output pane is read-only: selection, scrolling and
                // cursor moves are applied, edits are dropped.
                if !action.is_edit() {
                    self.log_content.perform(action);
                }
                Task::none()
            }
            Message::CopyLog => {
                iced::clipboard::write(self.log_messages.join("\n"))
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
                        tab.editor.set_syntax(Self::UNTITLED_SYNTAX);
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

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::text_editor::{Action, Edit};

    /// The first line every fresh [`DemoApp`] logs.
    const STARTUP_LOG: &str = "[INFO] Application started";

    /// A cursor on `line`, with no selection.
    fn caret_on(line: usize) -> text_editor::Cursor {
        text_editor::Cursor {
            position: text_editor::Position { line, column: 4 },
            selection: None,
        }
    }

    #[test]
    fn test_shift_cursor_up_moves_a_surviving_line_by_the_amount_removed() {
        let shifted = shift_cursor_up(caret_on(500), 201);

        assert_eq!(shifted.position.line, 299);
        assert_eq!(shifted.position.column, 4, "the column is unaffected");
    }

    #[test]
    fn test_shift_cursor_up_collapses_a_line_that_was_trimmed_away() {
        // Line 100 no longer exists after 201 lines were dropped. Column 4 is
        // not guaranteed to exist on the line it lands on either, so the whole
        // position collapses rather than keeping a column out of thin air.
        let shifted = shift_cursor_up(caret_on(100), 201);

        assert_eq!(shifted.position.line, 0);
        assert_eq!(shifted.position.column, 0);
    }

    #[test]
    fn test_shift_cursor_up_moves_the_selection_with_the_cursor() {
        // Both ends move, or the selection would silently grow or shrink.
        let cursor = text_editor::Cursor {
            position: text_editor::Position { line: 500, column: 4 },
            selection: Some(text_editor::Position { line: 400, column: 9 }),
        };

        let shifted = shift_cursor_up(cursor, 201);

        assert_eq!(shifted.position.line, 299);
        assert_eq!(shifted.selection.map(|position| position.line), Some(199));
    }

    #[test]
    fn test_the_log_stops_growing_once_it_reaches_its_bound() {
        // The bound exists because the LSP client feeds this log from a server
        // this application does not control. Logging past it must drop the
        // oldest lines, not the newest, and must leave the two views in step.
        let (mut app, _) = DemoApp::new();

        for index in 0..MAX_LOG_LINES + LOG_TRIM_BATCH {
            app.log("OUTPUT", &format!("line {index}"));
        }

        assert!(
            app.log_messages.len() <= MAX_LOG_LINES,
            "the log grew to {}",
            app.log_messages.len()
        );
        assert_eq!(app.log_content.text(), app.log_messages.join("\n"));
        assert_eq!(
            app.log_messages.last().map(String::as_str),
            Some("[OUTPUT] line 2199"),
            "the newest line must survive"
        );
        assert!(
            !app.log_messages.iter().any(|line| line.contains("line 0")),
            "the oldest lines must be the ones dropped"
        );
    }

    #[test]
    fn test_trimming_leaves_a_follower_following() {
        // A trim rebuilds the content, so the cheap mistake is to let it reset
        // the reader. Nobody has moved this cursor, so it stays on the tail.
        let (mut app, _) = DemoApp::new();

        for index in 0..MAX_LOG_LINES + LOG_TRIM_BATCH {
            app.log("OUTPUT", &format!("line {index}"));
        }

        let last_line = app.log_content.line_count() - 1;
        assert_eq!(app.log_content.cursor().position.line, last_line);
    }

    #[test]
    fn test_appending_many_lines_keeps_the_two_views_identical() {
        // `log` appends to `log_content` instead of rebuilding it, so the two
        // can now drift where they could not before. This is the invariant
        // that says they do not -- checked with no `trim_end`, since a stray
        // leading or trailing newline is exactly the way an append goes wrong.
        let (mut app, _) = DemoApp::new();

        for index in 0..50 {
            app.log("OUTPUT", &format!("line {index}"));
        }

        assert_eq!(app.log_content.text(), app.log_messages.join("\n"));
        assert_eq!(app.log_messages.len(), 51);
    }

    #[test]
    fn test_the_first_line_after_a_clear_carries_no_leading_blank() {
        // The one case where the separating newline must not be written: the
        // log is empty, so there is nothing to separate the line from.
        let (mut app, _) = DemoApp::new();
        app.log("OUTPUT", "hello");

        let _ = app.update(Message::ClearLog);

        assert_eq!(app.log_content.text(), "[INFO] Log cleared");
    }

    #[test]
    fn test_the_pane_follows_the_tail_while_nobody_has_moved_the_cursor() {
        // Nothing has touched the pane, so the cursor rides the end and the
        // view scrolls to each new line.
        let (mut app, _) = DemoApp::new();

        app.log("OUTPUT", "hello");

        let last_line = app.log_content.line_count() - 1;
        assert_eq!(app.log_content.cursor().position.line, last_line);
    }

    #[test]
    fn test_a_reader_keeps_their_place_when_a_line_arrives() {
        // The defect this replaced: rebuilding the whole `Content` sent the
        // pane back to the top on every message, so a reader was pulled away
        // from what they were reading by the next thing the LSP server said.
        let (mut app, _) = DemoApp::new();
        app.log("OUTPUT", "one");
        app.log("OUTPUT", "two");
        let _ = app.update(Message::LogAction(Action::Move(
            iced::widget::text_editor::Motion::DocumentStart,
        )));

        app.log("OUTPUT", "three");

        assert_eq!(
            app.log_content.cursor().position.line,
            0,
            "the reader's cursor must not be dragged to the new line"
        );
    }

    #[test]
    fn test_a_selection_survives_a_line_arriving() {
        // Selecting and copying is the whole reason the output pane is a
        // `text_editor` rather than a column of `text` widgets; a selection
        // that dies on the next log message is the feature not working.
        let (mut app, _) = DemoApp::new();
        let _ = app.update(Message::LogAction(Action::SelectAll));
        let selected = app.log_content.selection();
        assert_eq!(selected.as_deref(), Some(STARTUP_LOG));

        app.log("OUTPUT", "an interrupting message");

        assert_eq!(app.log_content.selection(), selected);
    }

    #[test]
    fn test_log_content_mirrors_log_messages() {
        let (mut app, _) = DemoApp::new();

        app.log("OUTPUT", "hello");

        assert_eq!(
            app.log_content.text().trim_end(),
            app.log_messages.join("\n")
        );
    }

    #[test]
    fn test_clear_log_rebuilds_the_content() {
        let (mut app, _) = DemoApp::new();
        app.log("OUTPUT", "hello");

        let _ = app.update(Message::ClearLog);

        assert_eq!(app.log_content.text().trim_end(), "[INFO] Log cleared");
    }

    #[test]
    fn test_log_action_applies_selection() {
        let (mut app, _) = DemoApp::new();

        let _ = app.update(Message::LogAction(Action::SelectAll));

        assert_eq!(app.log_content.selection().as_deref(), Some(STARTUP_LOG));
    }

    #[test]
    fn test_log_action_ignores_edits() {
        let (mut app, _) = DemoApp::new();

        let _ = app.update(Message::LogAction(Action::Edit(Edit::Insert('x'))));

        assert_eq!(app.log_content.text().trim_end(), STARTUP_LOG);
    }
}
