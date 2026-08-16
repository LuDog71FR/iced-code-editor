//! Rendering: the `canvas::Program` implementation and its rendering layers
//! (gutter, text, overlays), plus the Iced view construction and the
//! logical-to-visual line wrapping calculator they all depend on.

mod canvas;
pub(crate) mod gutter;
pub(crate) mod overlays;
pub(crate) mod text;
pub(crate) mod view;
pub(crate) mod wrapping;
