//! Optional editor features: bracket matching, the right-click context
//! menu, code folding, go-to-line, search/replace, and Vim emulation.

pub(crate) mod bracket_match;
pub(crate) mod context_menu;
pub mod folding;
pub(crate) mod goto_line;
pub(crate) mod search;
pub(crate) mod vim;
