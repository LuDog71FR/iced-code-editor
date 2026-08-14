// Imports for LSP (Language Server Protocol) functionality
use super::{DemoApp, EditorId, LspProgress, Template};
use crate::app::Message;

/// Delay in milliseconds before a hover request is sent after the cursor stops.
const LSP_HOVER_REQUEST_DELAY_MS: u64 = 400;
use iced::Point;
use iced::Task;
use iced::widget::Id;
use iced::widget::operation::scroll_to;
use iced::widget::scrollable;
use iced_code_editor::{
    LspDocument, LspEvent, LspLanguage, LspPosition, LspProcessClient,
    Message as EditorMessage, lsp_language_for_extension,
    lsp_language_for_path,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use url::Url;

/// Returns the LSP language for a built-in template (all use Lua).
fn lsp_language_for_template(template: Template) -> Option<LspLanguage> {
    lsp_language_for_extension(match template {
        Template::Empty
        | Template::HelloWorld
        | Template::Fibonacci
        | Template::Factorial => "lua",
    })
}

/// Represents a pending hover request that is waiting to be processed
#[derive(Clone, Copy)]
pub(super) struct LspHoverPending {
    /// The editor where the hover request originated
    pub(super) editor_id: EditorId,
    /// The position in the document where the hover was requested
    pub(super) position: LspPosition,
    /// The screen coordinates where the hover tooltip should appear
    pub(super) point: Point,
    /// The time when this hover request should be executed (after delay)
    pub(super) ready_at: Instant,
}

/// Converts an EditorId to a string label for use in URIs
fn editor_id_label(editor_id: EditorId) -> String {
    format!("editor_{}", editor_id.0)
}

/// Creates a virtual URI for a template file that doesn't exist on disk
fn virtual_uri_for_template(editor_id: EditorId, template: Template) -> String {
    let mut name = template.name().to_lowercase();
    name = name.replace(' ', "_");
    if name.is_empty() {
        name = "untitled".to_string();
    }
    format!("untitled://{}/{}.lua", editor_id_label(editor_id), name)
}

/// Converts a filesystem path to a `file://` URI.
///
/// Percent-encoding is delegated to [`Url`], so every character that is
/// reserved in a URI (space, `#`, `?`, `%`, non-ASCII) is encoded and survives
/// the round-trip through [`file_uri_to_path`].
///
/// [`Url::from_file_path`] requires an absolute path. A relative one falls
/// back to plain concatenation, which is what the language server received in
/// every case before.
fn path_to_file_uri(path: &Path) -> String {
    Url::from_file_path(path).map_or_else(
        |()| format!("file://{}", path.display()),
        |uri| uri.into(),
    )
}

/// Converts a `file://` URI back to a filesystem path.
///
/// Returns `None` when the URI does not use the `file` scheme or does not map
/// to a path on this platform. Percent-encoded sequences are decoded, which a
/// bare `strip_prefix("file://")` would leave in the path as literal `%20`.
fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    Url::parse(uri).ok()?.to_file_path().ok()
}

impl DemoApp {
    /// Applies a completion item by inserting the text at the current cursor position
    /// and replacing the current word being typed
    pub(super) fn apply_completion(&mut self, completion_text: &str) {
        if let Some(tab) =
            self.tabs.iter_mut().find(|t| t.id == self.active_tab_id)
        {
            let content = tab.editor.content();
            let (line, col) = tab.editor.cursor_position();

            // Find the start of the current word
            let line_content = content.lines().nth(line).unwrap_or("");
            let word_start_col = Self::find_word_start(line_content, col);

            // Calculate how many characters to delete
            let chars_to_delete = col - word_start_col;

            // Delete the current word being typed and insert the completion
            for _ in 0..chars_to_delete {
                let _ = tab.editor.update(&EditorMessage::Backspace);
            }

            // Insert the completion text character by character
            for ch in completion_text.chars() {
                let _ = tab.editor.update(&EditorMessage::CharacterInput(ch));
            }

            tab.is_dirty = tab.editor.is_modified();
            self.log(
                "INFO",
                &format!("Applied completion: {}", completion_text),
            );
        }
    }

