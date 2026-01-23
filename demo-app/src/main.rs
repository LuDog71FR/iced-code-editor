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
fn main() -> iced::Result {
    iced::application(app::DemoApp::new, app::DemoApp::update, ui::view)
        .subscription(app::DemoApp::subscription)
        .theme(app::DemoApp::theme)
        .font(
            include_bytes!("../../fonts/JetBrainsMono-Regular.ttf").as_slice(),
        )
        .run()
}
