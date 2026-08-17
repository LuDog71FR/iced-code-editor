//! Vim emulation for the canvas editor.
//!
//! This module parses key sequences into motions, operators, and actions,
//! and tracks the modal state (normal/insert/visual/visual-line) needed to
//! reproduce Vim-style editing behavior.

mod update;

/// The active editing mode when Vim behavior is enabled.
///
/// Read it through [`CodeEditor::vim_mode`], which returns `None` while Vim is
/// disabled — so a status bar can distinguish "Vim is off" from "Vim is in
/// Normal mode".
///
/// # Examples
///
/// ```
/// use iced_code_editor::{CodeEditor, VimMode};
///
/// let editor = CodeEditor::new("fn main() {}", "rs");
/// // Vim is off by default.
/// assert_eq!(editor.vim_mode(), None);
///
/// // Enabling it always starts in Normal mode.
/// let editor = editor.with_vim_enabled(true);
/// assert_eq!(editor.vim_mode(), Some(VimMode::Normal));
/// assert_eq!(VimMode::default(), VimMode::Normal);
/// ```
///
/// [`CodeEditor::vim_mode`]: crate::CodeEditor::vim_mode
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum VimMode {
    /// Command-oriented navigation and editing mode.
    #[default]
    Normal,
    /// Text insertion mode.
    Insert,
    /// Character-wise visual selection mode.
    Visual,
    /// Line-wise visual selection mode.
    VisualLine,
}

/// A cursor motion recognized by the Vim parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VimMotion {
    /// Move left one character (`h`).
    Left,
    /// Move down one visible line (`j`).
    Down,
    /// Move up one visible line (`k`).
    Up,
    /// Move right one character (`l`).
    Right,
    /// Move to the start of the next word (`w`).
    WordForward,
    /// Move to the start of the previous word (`b`).
    WordBackward,
    /// Move to the end of the current/next word (`e`).
    WordEnd,
    /// Move to column 0 of the current line (`0`).
    LineStart,
    /// Move to the first non-blank character of the line (`^`).
    FirstNonBlank,
    /// Move to the last character of the line (`$`).
    LineEnd,
    /// Move to line `count` (1-based), or line 1 if no count is given (`gg`).
    DocumentStart,
    /// Move to line `count` (1-based) if a count is explicitly given,
    /// otherwise the last line (`G`).
    DocumentEnd,
}

/// An operator waiting for, or combined with, a motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VimOperator {
    /// Delete the targeted range (`d`).
    Delete,
    /// Delete the targeted range and enter Insert mode (`c`).
    Change,
    /// Yank (copy) the targeted range into the unnamed register (`y`).
    Yank,
}

/// The insertion position requested by a Normal-mode command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VimInsertPosition {
    /// Enter Insert mode before the cursor (`i`).
    BeforeCursor,
    /// Enter Insert mode after the cursor (`a`).
    AfterCursor,
    /// Enter Insert mode at the first non-blank character of the line (`I`).
    FirstNonBlank,
    /// Enter Insert mode at the end of the line (`A`).
    EndOfLine,
    /// Open a new line below the current one and enter Insert mode (`o`).
    NewLineBelow,
    /// Open a new line above the current one and enter Insert mode (`O`).
    NewLineAbove,
}

/// The side of the cursor on which a paste should occur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VimPastePosition {
    /// Paste after the cursor (`p`).
    AfterCursor,
    /// Paste before the cursor (`P`).
    BeforeCursor,
}

