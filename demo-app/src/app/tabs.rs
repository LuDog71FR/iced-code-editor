//! Tab and editor lifecycle management for [`DemoApp`]: creating editors,
//! resolving which tab should hold newly opened/created content, looking
//! tabs up by ID, forwarding editor events to the right tab, and tracking
//! whether the tab bar overflows the window width.

use super::{DemoApp, Message};
use crate::types::EditorId;
use iced::Task;
use iced_code_editor::Message as EditorMessage;
use iced_code_editor::{CodeEditor, ContextMenuEntry, ContextMenuItem, theme};
use std::path::{Path, PathBuf};

/// A single open editor tab.
pub struct EditorTab {
    /// Identifies this tab among the app's open tabs.
    pub id: EditorId,
    /// The editor instance backing this tab.
    pub editor: CodeEditor,
    /// Path to the file backing this tab, or `None` for an unsaved tab.
    pub file_path: Option<PathBuf>,
    /// Whether the tab has unsaved changes.
    pub is_dirty: bool,
    /// Key of the LSP server currently attached to this tab, if any.
    #[cfg(not(target_arch = "wasm32"))]
    pub lsp_server_key: Option<&'static str>,
}

impl DemoApp {
    /// Syntax identifier used by tabs that are not backed by a file.
    ///
    /// The demo's built-in templates ([`crate::types::Template`]) are all Lua,
    /// so an untitled tab starts out as Lua.
    pub(super) const UNTITLED_SYNTAX: &'static str = "lua";

    /// Value [`Self::format_on_save`] starts at, also used to seed the palette
    /// badge of a freshly built editor before the app's own state is applied.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) const DEFAULT_FORMAT_ON_SAVE: bool = true;

    /// Syntax identifier for a file whose language cannot be guessed.
    ///
    /// `CodeEditor::set_syntax` normalizes this to syntect's plain-text
    /// grammar, so the status bar reports "Plain Text" rather than naming a
    /// language the file is not written in.
    pub(super) const PLAIN_TEXT_SYNTAX: &'static str = "text";

    /// Resolves the syntax highlighting identifier for a tab backed by `path`.
    ///
    /// The identifier is the file's extension, lowercased so `FOO.RS` and
    /// `foo.rs` resolve alike. An extension the highlighter does not know
    /// degrades to plain text on its own.
    ///
    /// The two cases without an extension are *not* the same, and the
    /// difference is the whole point of this function. An untitled tab holds
    /// one of the demo's Lua templates, so it gets [`Self::UNTITLED_SYNTAX`].
    /// A real file with no extension — `Makefile`, `LICENSE`, `.bashrc` — is a
    /// file whose language is simply unknown, and colouring it as Lua would
    /// both misrender it and make the status bar claim "Lua"; it gets
    /// [`Self::PLAIN_TEXT_SYNTAX`] instead.
    ///
    /// # Arguments
    ///
    /// * `path` - The file backing the tab, or `None` for an untitled tab
    ///
    /// # Returns
    ///
    /// The syntax identifier to hand to `CodeEditor::set_syntax`.
    pub(super) fn syntax_for_path(path: Option<&Path>) -> String {
        let Some(path) = path else {
            return Self::UNTITLED_SYNTAX.to_string();
        };

        path.extension()
            .and_then(|extension| extension.to_str())
            .filter(|extension| !extension.is_empty())
            .map_or_else(
                || Self::PLAIN_TEXT_SYNTAX.to_string(),
                str::to_lowercase,
            )
    }