    /// Returns the word currently being typed, ending at `cursor_col`.
    ///
    /// `cursor_col` is a character offset, as reported by
    /// `CodeEditor::cursor_position`. The word is rebuilt from `chars()`
    /// rather than sliced by byte index, so lines holding multi-byte
    /// characters (accents, CJK, emoji) are handled correctly.
    ///
    /// Returns an empty string when no word character precedes the cursor.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(DemoApp::current_word_at("foo.ba", 6), "ba");
    /// assert_eq!(DemoApp::current_word_at("héllo", 5), "héllo");
    /// ```
    pub(super) fn current_word_at(line: &str, cursor_col: usize) -> String {
        let word_start = Self::find_word_start(line, cursor_col);
        line.chars()
            .skip(word_start)
            .take(cursor_col.saturating_sub(word_start))
            .collect()
    }

    /// Finds the start column of the current word being typed
    pub(super) fn find_word_start(line: &str, cursor_col: usize) -> usize {
        let chars: Vec<char> = line.chars().collect();
        let mut word_start = cursor_col;

        // Move backwards to find the start of the word
        while word_start > 0 {
            let ch = chars.get(word_start - 1).copied().unwrap_or(' ');
            if !ch.is_alphanumeric() && ch != '_' {
                break;
            }
            word_start -= 1;
        }

        word_start
    }

