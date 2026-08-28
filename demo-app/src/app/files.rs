//! File open/save/reveal and LSP jump-to-definition handling for
//! [`DemoApp`].
//!
//! Wraps the async dialog/disk operations in [`crate::file_ops`] with the
//! tab bookkeeping (dirty flag, reveal-in-file-manager policy, LSP sync)
//! each one needs.

use super::{DemoApp, Message};
use crate::file_ops;
use iced::Task;
use iced_code_editor::theme;
use std::path::{Path, PathBuf};

#[cfg(not(target_arch = "wasm32"))]
use crate::types::EditorId;

impl DemoApp {
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
    pub(super) fn handle_reveal_in_file_manager(
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
    pub(super) fn handle_file_revealed(
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
    pub(super) fn handle_file_open(&mut self) -> Task<Message> {
        self.log(
            "INFO",
            &format!("Opening file for {:?} editor...", self.active_tab_id),
        );
        Task::perform(file_ops::open_file_dialog(), Message::FileOpened)
    }

    /// Handles the result of a file open operation.
    pub(super) fn handle_file_opened(
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
                let target_tab_id =
                    self.open_content_in_tab(Some(&path), &content);

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
    pub(super) fn handle_file_save(
        &mut self,
        editor_id: EditorId,
    ) -> Task<Message> {
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
    pub(super) fn handle_file_save_as(
        &mut self,
        editor_id: EditorId,
    ) -> Task<Message> {
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
    pub(super) fn handle_file_saved(
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
                // A "Save As" can change the extension, so the highlighting
                // has to follow the new name.
                editor.set_syntax(&DemoApp::syntax_for_path(Some(&path)));
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

    /// Returns `true` if `path` lies within the current working directory.
    ///
    /// `Definition` responses used for jump-to-file come from the language
    /// server process, which is untrusted input: a malicious or compromised
    /// server could otherwise point [`Message::JumpToFile`] at any
    /// filesystem-readable path (e.g. an SSH private key). Confining jump
    /// targets to the workspace root prevents that.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let cwd = std::env::current_dir().unwrap();
    /// assert!(DemoApp::is_lsp_jump_target_allowed(&cwd.join("src/main.rs")));
    /// assert!(!DemoApp::is_lsp_jump_target_allowed(Path::new("/etc/passwd")));
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    fn is_lsp_jump_target_allowed(path: &Path) -> bool {
        let Ok(cwd) = std::env::current_dir() else {
            return false;
        };
        let root = cwd.canonicalize().unwrap_or(cwd);
        // Fail closed: a target that can't be canonicalized (e.g. a `..`
        // traversal through a component that doesn't exist) can't be opened
        // anyway, and falling back to a lexical (uncanonicalized) path would
        // let a traversal like `<cwd>/missing/../../etc/passwd` pass the
        // `starts_with` check below on its literal components alone.
        let Ok(candidate) = path.canonicalize() else {
            return false;
        };
        candidate.starts_with(&root)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn handle_jump_to_file(
        &mut self,
        path: PathBuf,
        line: usize,
        col: usize,
    ) -> Task<Message> {
        if !Self::is_lsp_jump_target_allowed(&path) {
            self.log(
                "WARN",
                &format!(
                    "Ignored LSP jump-to-definition outside workspace: {}",
                    path.display()
                ),
            );
            return Task::none();
        }

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
    pub(super) fn handle_file_opened_and_jump(
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
                let target_tab_id =
                    self.open_content_in_tab(Some(&path), &content);

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_lsp_jump_target_allowed_confines_to_workspace_root() {
        // `current_dir()` is expected to be available in the test runner;
        // skip rather than fail hard if the sandbox lacks it.
        let Ok(cwd) = std::env::current_dir() else {
            return;
        };
        assert!(DemoApp::is_lsp_jump_target_allowed(&cwd.join("Cargo.toml")));
        assert!(!DemoApp::is_lsp_jump_target_allowed(Path::new("/etc/passwd")));
    }

    #[test]
    fn test_is_lsp_jump_target_allowed_rejects_traversal_through_missing_dir() {
        let Ok(cwd) = std::env::current_dir() else {
            return;
        };
        // The leading component doesn't exist, so `canonicalize()` fails —
        // but the path still lexically starts with `cwd`, since its `..`
        // components only walk back out on a real filesystem. A fallback to
        // the uncanonicalized path here would wrongly allow this traversal.
        let traversal = cwd.join("nonexistent_subdir_xyz/../../etc/passwd");
        assert!(!DemoApp::is_lsp_jump_target_allowed(&traversal));
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
