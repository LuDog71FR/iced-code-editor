//! Message handling for the command palette.

use iced::Task;
use iced::widget::operation::{focus, scroll_to};
use iced::widget::scrollable::AbsoluteOffset;

use super::dialog::{row_top, separator_after};
use super::{PaletteAction, PaletteEntry};
use crate::canvas_editor::{CodeEditor, Message};

impl CodeEditor {
    /// Opens the palette and focuses its input.
    ///
    /// The search and go-to-line dialogs are closed first: all three are
    /// modal input surfaces stacked over the same canvas, and only one can
    /// hold keyboard focus.
    ///
    /// Closing search is also why this is the one palette handler that clears
    /// `overlay_cache`: search match highlights are canvas geometry, so they
    /// have to be erased. The palette itself is a widget in the view tree,
    /// which the cache knows nothing about.
    pub(crate) fn handle_open_command_palette_msg(&mut self) -> Task<Message> {
        if !self.command_palette_enabled {
            return Task::none();
        }

        self.search_state.close();
        self.goto_line_state.close();
        self.command_palette_state.open();
        self.overlay_cache.clear();

        let entries = self.command_palette_entries();
        Task::batch([
            focus(self.command_palette_state.input_id.clone()),
            self.scroll_command_palette_to_selection(&entries),
        ])
    }

    /// Closes the palette without running anything.
    ///
    /// Deliberately leaves `overlay_cache` alone, as does
    /// [`Self::handle_submit_command_palette_msg`]: nothing drawn into that
    /// layer — current-line highlight, search highlights, bracket match,
    /// selection, cursor, jump link — reads the palette's state, so clearing
    /// it would only force all of that geometry to be rebuilt on the next
    /// frame for no visible change.
    pub(crate) fn handle_close_command_palette_msg(&mut self) -> Task<Message> {
        self.command_palette_state.close();
        Task::none()
    }

    /// Applies a new filter and highlights the first matching command.
    ///
    /// The highlight resets rather than being kept at the same index: after
    /// a keystroke the row at that index is a different command, so keeping
    /// it would run something the user never looked at.
    pub(crate) fn handle_command_palette_changed_msg(
        &mut self,
        query: &str,
    ) -> Task<Message> {
        self.command_palette_state.query = query.to_string();
        self.command_palette_state.select_first_row();
        let entries = self.command_palette_entries();
        self.scroll_command_palette_to_selection(&entries)
    }

    /// Moves the highlight one row down (`forward`) or up.
    pub(crate) fn handle_command_palette_navigate_msg(
        &mut self,
        forward: bool,
    ) -> Task<Message> {
        let entries = self.command_palette_entries();
        self.command_palette_state
            .navigate(if forward { 1 } else { -1 }, entries.len());
        self.scroll_command_palette_to_selection(&entries)
    }

    /// Highlights the clicked row and runs it.
    pub(crate) fn handle_command_palette_selected_msg(
        &mut self,
        index: usize,
    ) -> Task<Message> {
        self.command_palette_state.selected = index;
        self.handle_submit_command_palette_msg()
    }

    /// Runs the highlighted command and closes the palette.
    ///
    /// The action is emitted as a task rather than applied in place, so it
    /// travels back through the host application exactly as it would if the
    /// user had pressed the shortcut. That matters for the actions the editor
    /// itself does not perform — saving, revealing the file, and every
    /// host-registered command — which the host intercepts on its way in.
    pub(crate) fn handle_submit_command_palette_msg(
        &mut self,
    ) -> Task<Message> {
        let entries = self.command_palette_entries();
        let Some(entry) = entries.get(self.command_palette_state.selected)
        else {
            return Task::none();
        };
        let message = match &entry.action {
            PaletteAction::Builtin(message) => (**message).clone(),
            PaletteAction::Custom(id) => {
                Message::CommandPaletteAction(id.clone())
            }
        };
        let key = entry.key();

        self.command_palette_state.record_recent(key);
        self.command_palette_state.close();
        Task::done(message)
    }

