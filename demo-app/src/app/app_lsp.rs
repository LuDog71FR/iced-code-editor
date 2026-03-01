// Imports for LSP (Language Server Protocol) functionality
use super::{DemoApp, EditorId, Template};
use crate::lsp_config::{
    LspLanguage, lsp_language_for_path, lsp_language_for_template,
};
use crate::lsp_process_client::{LspEvent, LspProcessClient};
use iced::Point;
use iced::widget::text_editor;
use iced_code_editor::{LspDocument, LspPosition};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

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
fn editor_id_label(editor_id: EditorId) -> &'static str {
    match editor_id {
        EditorId::Left => "left",
        EditorId::Right => "right",
    }
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

/// Converts a filesystem path to a file:// URI
fn path_to_file_uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    let path_str = path.to_string_lossy().replace(' ', "%20");
    uri.push_str(&path_str);
    uri
}

impl DemoApp {
    /// Gets the LSP server key for the specified editor
    pub(super) fn lsp_server_for_editor(
        &self,
        editor_id: EditorId,
    ) -> Option<&'static str> {
        match editor_id {
            EditorId::Left => self.lsp_server_left,
            EditorId::Right => self.lsp_server_right,
        }
    }

    /// Sets the LSP server key for the specified editor
    pub(super) fn set_lsp_server_for_editor(
        &mut self,
        editor_id: EditorId,
        server: Option<&'static str>,
    ) {
        match editor_id {
            EditorId::Left => self.lsp_server_left = server,
            EditorId::Right => self.lsp_server_right = server,
        }
    }

    /// Detaches the LSP client from the specified editor
    pub(super) fn detach_lsp_for_editor(&mut self, editor_id: EditorId) {
        let editor = self.get_editor(editor_id);
        editor.detach_lsp();
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
            let editor = self.get_editor(editor_id);
            editor
                .lsp_open_document(LspDocument::new(uri, language.language_id));
            return true;
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
                let editor = self.get_editor(editor_id);
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

    /// Handles mouse-triggered hover requests
    /// Implements hover delay and interactive hover dismissal logic
    pub(super) fn handle_lsp_hover_from_mouse(
        &mut self,
        editor_id: EditorId,
        point: Point,
    ) {
        // If hover is interactive (mouse is over the tooltip), check if we should dismiss
        if self.lsp_hover_interactive {
            if !self.lsp_hover_visible
                || self.lsp_overlay_editor != Some(editor_id)
            {
                self.lsp_hover_interactive = false;
                self.lsp_hover_hide_deadline = None;
                self.lsp_hover_pending = None;
            } else {
                return;
            }
        }

        // Find the text position at the mouse point
        let anchor = match editor_id {
            EditorId::Left => self.editor_left.lsp_hover_anchor_at_point(point),
            EditorId::Right => {
                self.editor_right.lsp_hover_anchor_at_point(point)
            }
        };

        let Some((position, anchor_point)) = anchor else {
            // No valid anchor point - schedule hide if hover is visible
            if self.lsp_hover_visible
                && self.lsp_overlay_editor == Some(editor_id)
            {
                return;
            }
            if self.lsp_hover_visible {
                self.lsp_hover_hide_deadline =
                    Some(Instant::now() + Duration::from_millis(400));
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
        self.lsp_hover_interactive = false;
        self.lsp_hover_pending = Some(LspHoverPending {
            editor_id,
            position,
            point: anchor_point,
            ready_at: Instant::now() + Duration::from_millis(400),
        });
        self.lsp_hover_hide_deadline = None;
    }

    /// Processes hover-related timers (pending requests and hide deadlines)
    /// Should be called periodically to trigger delayed hover requests and auto-hide
    pub(super) fn process_lsp_hover_timers(&mut self) {
        let now = Instant::now();

        // Clear hover if visible but no editor is associated
        if self.lsp_hover_visible && self.lsp_overlay_editor.is_none() {
            self.clear_lsp_hover();
        }

        // Process pending hover request if the delay has passed
        if let Some(pending) = self.lsp_hover_pending.take() {
            if now >= pending.ready_at {
                // Send hover request to the LSP server
                let request_sent = match pending.editor_id {
                    EditorId::Left => {
                        self.editor_left.lsp_flush_pending_changes();
                        self.editor_left
                            .lsp_request_hover_at_position(pending.position)
                    }
                    EditorId::Right => {
                        self.editor_right.lsp_flush_pending_changes();
                        self.editor_right
                            .lsp_request_hover_at_position(pending.position)
                    }
                };

                if request_sent {
                    self.lsp_hover_position = Some(pending.point);
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
            && !self.lsp_hover_interactive
        {
            self.clear_lsp_hover();
        }
    }

    /// Clears all hover-related state
    pub(super) fn clear_lsp_hover(&mut self) {
        self.lsp_last_hover = None;
        self.lsp_hover_visible = false;
        self.lsp_hover_position = None;
        self.lsp_hover_anchor = None;
        self.lsp_hover_interactive = false;
        self.lsp_hover_pending = None;
        self.lsp_hover_hide_deadline = None;
        self.lsp_hover_content = text_editor::Content::with_text("");

        // Only clear overlay editor if completion is not visible
        if !self.lsp_completion_visible {
            self.lsp_overlay_editor = None;
        }
    }

    /// Drains and processes all pending LSP events from the event channel
    /// Handles hover responses and completion items from the LSP server
    pub(super) fn drain_lsp_events(&mut self) {
        let Some(receiver) = self.lsp_events.take() else { return };
        let receiver = receiver;

        loop {
            match receiver.try_recv() {
                Ok(event) => match event {
                    // Handle hover response from LSP server
                    LspEvent::Hover { text } => {
                        if text.trim().is_empty() {
                            self.clear_lsp_hover();
                        } else {
                            self.lsp_last_hover = Some(text);
                            if let Some(hover) = self.lsp_last_hover.as_ref() {
                                self.lsp_hover_content =
                                    text_editor::Content::with_text(hover);
                            }
                            self.lsp_hover_visible = true;
                            self.lsp_hover_hide_deadline = None;
                            if self.lsp_overlay_editor.is_none() {
                                self.lsp_overlay_editor =
                                    Some(self.active_editor);
                            }
                        }
                    }
                    // Handle completion response from LSP server
                    LspEvent::Completion { items } => {
                        self.lsp_last_completion = items;
                        self.lsp_completion_visible =
                            !self.lsp_last_completion.is_empty();
                        self.lsp_completion_selected = 0;
                        if self.lsp_overlay_editor.is_none()
                            && self.lsp_completion_visible
                        {
                            self.lsp_overlay_editor = Some(self.active_editor);
                        }
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
    }
}
