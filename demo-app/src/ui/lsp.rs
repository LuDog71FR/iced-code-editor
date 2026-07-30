//! LSP (Language Server Protocol) UI components.
//!
//! This module provides the UI integration point for LSP overlay features such as
//! hover tooltips and auto-completion menus.

use crate::app::{DemoApp, Message};
use crate::types::EditorId;
#[cfg(target_arch = "wasm32")]
use iced::widget::{Space, column, container};
#[cfg(not(target_arch = "wasm32"))]
use iced::widget::{Space, container};
#[cfg(not(target_arch = "wasm32"))]
use iced::{Element, Length};
#[cfg(target_arch = "wasm32")]
use iced::{Element, Length};

/// Returns an empty LSP panel placeholder.
/// Currently not implemented — returns a minimal zero-size container.
pub fn view_lsp_panel() -> Element<'static, Message> {
    #[cfg(not(target_arch = "wasm32"))]
    return container(Space::new())
        .width(Length::Shrink)
        .height(Length::Shrink)
        .into();

    #[cfg(target_arch = "wasm32")]
    column![].into()
}

/// Creates overlay UI elements for LSP features (hover tooltips and completion menus).
///
/// Returns an empty container when:
/// - Running on WebAssembly (LSP not available)
/// - This editor does not have LSP overlay focus
///
/// Otherwise delegates to [`iced_code_editor::view_lsp_overlay`].
pub fn view_lsp_overlay(
    _app: &DemoApp,
    _editor_id: EditorId,
) -> Element<'_, Message> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if _app.lsp_overlay_editor != Some(_editor_id) {
            return container(
                Space::new().width(Length::Shrink).height(Length::Shrink),
            )
            .into();
        }

        let Some(tab) = _app.tabs.iter().find(|t| t.id == _editor_id) else {
            return container(
                Space::new().width(Length::Shrink).height(Length::Shrink),
            )
            .into();
        };

        iced_code_editor::view_lsp_overlay(
            &_app.lsp_overlay,
            &tab.editor,
            &_app.current_theme,
            _app.current_font_size,
            _app.current_line_height,
            Message::LspOverlay,
        )
    }

    #[cfg(target_arch = "wasm32")]
    container(Space::new().width(Length::Shrink).height(Length::Shrink)).into()
}