    /// Creates an editor with the demo context-menu and command-palette
    /// entries.
    ///
    /// Both surfaces emit the same action identifiers, which
    /// [`Self::handle_app_action`] resolves in one place — so an action
    /// offered by both behaves identically whichever one the user reaches
    /// for.
    ///
    /// The labels stay in English while the editor's own strings follow the
    /// language picker: `Translations` is the library's catalogue, and the
    /// demo has no catalogue of its own. A host application that wants a
    /// uniformly translated palette translates its labels before registering
    /// them — the entries are plain `String`s precisely so it can.
    ///
    /// None of them carries a `with_shortcut` hint, because none of them is
    /// bound to a key here. A hint is display-only — binding the key is the
    /// host's job — so advertising one the host has not bound points the user
    /// at a combination that does something else or nothing at all.
    pub(super) fn new_editor(content: &str) -> CodeEditor {
        #[cfg(not(target_arch = "wasm32"))]
        let format_on_save = Some(Self::DEFAULT_FORMAT_ON_SAVE);
        #[cfg(target_arch = "wasm32")]
        let format_on_save = None;

        CodeEditor::new(content, Self::UNTITLED_SYNTAX)
            .with_custom_context_menu_entries(vec![
                ContextMenuEntry::item(
                    "app.format_document",
                    "Format document",
                ),
                ContextMenuEntry::separator(),
                ContextMenuEntry::item("app.rename_symbol", "Rename symbol")
                    .with_enabled(false),
            ])
            .with_default_context_menu_enabled(true)
            .with_custom_command_palette_entries(Self::palette_entries(
                format_on_save,
            ))
    }

    /// Builds the palette commands this demo registers with every editor.
    ///
    /// `format_on_save` is baked into the entry that toggles it, since the
    /// palette shows a command's current state as an On/Off badge and reads it
    /// from the registration. Whenever the setting changes, the entries have to
    /// be registered again — see [`Self::refresh_palette_entries`].
    ///
    /// # Arguments
    ///
    /// * `format_on_save` - Whether formatting on save is currently enabled,
    ///   or `None` on WebAssembly, where there is no language server to format
    ///   with and the command therefore has no state to report
    ///
    /// # Returns
    ///
    /// The command list, in the order the palette should show it
    fn palette_entries(format_on_save: Option<bool>) -> Vec<ContextMenuItem> {
        let mut toggle_format_on_save = ContextMenuItem::new(
            "app.toggle_format_on_save",
            "Toggle Format On Save",
        );
        if let Some(enabled) = format_on_save {
            toggle_format_on_save = toggle_format_on_save.with_status(enabled);
        }

        vec![
            ContextMenuItem::new("app.open_file", "Open File"),
            ContextMenuItem::new("app.save_file_as", "Save File As"),
            ContextMenuItem::new("app.new_tab", "New Tab"),
            ContextMenuItem::new("app.close_tab", "Close Tab"),
            ContextMenuItem::new("app.run_code", "Run Code"),
            ContextMenuItem::new("app.clear_log", "Clear Log"),
            ContextMenuItem::new("app.toggle_settings", "Settings"),
            ContextMenuItem::new("app.format_document", "Format Document"),
            toggle_format_on_save,
        ]
    }