    /// Scrolls the result list to the window computed by
    /// [`super::CommandPaletteState::navigate`].
    ///
    /// The offset is taken from `first_visible_row` rather than from
    /// `selected`: scrolling to the selected row would pin it to the top of
    /// the list, so the very first `Down` would push row 0 out of sight.
    ///
    /// `entries` is passed in rather than recomputed: the callers already hold
    /// the list, and it is what says where the recently-used separator sits,
    /// which shifts every row below it.
    ///
    /// # Arguments
    ///
    /// * `entries` - The rows currently displayed, in display order
    ///
    /// # Returns
    ///
    /// A `Task<Message>` scrolling the result list to that window
    fn scroll_command_palette_to_selection(
        &self,
        entries: &[PaletteEntry],
    ) -> Task<Message> {
        scroll_to(
            self.command_palette_state.scrollable_id.clone(),
            AbsoluteOffset {
                x: 0.0,
                y: row_top(
                    self.command_palette_state.first_visible_row,
                    separator_after(entries),
                ),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContextMenuItem;

    /// Returns the label of the row the palette would run right now.
    fn selected_label(editor: &CodeEditor) -> String {
        let entries = editor.command_palette_entries();
        entries
            .get(editor.command_palette_state.selected)
            .map(|entry| entry.label.clone())
            .unwrap_or_default()
    }

    #[test]
    fn test_open_closes_the_other_dialogs_and_clears_the_query() {
        let mut editor = CodeEditor::new("one\ntwo", "rs");
        let _ = editor.update(&Message::OpenSearch);
        let _ = editor.update(&Message::OpenCommandPalette);
        let _ =
            editor.update(&Message::CommandPaletteChanged("fold".to_string()));
        let _ = editor.update(&Message::CloseCommandPalette);

        let _ = editor.update(&Message::OpenCommandPalette);

        assert!(editor.command_palette_state.is_open);
        assert!(editor.command_palette_state.query.is_empty());
        assert!(!editor.search_state.is_open);
        assert!(!editor.goto_line_state.is_open);
    }

    #[test]
    fn test_running_a_command_lists_it_first_the_next_time() {
        let mut editor = CodeEditor::new("one\ntwo", "rs");
        let _ = editor.update(&Message::OpenCommandPalette);
        let first_label = selected_label(&editor);

        // Run the second row, then reopen on an empty query.
        let _ = editor.update(&Message::CommandPaletteNavigate(true));
        let promoted_label = selected_label(&editor);
        let _ = editor.update(&Message::SubmitCommandPalette);
        let _ = editor.update(&Message::OpenCommandPalette);

        let entries = editor.command_palette_entries();
        assert_eq!(entries[0].label, promoted_label);
        assert!(entries[0].is_recent);
        assert_eq!(entries[1].label, first_label);
        assert!(!entries[1].is_recent);
    }

    #[test]
    fn test_only_the_last_three_commands_are_promoted() {
        let mut editor = CodeEditor::new("one\ntwo", "rs");
        let mut expected = Vec::new();

        // Run four distinct commands, taking the row one further down each
        // time so the history fills with four different entries.
        for steps in 0..4 {
            let _ = editor.update(&Message::OpenCommandPalette);
            for _ in 0..steps {
                let _ = editor.update(&Message::CommandPaletteNavigate(true));
            }
            expected.insert(0, selected_label(&editor));
            let _ = editor.update(&Message::SubmitCommandPalette);
        }

        let _ = editor.update(&Message::OpenCommandPalette);
        let entries = editor.command_palette_entries();
        let recent: Vec<String> = entries
            .iter()
            .filter(|entry| entry.is_recent)
            .map(|entry| entry.label.clone())
            .collect();

        assert_eq!(recent, expected[..3]);
    }

    #[test]
    fn test_open_is_ignored_when_the_palette_is_disabled() {
        let mut editor = CodeEditor::new("one\ntwo", "rs")
            .with_command_palette_enabled(false);

        let _ = editor.update(&Message::OpenCommandPalette);

        assert!(!editor.command_palette_state.is_open);
    }

    #[test]
    fn test_filtering_narrows_the_list_and_resets_the_highlight() {
        let mut editor = CodeEditor::new("one\ntwo", "rs");
        let _ = editor.update(&Message::OpenCommandPalette);
        let _ = editor.update(&Message::CommandPaletteNavigate(true));
        assert_ne!(editor.command_palette_state.selected, 0);

        let _ = editor
            .update(&Message::CommandPaletteChanged("fold all".to_string()));

        assert_eq!(editor.command_palette_state.selected, 0);
        assert_eq!(selected_label(&editor), "Fold All");
    }

    #[test]
    fn test_navigation_wraps_over_the_filtered_list() {
        let mut editor = CodeEditor::new("one\ntwo", "rs");
        let _ = editor.update(&Message::OpenCommandPalette);
        let _ =
            editor.update(&Message::CommandPaletteChanged("fold".to_string()));
        let count = editor.command_palette_entries().len();
        assert!(count > 1);

        let _ = editor.update(&Message::CommandPaletteNavigate(false));

        assert_eq!(editor.command_palette_state.selected, count - 1);
    }

    #[test]
    fn test_submit_emits_the_built_in_action_and_closes_the_palette() {
        let mut editor = CodeEditor::new("fn main() {}", "rs");
        let _ = editor.update(&Message::OpenCommandPalette);
        let _ = editor
            .update(&Message::CommandPaletteChanged("goto line".to_string()));
        assert_eq!(selected_label(&editor), "Go to Line");

        let _ = editor.update(&Message::SubmitCommandPalette);

        assert!(!editor.command_palette_state.is_open);
        // The action itself is only emitted; running it is the host's round
        // trip, which the editor performs on the message it gets back.
        assert!(!editor.goto_line_state.is_open);
        let _ = editor.update(&Message::OpenGotoLine);
        assert!(editor.goto_line_state.is_open);
    }

    #[test]
    fn test_submit_forwards_host_commands_by_id() {
        let mut editor = CodeEditor::new("fn main() {}", "rs")
            .with_custom_command_palette_entries(vec![ContextMenuItem::new(
                "app.open_file",
                "Open File",
            )]);
        let _ = editor.update(&Message::OpenCommandPalette);
        let _ = editor
            .update(&Message::CommandPaletteChanged("open file".to_string()));

        assert_eq!(selected_label(&editor), "Open File");
        let _ = editor.update(&Message::SubmitCommandPalette);
        assert!(!editor.command_palette_state.is_open);
    }

    #[test]
    fn test_submit_without_a_match_leaves_the_palette_open() {
        let mut editor = CodeEditor::new("fn main() {}", "rs");
        let _ = editor.update(&Message::OpenCommandPalette);
        let _ = editor.update(&Message::CommandPaletteChanged(
            "zzz no such command".to_string(),
        ));
        assert!(editor.command_palette_entries().is_empty());

        let _ = editor.update(&Message::SubmitCommandPalette);

        assert!(editor.command_palette_state.is_open);
    }

    #[test]
    fn test_clicking_a_row_runs_that_row() {
        let mut editor = CodeEditor::new("fn main() {}", "rs");
        let _ = editor.update(&Message::OpenCommandPalette);
        let _ =
            editor.update(&Message::CommandPaletteChanged("fold".to_string()));
        let target = editor.command_palette_entries().len() - 1;

        let _ = editor.update(&Message::CommandPaletteSelected(target));

        assert_eq!(editor.command_palette_state.selected, target);
        assert!(!editor.command_palette_state.is_open);
    }

    #[test]
    fn test_command_palette_action_is_left_to_the_host() {
        let mut editor = CodeEditor::new("fn main() {}", "rs");
        let before = editor.content();

        let _ = editor.update(&Message::CommandPaletteAction(
            "app.open_file".to_string(),
        ));

        assert_eq!(editor.content(), before);
    }
}
