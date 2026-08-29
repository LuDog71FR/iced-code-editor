//! Rendering: the `canvas::Program` implementation and its rendering layers
//! (gutter, highlighting, text, overlays), plus the Iced view construction and
//! the logical-to-visual line wrapping calculator they all depend on.

mod canvas;
pub(crate) mod gutter;
pub(crate) mod highlighting;
pub(crate) mod overlays;
pub(crate) mod text;
pub(crate) mod view;
pub(crate) mod wrapping;