/// A complete, buffer-independent intent emitted by [`VimState`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VimAction {
    /// Switch to a different Vim mode.
    Mode(VimMode),
    /// Move the cursor by a motion, repeated `count` times. `explicit_count`
    /// distinguishes an explicitly typed count (e.g. `1G`) from the default
    /// of 1 (e.g. bare `G`), since some motions (`G`) behave differently in
    /// each case.
    Motion { motion: VimMotion, count: usize, explicit_count: bool },
    /// Enter Insert mode at the given position, repeating the eventual
    /// inserted text `count` times on exit.
    Insert { position: VimInsertPosition, count: usize },
    /// Apply an operator to the range covered by a motion, repeated `count`
    /// times. See [`VimAction::Motion`] for the meaning of `explicit_count`.
    Operator {
        operator: VimOperator,
        motion: VimMotion,
        count: usize,
        explicit_count: bool,
    },
    /// Apply an operator to `count` whole lines (e.g. `dd`, `yy`, `cc`).
    LineOperator { operator: VimOperator, count: usize },
    /// Apply an operator to the active Visual/Visual Line selection.
    VisualOperator(VimOperator),
    /// Delete `count` characters under and after the cursor (`x`).
    DeleteCharacters { count: usize },
    /// Paste the unnamed register `count` times at the given position.
    Paste { position: VimPastePosition, count: usize },
    /// Undo the last `count` grouped commands (`u`).
    Undo { count: usize },
    /// Redo the last `count` undone commands (`Ctrl+R`).
    Redo { count: usize },
    /// Repeat the last `/` search, optionally reversing its direction
    /// (`n`/`N`).
    RepeatSearch { reverse: bool },
    /// The pending `/` or `:` command-line input changed.
    CommandLineChanged,
    /// A `/` search was submitted with the given pattern.
    SubmitSearch(String),
    /// A `:N` command was submitted, requesting a jump to the given
    /// 1-based line.
    SubmitGotoLine(usize),
    /// A `:w` or `:wq` command was submitted, requesting a host save and
    /// optionally exiting Vim mode.
    WriteFile { exit_vim: bool },
    /// A `:q` command was submitted, requesting Vim mode be turned off.
    ExitVimMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VimCommandLineKind {
    Search,
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VimCommandLine {
    kind: VimCommandLineKind,
    input: String,
}

/// Whether register text represents a character range or complete lines.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VimRegisterKind {
    #[default]
    Characterwise,
    Linewise,
}

/// The per-editor unnamed Vim register.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct VimRegister {
    pub(crate) text: String,
    pub(crate) kind: VimRegisterKind,
}

/// Pure Vim parsing state owned by one editor instance.
#[derive(Debug, Default)]
pub(crate) struct VimState {
    mode: VimMode,
    count: Option<usize>,
    g_prefix: bool,
    pending_operator: Option<VimOperator>,
    pending_operator_count: usize,
    visual_anchor: Option<(usize, usize)>,
    visual_active: Option<(usize, usize)>,
    command_line: Option<VimCommandLine>,
    last_search: Option<String>,
    pub(crate) register: VimRegister,
}

