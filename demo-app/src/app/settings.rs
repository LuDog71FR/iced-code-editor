//! Global editor settings for [`DemoApp`]: font, size, line height,
//! language, theme, indentation style, and the boolean per-editor toggles
//! (see [`EditorToggle`]).
//!
//! Font/size/line-height/language/theme changes apply to every open tab;
//! indent style and the boolean toggles apply to a single tab, identified
//! by [`EditorId`].

use super::{DemoApp, Message};
use crate::types::{EditorId, EditorToggle, FontOption, LanguageOption};
use iced::{Task, Theme};
use iced_code_editor::{IndentStyle, theme};

impl DemoApp {
    /// Handles font changes by updating all editors.
    pub(super) fn handle_font_changed(
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
    pub(super) fn handle_font_size_changed(
        &mut self,
        size: f32,
    ) -> Task<Message> {
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
    pub(super) fn handle_line_height_changed(
        &mut self,
        height: f32,
    ) -> Task<Message> {
        self.current_line_height = height;
        for tab in &mut self.tabs {
            tab.editor.set_line_height(height);
        }
        Task::none()
    }

    /// Handles UI language changes by updating all editors.
    pub(super) fn handle_language_changed(
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
    pub(super) fn handle_theme_changed(
        &mut self,
        new_theme: Theme,
    ) -> Task<Message> {
        self.log("INFO", &format!("Theme changed to: {:?}", new_theme));
        let style = theme::from_iced_theme(&new_theme);
        self.current_theme = new_theme;
        for tab in &mut self.tabs {
            tab.editor.set_theme(style);
        }
        Task::none()
    }

    /// Handles changing the indentation style for a specific editor.
    pub(super) fn handle_indent_style_changed(
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

    /// Handles toggling a boolean editor setting (see [`EditorToggle`]).
    ///
    /// Every checkbox in the options panel routes through here: the setting
    /// itself is applied via [`EditorToggle::apply`] and the change is
    /// logged uniformly for all eleven toggles — previously each toggle had
    /// its own near-identical handler, and six of them never logged the
    /// change, an inconsistency visible in the log pane that this shared
    /// path removes by construction. [`EditorToggle::Lsp`] gets one extra
    /// step — attaching or detaching the actual language-server process —
    /// since that requires spawning a subprocess and is not available on
    /// WASM.
    pub(super) fn handle_toggle_editor(
        &mut self,
        editor_id: EditorId,
        toggle: EditorToggle,
        enabled: bool,
    ) -> Task<Message> {
        self.log(
            "INFO",
            &format!(
                "{} {} in {:?} editor",
                toggle.label(),
                if enabled { "enabled" } else { "disabled" },
                editor_id
            ),
        );

        if let Some(tab) = self.get_tab(editor_id) {
            toggle.apply(&mut tab.editor, enabled);
        }

        #[cfg(not(target_arch = "wasm32"))]
        if toggle == EditorToggle::Lsp {
            if enabled {
                self.sync_lsp_for_editor(editor_id);
            } else {
                self.set_lsp_server_for_editor(editor_id, None);
            }
        }

        Task::none()
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

        let _ = app.handle_toggle_editor(tab_id, EditorToggle::Vim, true);
        assert!(app.get_active_editor().is_some_and(|e| e.vim_enabled()));

        let _ = app.handle_toggle_editor(tab_id, EditorToggle::Vim, false);
        assert!(!app.get_active_editor().is_some_and(|e| e.vim_enabled()));
    }

    #[test]
    fn test_toggle_auto_close_brackets_updates_editor_setting() {
        let (mut app, _) = DemoApp::new();
        let tab_id = app.active_tab_id;

        assert!(
            app.get_active_editor().is_some_and(|e| e.auto_close_brackets())
        );

        let _ = app.handle_toggle_editor(
            tab_id,
            EditorToggle::AutoCloseBrackets,
            false,
        );
        assert!(
            !app.get_active_editor().is_some_and(|e| e.auto_close_brackets())
        );

        let _ = app.handle_toggle_editor(
            tab_id,
            EditorToggle::AutoCloseBrackets,
            true,
        );
        assert!(
            app.get_active_editor().is_some_and(|e| e.auto_close_brackets())
        );
    }
}
