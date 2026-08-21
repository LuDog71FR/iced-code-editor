//! Optional editor features: bracket matching, inline color previews, the
//! right-click context menu, code folding, go-to-line, indentation guides,
//! search/replace, and Vim emulation.

pub(crate) mod bracket_match;
pub(crate) mod color_preview;
pub(crate) mod context_menu;
pub mod folding;
pub(crate) mod goto_line;
pub(crate) mod indent_guides;
pub(crate) mod search;
pub(crate) mod vim;