impl VimState {
    pub(crate) fn mode(&self) -> VimMode {
        self.mode
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn enter_clean_normal_mode(&mut self) {
        self.mode = VimMode::Normal;
        self.clear_visual();
        self.command_line = None;
        self.clear_pending();
    }

    pub(crate) fn command_line_active(&self) -> bool {
        self.command_line.is_some()
    }

    pub(crate) fn last_search(&self) -> Option<&str> {
        self.last_search.as_deref()
    }

    pub(crate) fn command_line_text(&self) -> Option<String> {
        self.command_line.as_ref().map(|command_line| {
            let prefix = match command_line.kind {
                VimCommandLineKind::Search => '/',
                VimCommandLineKind::Command => ':',
            };
            format!("{prefix}{}", command_line.input)
        })
    }

    pub(crate) fn pending_keys(&self) -> String {
        let mut pending = String::new();
        if let Some(operator) = self.pending_operator {
            if self.pending_operator_count > 1 {
                pending.push_str(&self.pending_operator_count.to_string());
            }
            pending.push(match operator {
                VimOperator::Delete => 'd',
                VimOperator::Change => 'c',
                VimOperator::Yank => 'y',
            });
            if let Some(count) = self.count {
                pending.push_str(&count.to_string());
            }
        } else if let Some(count) = self.count {
            pending.push_str(&count.to_string());
        }
        if self.g_prefix {
            pending.push('g');
        }
        pending
    }

    pub(crate) fn status_line_text(&self) -> (String, String) {
        if let Some(command_line) = self.command_line_text() {
            return (command_line, String::new());
        }

        let mode = match self.mode {
            VimMode::Normal => "NORMAL",
            VimMode::Insert => "INSERT",
            VimMode::Visual => "VISUAL",
            VimMode::VisualLine => "VISUAL LINE",
        };

        (format!("-- {mode} --"), self.pending_keys())
    }

    pub(crate) fn begin_visual(&mut self, position: (usize, usize)) {
        self.visual_anchor = Some(position);
        self.visual_active = Some(position);
    }

    pub(crate) fn visual_positions(
        &self,
    ) -> Option<((usize, usize), (usize, usize))> {
        Some((self.visual_anchor?, self.visual_active?))
    }

    pub(crate) fn set_visual_active(&mut self, position: (usize, usize)) {
        self.visual_active = Some(position);
    }

    pub(crate) fn clear_visual(&mut self) {
        self.visual_anchor = None;
        self.visual_active = None;
    }

    pub(crate) fn set_mode_from_mouse(&mut self, mode: VimMode) {
        self.mode = mode;
        self.clear_pending();
    }

    pub(crate) fn enter_insert_mode(&mut self) {
        self.mode = VimMode::Insert;
        self.clear_visual();
        self.command_line = None;
        self.clear_pending();
    }

    pub(crate) fn parse_key(&mut self, key: char) -> Option<VimAction> {
        if self.command_line.is_some() {
            return self.parse_command_line_key(key);
        }

        if self.mode == VimMode::Insert {
            return if key == '\u{1b}' {
                Some(self.set_mode(VimMode::Normal))
            } else {
                None
            };
        }

        if key == '\u{1b}' {
            return Some(self.set_mode(VimMode::Normal));
        }

        if key.is_ascii_digit() && (key != '0' || self.count.is_some()) {
            self.push_count_digit(key);
            return None;
        }

        if self.g_prefix {
            if key == 'g' {
                self.g_prefix = false;
                return Some(self.finish_motion(VimMotion::DocumentStart));
            }
            self.clear_pending();
            return None;
        }

        if key == 'g' {
            self.g_prefix = true;
            return None;
        }

        if let Some(operator) = operator_for_key(key) {
            return self.parse_operator(operator);
        }

        if let Some(motion) = motion_for_key(key) {
            return Some(self.finish_motion(motion));
        }

        let action = match key {
            'i' => Some(self.insert(VimInsertPosition::BeforeCursor)),
            'a' => Some(self.insert(VimInsertPosition::AfterCursor)),
            'I' => Some(self.insert(VimInsertPosition::FirstNonBlank)),
            'A' => Some(self.insert(VimInsertPosition::EndOfLine)),
            'o' => Some(self.insert(VimInsertPosition::NewLineBelow)),
            'O' => Some(self.insert(VimInsertPosition::NewLineAbove)),
            'v' => Some(self.set_mode(VimMode::Visual)),
            'V' => Some(self.set_mode(VimMode::VisualLine)),
            'x' => {
                Some(VimAction::DeleteCharacters { count: self.take_count() })
            }
            'p' => Some(VimAction::Paste {
                position: VimPastePosition::AfterCursor,
                count: self.take_count(),
            }),
            'P' => Some(VimAction::Paste {
                position: VimPastePosition::BeforeCursor,
                count: self.take_count(),
            }),
            'u' => Some(VimAction::Undo { count: self.take_count() }),
            '\u{12}' => Some(VimAction::Redo { count: self.take_count() }),
            'n' => Some(VimAction::RepeatSearch { reverse: false }),
            'N' => Some(VimAction::RepeatSearch { reverse: true }),
            '/' => Some(self.open_command_line(VimCommandLineKind::Search)),
            ':' => Some(self.open_command_line(VimCommandLineKind::Command)),
            _ => None,
        };

        if action.is_none() {
            self.clear_pending();
        }
        action
    }

    fn open_command_line(&mut self, kind: VimCommandLineKind) -> VimAction {
        self.clear_pending();
        self.command_line = Some(VimCommandLine { kind, input: String::new() });
        VimAction::CommandLineChanged
    }

    fn parse_command_line_key(&mut self, key: char) -> Option<VimAction> {
        match key {
            '\u{1b}' => {
                self.command_line = None;
                Some(VimAction::CommandLineChanged)
            }
            '\u{8}' => {
                if let Some(command_line) = self.command_line.as_mut()
                    && command_line.input.pop().is_none()
                {
                    self.command_line = None;
                }
                Some(VimAction::CommandLineChanged)
            }
            '\n' | '\r' => {
                let command_line = self.command_line.take()?;
                if command_line.input.is_empty() {
                    return Some(VimAction::CommandLineChanged);
                }
                match command_line.kind {
                    VimCommandLineKind::Search => {
                        self.last_search = Some(command_line.input.clone());
                        Some(VimAction::SubmitSearch(command_line.input))
                    }
                    VimCommandLineKind::Command => {
                        match command_line.input.as_str() {
                            "q" => Some(VimAction::ExitVimMode),
                            "w" => {
                                Some(VimAction::WriteFile { exit_vim: false })
                            }
                            "wq" => {
                                Some(VimAction::WriteFile { exit_vim: true })
                            }
                            _ => command_line
                                .input
                                .parse::<usize>()
                                .ok()
                                .filter(|line| *line > 0)
                                .map(VimAction::SubmitGotoLine)
                                .or(Some(VimAction::CommandLineChanged)),
                        }
                    }
                }
            }
            key if !key.is_control() => {
                if let Some(command_line) = self.command_line.as_mut() {
                    command_line.input.push(key);
                }
                Some(VimAction::CommandLineChanged)
            }
            _ => None,
        }
    }

    fn set_mode(&mut self, mode: VimMode) -> VimAction {
        self.mode = mode;
        self.clear_pending();
        VimAction::Mode(mode)
    }

    fn insert(&mut self, position: VimInsertPosition) -> VimAction {
        let count = self.take_count();
        self.mode = VimMode::Insert;
        self.clear_pending();
        VimAction::Insert { position, count }
    }

    fn parse_operator(&mut self, operator: VimOperator) -> Option<VimAction> {
        if self.mode != VimMode::Normal {
            self.clear_pending();
            return Some(VimAction::VisualOperator(operator));
        }

        if let Some(pending) = self.pending_operator {
            if pending == operator {
                let count = self
                    .pending_operator_count
                    .saturating_mul(self.take_count());
                self.clear_pending();
                return Some(VimAction::LineOperator { operator, count });
            }
            self.clear_pending();
            return None;
        }

        self.pending_operator_count = self.take_count();
        self.pending_operator = Some(operator);
        None
    }

    fn finish_motion(&mut self, motion: VimMotion) -> VimAction {
        let motion_count_explicit = self.count.is_some();
        let motion_count = self.take_count();
        if let Some(operator) = self.pending_operator {
            let count =
                self.pending_operator_count.saturating_mul(motion_count);
            // The operator's own count slot doesn't track whether it was
            // explicitly typed, so `1dG` vs `dG` stays ambiguous; this
            // approximates "explicit" for the combined count.
            let explicit_count =
                motion_count_explicit || self.pending_operator_count > 1;
            self.clear_pending();
            VimAction::Operator { operator, motion, count, explicit_count }
        } else {
            self.g_prefix = false;
            VimAction::Motion {
                motion,
                count: motion_count,
                explicit_count: motion_count_explicit,
            }
        }
    }

    fn push_count_digit(&mut self, key: char) {
        let digit = key.to_digit(10).unwrap_or_default() as usize;
        self.count = Some(
            self.count
                .unwrap_or_default()
                .saturating_mul(10)
                .saturating_add(digit),
        );
    }

    fn take_count(&mut self) -> usize {
        self.count.take().unwrap_or(1).max(1)
    }

    fn clear_pending(&mut self) {
        self.count = None;
        self.g_prefix = false;
        self.pending_operator = None;
        self.pending_operator_count = 1;
    }
}

fn motion_for_key(key: char) -> Option<VimMotion> {
    match key {
        'h' => Some(VimMotion::Left),
        'j' => Some(VimMotion::Down),
        'k' => Some(VimMotion::Up),
        'l' => Some(VimMotion::Right),
        'w' => Some(VimMotion::WordForward),
        'b' => Some(VimMotion::WordBackward),
        'e' => Some(VimMotion::WordEnd),
        '0' => Some(VimMotion::LineStart),
        '^' => Some(VimMotion::FirstNonBlank),
        '$' => Some(VimMotion::LineEnd),
        'G' => Some(VimMotion::DocumentEnd),
        _ => None,
    }
}

fn operator_for_key(key: char) -> Option<VimOperator> {
    match key {
        'd' => Some(VimOperator::Delete),
        'c' => Some(VimOperator::Change),
        'y' => Some(VimOperator::Yank),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        VimAction, VimMotion, VimOperator, VimRegister, VimRegisterKind,
        VimState,
    };

    #[test]
    fn vim_parser_accumulates_count_and_operator() {
        let mut state = VimState::default();

        assert_eq!(state.parse_key('2'), None);
        assert_eq!(state.parse_key('d'), None);
        assert_eq!(state.parse_key('3'), None);
        assert_eq!(
            state.parse_key('w'),
            Some(VimAction::Operator {
                operator: VimOperator::Delete,
                motion: VimMotion::WordForward,
                count: 6,
                explicit_count: true,
            })
        );
    }

    #[test]
    fn vim_parser_zero_is_motion_without_leading_count() {
        let mut state = VimState::default();

        assert_eq!(
            state.parse_key('0'),
            Some(VimAction::Motion {
                motion: VimMotion::LineStart,
                count: 1,
                explicit_count: false,
            })
        );

        assert_eq!(state.parse_key('1'), None);
        assert_eq!(state.parse_key('0'), None);
        assert_eq!(
            state.parse_key('l'),
            Some(VimAction::Motion {
                motion: VimMotion::Right,
                count: 10,
                explicit_count: true,
            })
        );
    }

    #[test]
    fn vim_parser_distinguishes_explicit_count_for_document_end() {
        let mut state = VimState::default();

        assert_eq!(
            state.parse_key('G'),
            Some(VimAction::Motion {
                motion: VimMotion::DocumentEnd,
                count: 1,
                explicit_count: false,
            })
        );

        assert_eq!(state.parse_key('1'), None);
        assert_eq!(
            state.parse_key('G'),
            Some(VimAction::Motion {
                motion: VimMotion::DocumentEnd,
                count: 1,
                explicit_count: true,
            })
        );
    }

    #[test]
    fn vim_state_resets_typed_unnamed_register() {
        let mut state = VimState {
            register: VimRegister {
                text: "line\n".to_owned(),
                kind: VimRegisterKind::Linewise,
            },
            ..VimState::default()
        };

        assert_eq!(state.register.text, "line\n");
        assert_eq!(state.register.kind, VimRegisterKind::Linewise);

        state.reset();

        assert!(state.register.text.is_empty());
        assert_eq!(state.register.kind, VimRegisterKind::Characterwise);
    }

    #[test]
    fn vim_command_line_parser_accepts_edit_submit_and_cancel() {
        let mut state = VimState::default();

        assert_eq!(state.parse_key('/'), Some(VimAction::CommandLineChanged));
        assert_eq!(state.command_line_text().as_deref(), Some("/"));

        for key in "foo".chars() {
            assert_eq!(
                state.parse_key(key),
                Some(VimAction::CommandLineChanged)
            );
        }
        assert_eq!(state.command_line_text().as_deref(), Some("/foo"));

        assert_eq!(
            state.parse_key('\u{8}'),
            Some(VimAction::CommandLineChanged)
        );
        assert_eq!(state.command_line_text().as_deref(), Some("/fo"));
        assert_eq!(
            state.parse_key('\n'),
            Some(VimAction::SubmitSearch("fo".to_owned()))
        );
        assert_eq!(state.command_line_text(), None);

        assert_eq!(state.parse_key(':'), Some(VimAction::CommandLineChanged));
        for key in "12".chars() {
            let _ = state.parse_key(key);
        }
        assert_eq!(state.parse_key('\n'), Some(VimAction::SubmitGotoLine(12)));

        let _ = state.parse_key(':');
        let _ = state.parse_key('q');
        assert_eq!(state.parse_key('\n'), Some(VimAction::ExitVimMode));

        let _ = state.parse_key(':');
        let _ = state.parse_key('w');
        assert_eq!(
            state.parse_key('\n'),
            Some(VimAction::WriteFile { exit_vim: false })
        );

        let _ = state.parse_key(':');
        let _ = state.parse_key('w');
        let _ = state.parse_key('q');
        assert_eq!(
            state.parse_key('\n'),
            Some(VimAction::WriteFile { exit_vim: true })
        );

        let _ = state.parse_key('/');
        let _ = state.parse_key('x');
        assert_eq!(
            state.parse_key('\u{1b}'),
            Some(VimAction::CommandLineChanged)
        );
        assert_eq!(state.command_line_text(), None);
    }

    #[test]
    fn vim_pending_keys_formats_counts_prefixes_and_operators() {
        let mut state = VimState::default();

        assert_eq!(state.parse_key('5'), None);
        assert_eq!(state.pending_keys(), "5");
        assert_eq!(state.parse_key('d'), None);
        assert_eq!(state.pending_keys(), "5d");
        assert_eq!(state.parse_key('2'), None);
        assert_eq!(state.pending_keys(), "5d2");

        state.enter_clean_normal_mode();
        assert_eq!(state.parse_key('3'), None);
        assert_eq!(state.parse_key('g'), None);
        assert_eq!(state.pending_keys(), "3g");
    }

    #[test]
    fn vim_status_line_formats_mode_command_and_pending_input() {
        let mut state = VimState::default();
        assert_eq!(
            state.status_line_text(),
            ("-- NORMAL --".to_owned(), String::new())
        );

        let _ = state.parse_key('i');
        assert_eq!(
            state.status_line_text(),
            ("-- INSERT --".to_owned(), String::new())
        );
        let _ = state.parse_key('\u{1b}');

        let _ = state.parse_key('5');
        let _ = state.parse_key('d');
        assert_eq!(
            state.status_line_text(),
            ("-- NORMAL --".to_owned(), "5d".to_owned())
        );

        state.enter_clean_normal_mode();
        let _ = state.parse_key('/');
        let _ = state.parse_key('f');
        let _ = state.parse_key('o');
        let _ = state.parse_key('o');
        assert_eq!(
            state.status_line_text(),
            ("/foo".to_owned(), String::new())
        );

        let _ = state.parse_key('\u{1b}');
        let _ = state.parse_key('v');
        assert_eq!(
            state.status_line_text(),
            ("-- VISUAL --".to_owned(), String::new())
        );

        let _ = state.parse_key('V');
        assert_eq!(
            state.status_line_text(),
            ("-- VISUAL LINE --".to_owned(), String::new())
        );
    }
}
