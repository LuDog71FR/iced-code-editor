//! Application message types for [`DemoApp`]'s update loop.

use crate::types::{
    EditorId, EditorToggle, FontOption, LanguageOption, Template,
};
use iced::{Event, Theme};
use iced_code_editor::IndentStyle;
use iced_code_editor::Message as EditorMessage;
use std::path::PathBuf;

/// Application messages.
#[derive(Debug, Clone)]
pub enum Message {
    /// Toggle settings modal
    ToggleSettings,
    /// Toggle auto adjust line height
    ToggleAutoLineHeight(bool),
    /// Toggle the editor options dropdown panel
    ToggleEditorOptions,
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
    ///
    /// With format-on-save enabled and a language server attached, this first
    /// asks the server to format the document; the write itself is then
    /// carried out by [`Message::WriteFile`].
    SaveFile,
    /// Write the editor's current contents to its file, formatting no further.
    ///
    /// The second half of [`Message::SaveFile`], and the message the formatted
    /// document is saved with once the server's edits have been applied. Only
    /// the formatting detour produces it, so it does not exist where there is
    /// no language server to detour through.
    #[cfg(not(target_arch = "wasm32"))]
    WriteFile(EditorId),
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
    /// Interaction with the read-only output log: selection, scrolling and
    /// cursor moves. Editing actions are ignored by `update`.
    LogAction(iced::widget::text_editor::Action),
    /// Copy the whole output log to the clipboard
    CopyLog,
    /// Run code (simulated)
    RunCode,
    /// Toggle a boolean editor setting (wrap, folding, auto-indent, ...) —
    /// see [`EditorToggle`]
    ToggleEditor(EditorId, EditorToggle, bool),
    /// Change indentation style
    IndentStyleChanged(EditorId, IndentStyle),
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
    /// Ask the language server to format the whole document
    #[cfg(not(target_arch = "wasm32"))]
    FormatDocument(EditorId),
    #[cfg(not(target_arch = "wasm32"))]
    LspOverlay(iced_code_editor::LspOverlayMessage),
    #[cfg(not(target_arch = "wasm32"))]
    JumpToFile(PathBuf, usize, usize),
    #[cfg(not(target_arch = "wasm32"))]
    FileOpenedAndJump(Result<(PathBuf, String, usize, usize), String>),
}