    /// Re-registers the palette commands on every open tab.
    ///
    /// Called after a setting one of them reports has changed: the badge is a
    /// snapshot taken when the entries were registered, so without this the
    /// palette would keep showing the state the toggle had before it was run.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn refresh_palette_entries(&mut self) {
        let entries = Self::palette_entries(Some(self.format_on_save));
        for tab in &mut self.tabs {
            tab.editor.set_custom_command_palette_entries(entries.clone());
        }
    }

    /// Resolves an action identifier emitted by the context menu or the
    /// command palette.
    ///
    /// # Arguments
    ///
    /// * `editor_id` - The editor the action was triggered from
    /// * `id` - The identifier the entry was registered with
    ///
    /// # Returns
    ///
    /// The task performing the action, or `Task::none()` for the actions
    /// this demo only logs
    fn handle_app_action(
        &mut self,
        editor_id: EditorId,
        id: &str,
    ) -> Task<Message> {
        match id {
            "app.open_file" => Task::done(Message::OpenFile),
            "app.save_file_as" => Task::done(Message::SaveFileAs),
            "app.new_tab" => Task::done(Message::NewTab),
            "app.close_tab" => Task::done(Message::CloseTab(editor_id)),
            "app.run_code" => Task::done(Message::RunCode),
            "app.clear_log" => Task::done(Message::ClearLog),
            "app.toggle_settings" => Task::done(Message::ToggleSettings),
            "app.format_document" => {
                #[cfg(not(target_arch = "wasm32"))]
                return Task::done(Message::FormatDocument(editor_id));
                #[cfg(target_arch = "wasm32")]
                {
                    self.log("INFO", "Formatting needs a language server");
                    Task::none()
                }
            }
            "app.toggle_format_on_save" => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.format_on_save = !self.format_on_save;
                    let state = if self.format_on_save { "on" } else { "off" };
                    self.log("INFO", &format!("Format on save: {state}"));
                    self.refresh_palette_entries();
                }
                #[cfg(target_arch = "wasm32")]
                self.log("INFO", "Formatting needs a language server");
                Task::none()
            }
            "app.rename_symbol" => {
                self.log("INFO", "Rename symbol requested");
                Task::none()
            }
            unknown => {
                self.log(
                    "WARN",
                    &format!("Ignoring unknown editor action: {unknown}"),
                );
                Task::none()
            }
        }
    }

    /// Creates a [`Self::new_editor`] configured with the app's current
    /// font, size, line height, theme, and language.
    ///
    /// Does not set reveal-in-file-manager: that policy depends on whether
    /// the caller has an associated file path, decided by
    /// [`Self::open_content_in_tab`].
    pub(super) fn configured_editor(&self, content: &str) -> CodeEditor {
        let mut editor = Self::new_editor(content);
        #[cfg(not(target_arch = "wasm32"))]
        editor.set_custom_command_palette_entries(Self::palette_entries(Some(
            self.format_on_save,
        )));
        editor.set_font(self.current_font.font());
        editor.set_font_size(
            self.current_font_size,
            self.auto_adjust_line_height,
        );
        editor.set_line_height(self.current_line_height);
        editor.set_theme(theme::from_iced_theme(&self.current_theme));
        editor.set_language(self.current_language);
        editor
    }

    /// Resolves the tab that should hold `content`, reusing the active tab
    /// if it is empty, unmodified, and has no file path, or otherwise
    /// creating and activating a new tab via [`Self::configured_editor`].
    ///
    /// Applies the reveal-in-file-manager policy — enabled only on native
    /// targets and only when `path` is `Some` — and the syntax highlighting
    /// language derived from `path` to whichever tab is returned.
    ///
    /// Callers remain responsible for any further tab-specific work:
    /// syncing existing-editor content (`reset`), cursor placement, LSP
    /// sync, dirty-flag bookkeeping, and logging.
    pub(super) fn open_content_in_tab(
        &mut self,
        path: Option<&PathBuf>,
        content: &str,
    ) -> EditorId {
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
            let editor = self.configured_editor(content);
            let tab = EditorTab {
                id: new_id,
                editor,
                file_path: path.cloned(),
                is_dirty: false,
                #[cfg(not(target_arch = "wasm32"))]
                lsp_server_key: None,
            };
            self.tabs.push(tab);
            self.active_tab_id = new_id;
            new_id
        };

        let reveal_enabled = !cfg!(target_arch = "wasm32") && path.is_some();
        let syntax = Self::syntax_for_path(path.map(PathBuf::as_path));
        if let Some(tab) = self.get_tab(target_tab_id) {
            tab.editor.set_reveal_in_file_manager_enabled(reveal_enabled);
            // A reused tab may still be highlighting the previous file's
            // language, so this has to run for reused and fresh tabs alike.
            tab.editor.set_syntax(&syntax);
        }

        target_tab_id
    }

    /// Returns a mutable reference to the active tab.
    pub fn get_active_tab(&mut self) -> Option<&mut EditorTab> {
        let id = self.active_tab_id;
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    /// Returns a mutable reference to the tab identified by `id`.
    pub fn get_tab(&mut self, id: EditorId) -> Option<&mut EditorTab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    /// Returns a mutable reference to the active editor.
    pub(super) fn get_active_editor(&mut self) -> Option<&mut CodeEditor> {
        self.get_active_tab().map(|tab| &mut tab.editor)
    }

    /// Returns a mutable reference to the editor identified by `id`.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn get_editor(
        &mut self,
        id: EditorId,
    ) -> Option<&mut CodeEditor> {
        self.get_tab(id).map(|tab| &mut tab.editor)
    }

    /// Returns a mutable reference to the specified editor and its associated file path.
    pub(super) fn get_editor_and_file(
        &mut self,
        id: EditorId,
    ) -> Option<(&mut CodeEditor, &mut Option<PathBuf>)> {
        self.get_tab(id).map(|tab| (&mut tab.editor, &mut tab.file_path))
    }

    /// Handles editor-specific events by forwarding them to the appropriate editor.
    pub(super) fn handle_editor_event(
        &mut self,
        editor_id: EditorId,
        event: &EditorMessage,
    ) -> Task<Message> {
        if let EditorMessage::CustomContextMenuAction(id)
        | EditorMessage::CommandPaletteAction(id) = event
        {
            return self.handle_app_action(editor_id, id);
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
                        self.lsp_overlay.completion_filter =
                            Self::current_word_at(line_content, col);
                        self.lsp_overlay.filter_completions();
                    }
                }
            }
        }
        task
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
                // - Text: char count * 9 (approximate char width for size 14)
                // - Extra space for indicator/border: 2
                let text_width = label.chars().count() as f32 * 9.0;
                text_width + 45.0
            })
            .sum();

        let spacing_width = (self.tabs.len().saturating_sub(1) as f32) * 2.0;
        let total_width = total_tabs_width + spacing_width + 20.0; // +20 padding

        self.tabs_overflow = total_width > self.window_width;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_toggle_format_on_save_action_flips_the_setting() {
        let (mut app, _) = DemoApp::new();
        let editor_id = app.active_tab_id;
        assert!(app.format_on_save);

        let _ = app.handle_app_action(editor_id, "app.toggle_format_on_save");
        assert!(!app.format_on_save);
        assert_eq!(
            app.log_messages.last().map(String::as_str),
            Some("[INFO] Format on save: off")
        );

        let _ = app.handle_app_action(editor_id, "app.toggle_format_on_save");
        assert!(app.format_on_save);
    }

    /// Reads the On/Off state the palette would show for the format-on-save
    /// toggle in `editor`.
    #[cfg(not(target_arch = "wasm32"))]
    fn format_on_save_badge(editor: &CodeEditor) -> Option<bool> {
        editor
            .custom_command_palette_entries()
            .iter()
            .find(|item| item.id == "app.toggle_format_on_save")
            .and_then(|item| item.status)
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_toggle_format_on_save_action_updates_the_palette_badge() {
        let (mut app, _) = DemoApp::new();
        let editor_id = app.active_tab_id;
        let badge = |app: &DemoApp| {
            app.tabs
                .iter()
                .find(|tab| tab.id == editor_id)
                .and_then(|tab| format_on_save_badge(&tab.editor))
        };
        assert_eq!(badge(&app), Some(true));

        let _ = app.handle_app_action(editor_id, "app.toggle_format_on_save");

        assert_eq!(badge(&app), Some(false));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_new_tabs_inherit_the_current_format_on_save_badge() {
        let (mut app, _) = DemoApp::new();
        let editor_id = app.active_tab_id;
        let _ = app.handle_app_action(editor_id, "app.toggle_format_on_save");

        let editor = app.configured_editor("");

        assert_eq!(format_on_save_badge(&editor), Some(false));
    }

    #[test]
    fn test_configured_editor_applies_font_size_theme_and_language() {
        let (mut app, _) = DemoApp::new();
        app.current_font_size = 21.0;
        app.current_line_height = 30.0;

        let editor = app.configured_editor("hello");
        assert_eq!(editor.content(), "hello");
        assert!((editor.font_size() - 21.0).abs() < f32::EPSILON);
        assert!((editor.line_height() - 30.0).abs() < f32::EPSILON);
        // configured_editor never touches reveal-in-file-manager; that is
        // decided by open_content_in_tab based on the path.
        assert!(!editor.reveal_in_file_manager_enabled());
    }

    #[test]
    fn test_open_content_in_tab_reuses_empty_untouched_active_tab() {
        let (mut app, _) = DemoApp::new();
        // The initial tab created by `DemoApp::new()` has template content,
        // so it is not eligible for reuse. `NewTab` produces a genuinely
        // empty, clean tab.
        let _ = app.update(Message::NewTab);
        let active_id = app.active_tab_id;
        let tab_count_before = app.tabs.len();

        let target = app.open_content_in_tab(None, "");
        assert_eq!(target, active_id);
        assert_eq!(app.tabs.len(), tab_count_before);
    }

    #[test]
    fn test_open_content_in_tab_creates_new_tab_when_active_tab_is_dirty() {
        let (mut app, _) = DemoApp::new();
        // Start from a genuinely empty tab, then mark it dirty: it must no
        // longer be eligible for reuse despite having no content.
        let _ = app.update(Message::NewTab);
        let active_id = app.active_tab_id;
        if let Some(tab) = app.get_active_tab() {
            tab.is_dirty = true;
        }
        let tab_count_before = app.tabs.len();

        let target = app.open_content_in_tab(None, "");
        assert_ne!(target, active_id);
        assert_eq!(app.tabs.len(), tab_count_before + 1);
    }

    #[test]
    fn test_syntax_for_path_uses_lowercased_extension() {
        assert_eq!(
            DemoApp::syntax_for_path(Some(Path::new("/tmp/a.rs"))),
            "rs"
        );
        assert_eq!(
            DemoApp::syntax_for_path(Some(Path::new("/tmp/A.RS"))),
            "rs"
        );
    }

    #[test]
    fn test_an_untitled_tab_gets_the_template_language() {
        // No file behind it, so the content is one of the demo's Lua
        // templates.
        assert_eq!(DemoApp::syntax_for_path(None), DemoApp::UNTITLED_SYNTAX);
    }

    #[test]
    fn test_a_file_without_an_extension_is_plain_text_not_lua() {
        // A real file whose language is unknown. Colouring these as Lua both
        // misrendered them and made the status bar claim "Lua" once the active
        // grammar was displayed there.
        for name in ["Makefile", "LICENSE", ".bashrc", "Dockerfile"] {
            assert_eq!(
                DemoApp::syntax_for_path(Some(
                    &PathBuf::from("/tmp").join(name)
                )),
                DemoApp::PLAIN_TEXT_SYNTAX,
                "{name} has no extension to guess a language from"
            );
        }
    }

    #[test]
    fn test_a_file_without_an_extension_reports_plain_text_in_the_status_bar() {
        // End to end: the identifier has to be one the highlighter actually
        // resolves, not merely a different string from "lua".
        let (mut app, _) = DemoApp::new();

        let tab = app.open_content_in_tab(
            Some(&PathBuf::from("/tmp/Makefile")),
            "all:\n\tcargo build\n",
        );

        assert!(
            app.get_tab(tab)
                .is_some_and(|tab| tab.editor.syntax_name() == "Plain Text")
        );
    }

    #[test]
    fn test_open_content_in_tab_sets_syntax_from_path() {
        let (mut app, _) = DemoApp::new();

        let rust_tab = app.open_content_in_tab(
            Some(&PathBuf::from("/tmp/x.rs")),
            "fn main() {}",
        );
        assert!(
            app.get_tab(rust_tab)
                .is_some_and(|tab| tab.editor.syntax() == "rs")
        );

        // Reopening without a path returns to the untitled default rather than
        // keeping the previous file's language.
        let untitled_tab = app.open_content_in_tab(None, "");
        assert!(app.get_tab(untitled_tab).is_some_and(|tab| {
            tab.editor.syntax() == DemoApp::UNTITLED_SYNTAX
        }));
    }

    #[test]
    fn test_open_content_in_tab_enables_reveal_only_with_path() {
        let (mut app, _) = DemoApp::new();
        let path = PathBuf::from("/tmp/iced-code-editor/open-content.lua");

        let with_path = app.open_content_in_tab(Some(&path), "content");
        assert!(
            app.get_tab(with_path)
                .is_some_and(|tab| tab.editor.reveal_in_file_manager_enabled())
        );

        let without_path = app.open_content_in_tab(None, "");
        assert!(
            app.get_tab(without_path).is_some_and(|tab| !tab
                .editor
                .reveal_in_file_manager_enabled())
        );
    }
}
