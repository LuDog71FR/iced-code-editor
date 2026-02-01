//! Demo application for iced-code-editor with pane_grid layout.
//!
//! This demo reproduces a typical IDE layout with:
//! - A toolbar at the top
//! - A vertical pane_grid with:
//!   - Top pane: DropDown menu + CodeEditor (height constrained to 400px)
//!   - Bottom pane: Output/Log area
//!
//! Remarks:
//! This layout is designed to test overflow and z-index issues.

mod app;
mod file_ops;
mod types;
mod ui;

/// Main entry point for the demo application.
fn run_app() -> iced::Result {
    let settings = iced::Settings {
        default_font: iced::Font::with_name("Noto Sans CJK SC"),
        fonts: vec![
            include_bytes!("../../fonts/JetBrainsMono-Regular.ttf")
                .as_slice()
                .into(),
            include_bytes!("../../fonts/NotoSansCJKsc-Regular.otf")
                .as_slice()
                .into(),
        ],
        ..Default::default()
    };

    iced::application(app::DemoApp::new, app::DemoApp::update, ui::view)
        .subscription(app::DemoApp::subscription)
        .theme(app::DemoApp::theme)
        .settings(settings)
        .run()
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> iced::Result {
    run_app()
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    run_app().map_err(|e| JsValue::from_str(&e.to_string()))
}

#[cfg(target_arch = "wasm32")]
fn main() {}