    /// Gets the LSP server key for the specified editor
    pub(super) fn lsp_server_for_editor(
        &self,
        editor_id: EditorId,
    ) -> Option<&'static str> {
        self.tabs.iter().find(|t| t.id == editor_id)?.lsp_server_key
    }

    /// Sets the LSP server key for the specified editor
    pub(super) fn set_lsp_server_for_editor(
        &mut self,
        editor_id: EditorId,
        server: Option<&'static str>,
    ) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == editor_id) {
            tab.lsp_server_key = server;
        }
    }

    /// Detaches the LSP client from the specified editor
    pub(super) fn detach_lsp_for_editor(&mut self, editor_id: EditorId) {
        if let Some(editor) = self.get_editor(editor_id) {
            editor.detach_lsp();
        }
        self.set_lsp_server_for_editor(editor_id, None);
    }

    /// Determines the root URI for LSP based on a path hint
    /// Falls back to current working directory if the path is not within it
    pub(super) fn lsp_root_uri_for_path(
        root_hint: Option<&Path>,
    ) -> Option<String> {
        let cwd = std::env::current_dir().ok();
        let root_dir = root_hint
            .and_then(|path| {
                if path.is_dir() {
                    Some(path.to_path_buf())
                } else {
                    path.parent().map(PathBuf::from)
                }
            })
            .map(|hint_dir| {
                if let Some(cwd) = &cwd
                    && hint_dir.starts_with(cwd)
                {
                    cwd.clone()
                } else {
                    hint_dir
                }
            })
            .or(cwd)?;
        Some(path_to_file_uri(&root_dir))
    }

    /// Synchronizes LSP for a file path, detecting the language automatically
    pub(super) fn sync_lsp_for_path(
        &mut self,
        editor_id: EditorId,
        path: &Path,
    ) -> bool {
        let Some(language) = lsp_language_for_path(path) else {
            self.detach_lsp_for_editor(editor_id);
            return false;
        };
        let uri = path_to_file_uri(path);
        self.sync_lsp_for_language(editor_id, language, uri, Some(path))
    }

    /// Synchronizes LSP for a template (untitled document)
    pub(super) fn sync_lsp_for_template(
        &mut self,
        editor_id: EditorId,
        template: Template,
    ) -> bool {
        let Some(language) = lsp_language_for_template(template) else {
            self.detach_lsp_for_editor(editor_id);
            return false;
        };
        let uri = virtual_uri_for_template(editor_id, template);
        self.sync_lsp_for_language(editor_id, language, uri, None)
    }

    /// Synchronizes LSP for the given editor, using its file path or syntax.
    ///
    /// - If the tab has a `file_path`, delegates to [`sync_lsp_for_path`].
    /// - If the tab is untitled, detects the language from the editor's syntax
    ///   and uses a virtual URI of the form `untitled://{id}/untitled.{syntax}`.
    ///
    /// Returns `true` if an LSP server was successfully attached.
    ///
    /// [`sync_lsp_for_path`]: Self::sync_lsp_for_path
    pub(super) fn sync_lsp_for_editor(&mut self, editor_id: EditorId) -> bool {
        let file_path =
            self.get_tab(editor_id).and_then(|tab| tab.file_path.clone());

        if let Some(path) = file_path {
            return self.sync_lsp_for_path(editor_id, &path);
        }

        let syntax = self.get_editor(editor_id).map(|e| e.syntax().to_string());

        let Some(syntax) = syntax else {
            return false;
        };

        let Some(language) = lsp_language_for_extension(&syntax) else {
            self.detach_lsp_for_editor(editor_id);
            return false;
        };

        let uri = format!(
            "untitled://{}/untitled.{}",
            editor_id_label(editor_id),
            syntax
        );
        self.sync_lsp_for_language(editor_id, language, uri, None)
    }

    /// Synchronizes LSP for a specific language
    /// Reuses existing LSP server if compatible, otherwise creates a new one
    pub(super) fn sync_lsp_for_language(
        &mut self,
        editor_id: EditorId,
        language: LspLanguage,
        uri: String,
        root_hint: Option<&Path>,
    ) -> bool {
        // If the correct LSP server is already attached, just open a new document
        if self.lsp_server_for_editor(editor_id) == Some(language.server_key) {
            if let Some(editor) = self.get_editor(editor_id) {
                editor.lsp_open_document(LspDocument::new(
                    uri,
                    language.language_id,
                ));
                return true;
            }
            self.log("ERROR", "Editor not found for LSP document");
            self.set_lsp_server_for_editor(editor_id, None);
            return false;
        }

        // Check if we have an event sender for LSP communication
        let Some(sender) = self.lsp_event_sender.as_ref().cloned() else {
            self.detach_lsp_for_editor(editor_id);
            return false;
        };

        // Determine the root URI for the LSP server
        let Some(root_uri) = Self::lsp_root_uri_for_path(root_hint) else {
            self.log("ERROR", "LSP failed: root uri unavailable");
            self.detach_lsp_for_editor(editor_id);
            return false;
        };

        // Detach any existing LSP and create a new one
        self.detach_lsp_for_editor(editor_id);
        match LspProcessClient::new_with_server(
            &root_uri,
            sender,
            language.server_key,
        ) {
            Ok(client) => {
                let Some(editor) = self.get_editor(editor_id) else {
                    self.log("ERROR", "Editor not found for LSP attach");
                    self.set_lsp_server_for_editor(editor_id, None);
                    return false;
                };
                editor.attach_lsp(
                    Box::new(client),
                    LspDocument::new(uri, language.language_id),
                );
                self.set_lsp_server_for_editor(
                    editor_id,
                    Some(language.server_key),
                );
                true
            }
            Err(err) => {
                self.log("ERROR", &format!("LSP failed: {}", err));
                self.set_lsp_server_for_editor(editor_id, None);
                false
            }
        }
    }

    /// Handles mouse-triggered hover requests.
    ///
    /// Implements hover delay and interactive hover dismissal logic.
    pub(super) fn handle_lsp_hover_from_mouse(
        &mut self,
        editor_id: EditorId,
        point: Point,
    ) {
        // If hover is interactive (mouse is over the tooltip), check if we should dismiss
        if self.lsp_overlay.hover_interactive {
            if !self.lsp_overlay.hover_visible
                || self.lsp_overlay_editor != Some(editor_id)
            {
                self.lsp_overlay.hover_interactive = false;
                self.lsp_hover_hide_deadline = None;
                self.lsp_hover_pending = None;
            } else {
                return;
            }
        }

        // Find the text position at the mouse point
        let anchor =
            if let Some(tab) = self.tabs.iter().find(|t| t.id == editor_id) {
                tab.editor.lsp_hover_anchor_at_point(point)
            } else {
                None
            };

        let Some((position, anchor_point)) = anchor else {
            // No valid anchor point - schedule hide if hover is visible
            if self.lsp_overlay.hover_visible
                && self.lsp_overlay_editor == Some(editor_id)
            {
                return;
            }
            if self.lsp_overlay.hover_visible {
                self.lsp_hover_hide_deadline = Some(
                    Instant::now()
                        + Duration::from_millis(LSP_HOVER_REQUEST_DELAY_MS),
                );
            }
            return;
        };

        // Skip if hovering over the same position
        if let Some((last_editor, last_position)) = self.lsp_hover_anchor
            && last_editor == editor_id
            && last_position.line == position.line
            && last_position.character == position.character
        {
            return;
        }

        // Schedule a new hover request with a delay
        self.lsp_hover_anchor = Some((editor_id, position));
        self.lsp_overlay.hover_interactive = false;
        self.lsp_hover_pending = Some(LspHoverPending {
            editor_id,
            position,
            point: anchor_point,
            ready_at: Instant::now()
                + Duration::from_millis(LSP_HOVER_REQUEST_DELAY_MS),
        });
        self.lsp_hover_hide_deadline = None;
    }

    /// Processes hover-related timers (pending requests and hide deadlines).
    ///
    /// Should be called periodically to trigger delayed hover requests and auto-hide.
    pub(super) fn process_lsp_hover_timers(&mut self) {
        let now = Instant::now();

        // Clear hover if visible but no editor is associated
        if self.lsp_overlay.hover_visible && self.lsp_overlay_editor.is_none() {
            self.clear_lsp_hover();
        }

        // Process pending hover request if the delay has passed
        if let Some(pending) = self.lsp_hover_pending.take() {
            if now >= pending.ready_at {
                // Send hover request to the LSP server
                let request_sent = if let Some(tab) =
                    self.tabs.iter_mut().find(|t| t.id == pending.editor_id)
                {
                    tab.editor.lsp_flush_pending_changes();
                    tab.editor.lsp_request_hover_at_position(pending.position)
                } else {
                    false
                };

                if request_sent {
                    self.lsp_overlay.set_hover_position(pending.point);
                    self.lsp_overlay_editor = Some(pending.editor_id);
                } else {
                    self.lsp_hover_anchor = None;
                }
            } else {
                // Not ready yet, put it back
                self.lsp_hover_pending = Some(pending);
            }
        }

        // Check if we should auto-hide the hover tooltip
        if let Some(deadline) = self.lsp_hover_hide_deadline
            && now >= deadline
            && !self.lsp_overlay.hover_interactive
        {
            self.clear_lsp_hover();
        }
    }

    /// Clears all hover-related state.
    pub(super) fn clear_lsp_hover(&mut self) {
        self.lsp_overlay.clear_hover();
        self.lsp_hover_anchor = None;
        self.lsp_hover_pending = None;
        self.lsp_hover_hide_deadline = None;

        // Only clear overlay editor if completion is not visible
        if !self.lsp_overlay.completion_visible {
            self.lsp_overlay_editor = None;
        }
    }

    /// Navigates the completion list by `direction` steps and scrolls to the selection.
    ///
    /// Pass `-1` for up and `1` for down. Does nothing when the menu is hidden or empty.
    pub(super) fn navigate_completion(
        &mut self,
        direction: i32,
    ) -> Task<Message> {
        if self.lsp_overlay.completion_visible
            && !self.lsp_overlay.completion_items.is_empty()
        {
            self.lsp_overlay.navigate(direction);
            let scroll_y = self.lsp_overlay.scroll_offset_for_selected();
            return scroll_to(
                Id::new("completion_scrollable"),
                scrollable::AbsoluteOffset { x: 0.0, y: scroll_y },
            );
        }
        Task::none()
    }

    /// Clears `lsp_overlay_editor` when the hover tooltip is no longer visible.
    pub(super) fn clear_overlay_editor_if_no_hover(&mut self) {
        if !self.lsp_overlay.hover_visible {
            self.lsp_overlay_editor = None;
        }
    }

    /// Drains and processes all pending LSP events from the event channel
    /// Handles hover responses and completion items from the LSP server
    pub(super) fn drain_lsp_events(&mut self) -> Task<Message> {
        let Some(receiver) = self.lsp_events.take() else {
            return Task::none();
        };
        let receiver = receiver;
        let mut messages = Vec::new();

        loop {
            match receiver.try_recv() {
                Ok(event) => match event {
                    // Handle hover response from LSP server
                    LspEvent::Hover { text } => {
                        if text.trim().is_empty() {
                            self.clear_lsp_hover();
                        } else {
                            self.lsp_overlay.show_hover(text);
                            self.lsp_hover_hide_deadline = None;
                            if self.lsp_overlay_editor.is_none() {
                                self.lsp_overlay_editor =
                                    Some(self.active_tab_id);
                            }
                        }
                    }
                    // Handle completion response from LSP server
                    LspEvent::Completion { items } => {
                        // Record cursor position for menu placement
                        let position = self
                            .tabs
                            .iter()
                            .find(|t| t.id == self.active_tab_id)
                            .and_then(|tab| tab.editor.cursor_screen_position())
                            .unwrap_or(iced::Point::new(4.0, 4.0));

                        self.lsp_overlay.set_completions(items, position);

                        if self.lsp_overlay_editor.is_none()
                            && self.lsp_overlay.completion_visible
                        {
                            self.lsp_overlay_editor = Some(self.active_tab_id);
                        }
                    }
                    // Handle definition response from LSP server
                    LspEvent::Definition { uri, range } => {
                        if let Some(path) = file_uri_to_path(&uri) {
                            messages.push(Message::JumpToFile(
                                path,
                                range.start.line as usize,
                                range.start.character as usize,
                            ));
                        }
                    }
                    // Handle progress notification from LSP server
                    LspEvent::Progress {
                        token,
                        server_key,
                        title,
                        message,
                        percentage,
                        done,
                    } => {
                        if done {
                            if let Some(map) =
                                self.lsp_progress.get_mut(&server_key)
                            {
                                map.remove(&token);
                                if map.is_empty() {
                                    self.lsp_progress.remove(&server_key);
                                }
                            }
                        } else {
                            self.lsp_progress
                                .entry(server_key)
                                .or_default()
                                .insert(
                                    token,
                                    LspProgress { title, message, percentage },
                                );
                        }
                    }
                    LspEvent::Log { server_key, message } => {
                        self.log(
                            "LSP",
                            &format!("[{}] {}", server_key, message),
                        );
                    }
                },
                // No more events available right now
                Err(mpsc::TryRecvError::Empty) => {
                    self.lsp_events = Some(receiver);
                    break;
                }
                // LSP process has disconnected
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.lsp_events = None;
                    break;
                }
            }
        }

        if messages.is_empty() {
            Task::none()
        } else {
            Task::batch(
                messages
                    .into_iter()
                    .map(|msg| Task::perform(async move { msg }, |m| m)),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    // A handful of tests below carry an `#[allow(clippy::unwrap_used)]` on
    // `mpsc::Sender::send(..).unwrap()`. The channel is created and drained
    // within the same test, so a failed send means the test setup itself is
    // broken — a panic is the right failure report there, matching the
    // existing per-test allows in `history.rs`/`command.rs`.
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn test_file_uri_round_trip() {
        // `#` and `?` are legal in POSIX file names but reserved in a URI.
        for path in
            ["/tmp/simple.rs", "/tmp/mon dossier/héllo.rs", "/tmp/a#b?c/d.rs"]
        {
            let original = PathBuf::from(path);
            let uri = path_to_file_uri(&original);
            assert_eq!(
                file_uri_to_path(&uri),
                Some(original),
                "round-trip failed for {uri}"
            );
        }
    }

    #[test]
    fn test_file_uri_percent_encodes_reserved_characters() {
        let uri = path_to_file_uri(Path::new("/tmp/mon dossier/a.rs"));
        assert!(uri.contains("%20"), "space must be encoded, got {uri}");
    }

    #[test]
    fn test_file_uri_unchanged_for_plain_ascii_path() {
        // The common case must keep producing exactly what it did before.
        assert_eq!(
            path_to_file_uri(Path::new("/home/user/demo.lua")),
            "file:///home/user/demo.lua"
        );
    }

    #[test]
    fn test_file_uri_to_path_rejects_other_schemes() {
        // Template buffers use `untitled://`; they have no filesystem path.
        assert_eq!(file_uri_to_path("untitled://editor_0/untitled.lua"), None);
        assert_eq!(file_uri_to_path("not a uri"), None);
    }

    #[test]
    fn test_current_word_at_ascii() {
        assert_eq!(DemoApp::current_word_at("foo.ba", 6), "ba");
        assert_eq!(DemoApp::current_word_at("let value", 9), "value");
        assert_eq!(DemoApp::current_word_at("snake_case", 10), "snake_case");
    }

    #[test]
    fn test_current_word_at_multibyte() {
        // Regression: character offsets must never be used as byte offsets.
        // These two sliced mid-character and panicked before the fix.
        assert_eq!(DemoApp::current_word_at("aé", 2), "aé");
        assert_eq!(DemoApp::current_word_at("汉字", 2), "汉字");

        // These landed on char boundaries but returned a truncated word.
        assert_eq!(DemoApp::current_word_at("héllo", 5), "héllo");
        assert_eq!(DemoApp::current_word_at("héllo wor", 9), "wor");
        assert_eq!(DemoApp::current_word_at("汉字 ab", 5), "ab");
    }

    #[test]
    fn test_current_word_at_without_word() {
        assert_eq!(DemoApp::current_word_at("foo ", 4), "");
        assert_eq!(DemoApp::current_word_at("", 0), "");
        assert_eq!(DemoApp::current_word_at("a.", 2), "");
    }

    // ---- drain_lsp_events ----

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_drain_lsp_events_shows_hover_text() {
        let (mut app, _) = DemoApp::new();
        let (tx, rx) = mpsc::channel();
        app.lsp_events = Some(rx);
        tx.send(LspEvent::Hover { text: "docs".to_string() }).unwrap();

        let _ = app.drain_lsp_events();

        assert!(app.lsp_overlay.hover_visible);
        assert_eq!(app.lsp_overlay_editor, Some(app.active_tab_id));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_drain_lsp_events_empty_hover_clears_overlay() {
        let (mut app, _) = DemoApp::new();
        app.lsp_overlay.show_hover("stale".to_string());
        app.lsp_hover_anchor =
            Some((app.active_tab_id, LspPosition { line: 0, character: 0 }));
        let (tx, rx) = mpsc::channel();
        app.lsp_events = Some(rx);
        tx.send(LspEvent::Hover { text: String::new() }).unwrap();

        let _ = app.drain_lsp_events();

        assert!(!app.lsp_overlay.hover_visible);
        assert!(app.lsp_hover_anchor.is_none());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_drain_lsp_events_completion_shows_overlay() {
        let (mut app, _) = DemoApp::new();
        let (tx, rx) = mpsc::channel();
        app.lsp_events = Some(rx);
        tx.send(LspEvent::Completion {
            items: vec!["foo".to_string(), "bar".to_string()],
        })
        .unwrap();

        let _ = app.drain_lsp_events();

        assert!(app.lsp_overlay.completion_visible);
        assert_eq!(app.lsp_overlay_editor, Some(app.active_tab_id));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_drain_lsp_events_progress_done_removes_entry() {
        let (mut app, _) = DemoApp::new();
        app.lsp_progress
            .entry("rust-analyzer".to_string())
            .or_default()
            .insert(
                "token-1".to_string(),
                LspProgress {
                    title: "Indexing".to_string(),
                    message: None,
                    percentage: Some(50),
                },
            );
        let (tx, rx) = mpsc::channel();
        app.lsp_events = Some(rx);
        tx.send(LspEvent::Progress {
            token: "token-1".to_string(),
            server_key: "rust-analyzer".to_string(),
            title: "Indexing".to_string(),
            message: None,
            percentage: None,
            done: true,
        })
        .unwrap();

        let _ = app.drain_lsp_events();

        assert!(!app.lsp_progress.contains_key("rust-analyzer"));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_drain_lsp_events_log_appends_message() {
        let (mut app, _) = DemoApp::new();
        let (tx, rx) = mpsc::channel();
        app.lsp_events = Some(rx);
        tx.send(LspEvent::Log {
            server_key: "gopls".to_string(),
            message: "started".to_string(),
        })
        .unwrap();

        let _ = app.drain_lsp_events();

        assert_eq!(
            app.log_messages.last().map(String::as_str),
            Some("[LSP] [gopls] started")
        );
    }

    #[test]
    fn test_drain_lsp_events_disconnected_sender_clears_receiver() {
        let (mut app, _) = DemoApp::new();
        let (tx, rx) = mpsc::channel::<LspEvent>();
        app.lsp_events = Some(rx);
        drop(tx);

        let _ = app.drain_lsp_events();

        assert!(app.lsp_events.is_none());
    }

    // ---- process_lsp_hover_timers ----

    #[test]
    fn test_process_lsp_hover_timers_clears_pending_when_ready() {
        let (mut app, _) = DemoApp::new();
        let editor_id = app.active_tab_id;
        let position = LspPosition { line: 0, character: 0 };
        app.lsp_hover_anchor = Some((editor_id, position));
        app.lsp_hover_pending = Some(LspHoverPending {
            editor_id,
            position,
            point: Point::new(10.0, 10.0),
            ready_at: Instant::now() - Duration::from_millis(1),
        });

        app.process_lsp_hover_timers();

        assert!(app.lsp_hover_pending.is_none());
        // A fresh DemoApp has no LSP client attached, so the request cannot
        // be sent and the stale anchor is cleared instead of showing the
        // tooltip.
        assert!(app.lsp_hover_anchor.is_none());
    }

    #[test]
    fn test_process_lsp_hover_timers_keeps_pending_before_ready() {
        let (mut app, _) = DemoApp::new();
        let editor_id = app.active_tab_id;
        let position = LspPosition { line: 0, character: 0 };
        app.lsp_hover_pending = Some(LspHoverPending {
            editor_id,
            position,
            point: Point::new(10.0, 10.0),
            ready_at: Instant::now() + Duration::from_secs(60),
        });

        app.process_lsp_hover_timers();

        assert!(app.lsp_hover_pending.is_some());
    }

    #[test]
    fn test_process_lsp_hover_timers_hides_overlay_after_deadline() {
        let (mut app, _) = DemoApp::new();
        app.lsp_overlay.show_hover("docs".to_string());
        app.lsp_overlay_editor = Some(app.active_tab_id);
        app.lsp_hover_anchor =
            Some((app.active_tab_id, LspPosition { line: 0, character: 0 }));
        app.lsp_hover_hide_deadline =
            Some(Instant::now() - Duration::from_millis(1));

        app.process_lsp_hover_timers();

        assert!(!app.lsp_overlay.hover_visible);
        assert!(app.lsp_hover_anchor.is_none());
        assert!(app.lsp_hover_hide_deadline.is_none());
    }

    #[test]
    fn test_process_lsp_hover_timers_keeps_overlay_while_interactive() {
        let (mut app, _) = DemoApp::new();
        app.lsp_overlay.show_hover("docs".to_string());
        app.lsp_overlay.hover_interactive = true;
        app.lsp_overlay_editor = Some(app.active_tab_id);
        app.lsp_hover_hide_deadline =
            Some(Instant::now() - Duration::from_millis(1));

        app.process_lsp_hover_timers();

        assert!(app.lsp_overlay.hover_visible);
    }
}
