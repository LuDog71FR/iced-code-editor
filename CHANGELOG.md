# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- fix: **`Ctrl+.` (toggle fold at cursor) was unreachable on layouts where `.` requires Shift**
  - `handle_keyboard_shortcuts` matched the fold-toggle binding against the base `key` only; on French AZERTY, `.` is `Shift+;`, so `key` reports `;` and the shortcut never fired, while `Ctrl+/` (toggle comment) already handled this correctly by also checking `modified_key`
  - Both bindings now share a new `is_key_char(key, modified_key, ch)` helper that checks either key, so `Ctrl+.` works on AZERTY and future symbol shortcuts can't regress the same way

- fix: **LSP document mirror could desynchronize from the real document and keep serving stale positions silently**
  - `TextModel::apply_change` (the internal per-document copy used to convert cursor positions to LSP's UTF-16 columns) silently ignored a change whose range fell outside the tracked lines instead of reporting the failure; the change was still forwarded to the language server, so the local mirror and the server's copy diverged with no error and no recovery — every later hover, completion, and go-to-definition on that document then silently computed positions against the wrong text
  - `apply_change` now reports whether it applied the change; if a batch desynchronizes partway through, none of it is forwarded to the server, the document is dropped from the client's tracked documents so the next `did_open` reseeds it from scratch, and an `LspEvent::Log` is emitted so the desync shows up in the demo app's log pane instead of only as "the language server gives nonsense answers"

### Changed

- refactor: **Collapsed the demo app's eleven editor-toggle checkboxes into one data-driven path** (demo app)
  - Each boolean editor setting (line wrapping, folding, auto-indent, auto-close brackets, search/replace, line numbers, show whitespace, bracket-match highlight, bracket-pair colorization, Vim, LSP) had its own `Message` variant, its own near-identical `update` handler, and its own hand-written checkbox; five of the eleven handlers never logged the change while the other six did, an inconsistency visible in the log pane
  - Replaced with a single `EditorToggle` enum (`demo-app/src/types.rs`) that maps each setting to its `CodeEditor` getter/setter pair, one `Message::ToggleEditor(EditorId, EditorToggle, bool)` variant, one `handle_toggle_editor` handler, and one checkbox built per entry in `EditorToggle::ALL` — all eleven toggles now log consistently, and the LSP checkbox label changed from "Enable LSP" to "LSP" to match the other checkboxes' phrasing
  - No other behavior changes; covered by new unit tests in `types.rs`

- docs: **`ArrowDirection`'s four variants were undocumented, and nothing enforced that this couldn't happen again**
  - `ArrowDirection::{Up, Down, Left, Right}` had no doc comments — the only `missing_docs` gap in the crate; added one line per variant
  - Added `missing_docs = "deny"` to `[workspace.lints.rust]` so an undocumented public item is now a compile error across the whole workspace instead of something a reviewer has to notice manually; the crate-root-level `missing_docs` requirement on integration test binaries and the criterion-macro-generated bench entry point are satisfied with a short `//!` header and a scoped `#![allow(missing_docs)]` respectively


## [0.4.0] - 2026-08-16

### Added

- feat: **Bracket-pair colorization** (rainbow brackets)
  - Each `(`, `)`, `[`, `]`, `{`, `}` is colored by its nesting depth, so a matching pair always shares the same color, cycling through a fixed gold/orchid/light-sky-blue palette as depth increases
  - Depth is tracked with a sequential per-line cache (mirrors the syntax-highlight cache), so edits only invalidate depths from the changed line onward instead of rescanning the file
  - Enabled by default; toggle via `set_bracket_pair_colorization_enabled(bool)` / `bracket_pair_colorization_enabled()`, with a checkbox in the demo app toolbar

- feat: **Matching bracket/quote highlight**
  - Placing the cursor next to a bracket (`(`, `)`, `[`, `]`, `{`, `}`) or a quote (`"`, `'`) highlights it and its matching pair
  - Brackets are matched by scanning the buffer with nesting-depth tracking; quotes on the same line are paired sequentially (1st with 2nd, 3rd with 4th, ...)
  - Enabled by default; toggle via `set_bracket_match_highlight_enabled(bool)` / `bracket_match_highlight_enabled()`, with a checkbox in the demo app toolbar

- feat: **Auto-closing brackets/quotes** with surround selection
  - Typing an opening bracket/quote (`(`, `[`, `{`, `"`, `'`) auto-inserts its matching closing character with the cursor placed between them
  - Typing the closing character right after an already-inserted match moves the cursor past it instead of duplicating it
  - Typing an opening bracket/quote while text is selected wraps the selection in the pair instead of replacing it
  - Enabled by default; toggle via `set_auto_close_brackets(bool)` / `auto_close_brackets()`, with a checkbox in the demo app toolbar

### Changed

- perf: **Case-insensitive search skips the per-character boundary table for ASCII lines**
  - The column-drift fix for Unicode case-folding built a `String` plus a `Vec<(usize, usize)>` boundary table for every line scanned, even for pure-ASCII text, where a byte offset already equals a character/column offset and no table is needed
  - ASCII lines now take a fast path (`to_ascii_lowercase` + direct byte offsets); non-ASCII lines keep the boundary-table path unchanged

- perf: **Undo-history trimming no longer shifts the whole stack on each discard**
  - `enforce_size_limit` dropped the oldest command with `Vec::remove(0)`, which shifts every remaining element down by one; shrinking a deep history via `set_max_size` was `O(k·n)`
  - `undo_stack` is now a `VecDeque`, so trimming pops from the front in `O(1)`

- ci: **Continuous integration now covers the whole workspace and every feature**
  - `build`, `clippy` and `test` run with `--workspace --all-features`; they previously used the default members with no feature enabled, so `demo-app`, `simple-example` and everything behind the `lsp-process` and `two-face` features was never built, linted or tested
  - Raises the number of tests actually executed in CI from 428 to 491
  - The workflow also runs on pull requests targeting `main`, not only on pushes to it, so incoming contributions are checked before they are merged rather than after
  - Closing this gap surfaced 23 pre-existing clippy errors in feature-gated test modules: `decode_sent` now borrows its input instead of taking it by value, and the `expect`/`panic!`/`unwrap` used to report test failures carry a scoped `#[allow]` on the individual test, matching the existing convention in `update.rs` and `selection.rs`

- refactor: **Removed duplicated code across the editor and demo app**
  - Multi-cursor descending-order sorting, LSP JSON-RPC framing/location parsing, text-insertion loops, history size-limit trimming, scrollbar styling, LSP client/document access, and several near-identical `update.rs` message handler pairs were each consolidated into a single shared helper
  - Demo app tab-creation logic (open file, jump-to-definition, new tab) now shares one `open_content_in_tab` helper instead of three copies
  - No functional changes intended, aside from the fix below; all new helpers are unit-tested

- refactor: **Overlay colors (search matches, selection, bracket match, IME preedit) moved into `Style`**
  - Search-match, text-selection, bracket-match and IME-preedit-background colors were hard-coded `Color` literals in the canvas rendering code instead of living on `theme::Style` like every other editor color, so they could not be customized and, in the case of the selection color, were duplicated verbatim in two places
  - `Style` gains `search_match_color`, `search_match_current_color`, `selection_color`, `bracket_match_color` and `ime_preedit_background_color`, set by `from_iced_theme` to the same values as before (theme-independent by design, to preserve the conventional orange/yellow search-highlight look); no visual change

- refactor: **Removed dead and vestigial code found during review**
  - `Message::FocusNavigationTab` was declared and matched but never actually produced (only `FocusNavigationShiftTab` is ever dispatched, from Shift+Tab); the unused variant and its match arm are removed (a breaking change for any host constructing it directly, though none in this workspace did)
  - `CompositeCommand::new` took a `description: String` that was immediately discarded (composite-command descriptions were never implemented); the parameter is dropped, along with the now-meaningless `description`/`label` arguments on `HistoryManager::begin_group` and the internal `ensure_grouping_started` helper that only forwarded it
  - No functional changes

- refactor: **Split `update.rs` by topic to make the message-handling code easier to navigate**
  - The Vim key-dispatch handlers (`handle_vim_*`, ~600 lines) moved to a new `vim_update.rs`; the search/replace and go-to-line dialog handlers (~300 lines) moved to a new `search_update.rs`. Both are sibling `impl CodeEditor` blocks, the same pattern already used by `cursor.rs`, `clipboard.rs`, `selection.rs`, etc.
  - `update.rs` shrinks from ~5000 to ~3500 lines; the corresponding unit tests moved with their handlers into each new file's own test module
  - No functional changes; a few purely-internal helpers (`finish_edit_operation`, `finish_navigation_operation`, `ensure_grouping_started`, `end_grouping_if_active`, `capture_lsp_edit_snapshot`, `handle_goto_position`) went from private to `pub(crate)` so the moved handlers can still call them across the new file boundary

- refactor: **Split `mod.rs`, `canvas_impl.rs` and `command.rs` into topic-focused files, continuing the `update.rs` split above**
  - `mod.rs`: the LSP attach/detach, hover/completion/definition request, and incremental-change-queueing methods (~600 lines) moved to a new `lsp_sync.rs`, the same sibling-`impl CodeEditor` pattern as `cursor.rs`/`clipboard.rs`
  - `canvas_impl.rs` (~3000 lines, mixing drawing and input handling) split five ways: `canvas_impl.rs` keeps only the `canvas::Program` trait glue (`draw`/`update`/`mouse_interaction`); `gutter.rs`, `text.rs` and `overlays.rs` each own one rendering layer (line numbers/fold chevrons, syntax highlighting, and selection/cursor/search highlights respectively); `events.rs` holds all keyboard/mouse/IME handling
  - `command.rs` (~1700 lines) became a `command/` directory module: `command.rs` now holds only the `Command` trait and re-exports, with `command/edit.rs`, `command/composite.rs`, `command/lines.rs` and `command/comment.rs` each owning one command family; every external `super::command::{Type}` import kept working unchanged since the re-exports preserve the original path
  - No functional changes; unit tests moved with the code they cover into each new file's own test module (472 tests pass, same count as before)

- refactor: **Split `update.rs` into a topic-focused `update/` directory module, continuing the split above**
  - `update.rs` (~3550 lines) became a `update/` directory module the same way `command.rs` did: `mod.rs` keeps the cursor-adjustment helpers and the `finish_edit_operation`/grouping helper methods shared by every handler, and 11 sibling files each own one message-handling area — `text_input.rs` (character input, auto-close/surround brackets, Tab, Enter, focus navigation), `line_ops.rs` (move/duplicate lines, toggle comment), `deletion.rs` (backspace/delete), `navigation.rs` (arrows, Home/End, Page Up/Down, go-to-position), `mouse.rs` (click/drag/context menu), `clipboard.rs` (cut/select-all/paste), `history_ops.rs` (undo/redo), `focus_ime.rs` (canvas focus, IME), `scroll_timer.rs` (cursor-blink tick, scroll), `multi_cursor.rs` (Alt+Click, add-cursor-above/below, select-next-occurrence), and `dispatch.rs` (the top-level `update()` match)
  - No functional changes; nothing outside `canvas_editor` referenced an `update::` module path, so no external call site needed to change. Unit tests moved with the handlers they cover into each new file's own test module (472 tests pass, same count as before)

- refactor: **Minor cleanups found during review**
  - `lsp_language_for_extension` lowercased its input and then compared it with `eq_ignore_ascii_case`, which is already case-insensitive; dropped the redundant allocation
  - `Ctrl+/` line-comment toggling now recognizes C, C++, Java, C#, shell scripts, Ruby, TOML and YAML in addition to the languages already covered (Rust, JS/TS, Go, Python, Lua); it was previously a silent no-op for all of them

- refactor: **Reorganized `iced-code-editor/src/` into topic-focused module directories, completing the file-splitting work above**
  - `canvas_editor/` was a flat directory of ~30 files with no grouping; it's now organized by concern: `editing/` (cursor, selection, clipboard, history, the `command/` undo-redo pattern), `input/` (keyboard/mouse/IME events, the `update/` message dispatch), `render/` (the `canvas::Program` impl and its gutter/text/overlay/wrapping/view layers), `features/` (bracket-match, context menu, folding, go-to-line, search, Vim — each in its own subdirectory), and `lsp/` (the `LspClient` trait, buffer sync, and the `process/` subprocess client). `text_buffer.rs`/`text_utils.rs` moved into a new crate-root `buffer/` module alongside them
  - `mod.rs` itself (~3400 lines) was split by topic into `caches.rs` (visual-line/highlight/bracket-depth/max-width caches), `metrics.rs` (font/char/line/viewport dimensions), `config.rs` (the builder-style `set_*`/`with_*`/getter methods), `focus.rs` (focus management), and `bench_support.rs` (the `cfg(feature = "bench")` criterion harness, previously an inline module); it now holds only the `CodeEditor`/`Message` type definitions and `new()`/`reset()`, ~640 lines
  - No public API change and no functional changes; `iced_code_editor::{CodeEditor, Message, ...}` re-export paths in `lib.rs` are unaffected, only internal `canvas_editor::` paths moved. 472 unit tests pass unchanged

### Fixed

- fix: **LSP completion was inserted character by character, triggering auto-close and re-entrant completion requests** (demo app)
  - `apply_completion` deleted the word being typed with individual `Backspace`s (fine) but then inserted the completion text one `CharacterInput` at a time, running each character through the full input pipeline: auto-close inserted a spurious `)`/`"` for any label containing `(` or a quote, and `.`/identifier characters re-triggered a new completion request mid-insertion
  - The insertion now goes through a single `EditorMessage::Paste`, which inserts the text verbatim (no auto-close, no re-triggered request) and is one undo step instead of one per character

- fix: **`current_match_index` could go stale after an edit shrank the match list without emptying it**
  - `update_matches_after_edit` only reset the current-match index when the match list became fully empty; an edit that removed some (but not all) matches could leave the index pointing past the end, making `current_match()` return `None` even though matches still exist
  - The index is now clamped to the last valid entry when it falls out of range, matching the full-search path in `update_matches`

- fix: **`gopls` discovery via `$GOPATH` only recognized `:` as the path-list separator**
  - `GOPATH` follows the same platform convention as `PATH` (`;` on Windows, `:` elsewhere); the hardcoded `:` split silently failed to find `gopls` under `$GOPATH/bin` on Windows
  - Now uses `std::env::split_paths`, which picks the correct separator for the target platform

- fix: **Multi-cursor selection deletion and paste were not grouped in the undo history**
  - With N cursors holding selections, deleting or pasting pushed N separate undo commands instead of one; a single undo only reverted the last cursor's edit instead of all of them, unlike the single-cursor case and unlike Cut (which already grouped)
  - Deleting/pasting across more than one cursor now groups every cursor's command into one composite, so a single undo restores all of them

- fix: **`lsp_did_save` serialized the whole buffer even when no LSP client was attached**
  - The `with_lsp` refactor moved the full-document `to_string()` call ahead of the attached-client check, so every save allocated a complete copy of the document even for hosts that never enable LSP
  - The call now returns early when no client/document is attached, before paying for the allocation

- fix: **`TextBuffer` round-trip dropped the trailing newline and normalized CRLF to LF on save**
  - Lines are stored without their terminator (`str::lines()` strips both a final `\n` and `\r` in `\r\n`), and `to_string()` used to reassemble them with a hardcoded `\n` and no trailing newline, so every POSIX-style file lost its final `\n` and every CRLF file was silently converted to LF on save — with no dirty-flag warning, since the history never saw an edit
  - `TextBuffer::new` now records the source's line-ending style (LF or CRLF, by majority vote) and whether it ended with a trailing newline; `to_string()` and the LSP incremental-sync helper `line_range_to_string` both honor this so a load/save round trip reproduces the exact bytes

- fix: **`DemoApp::new()` spawned a real LSP server subprocess as a constructor side effect, making unit tests behave differently depending on `PATH`** (demo app)
  - Startup synchronously tried to auto-attach a `lua-language-server` client for the bundled demo script. On a machine without that server on `PATH` this silently failed and was invisible; on a machine with it installed (e.g. via Neovim's Mason), every `DemoApp::new()` call — including in unit tests — got a real attached LSP client, which broke `test_process_lsp_hover_timers_clears_pending_when_ready`'s assumption that a fresh app has none
  - The auto-attach now goes through the same async `Task`/`Message::ToggleLsp` flow as the manual LSP toggle instead of running synchronously in the constructor; unit tests discard the returned `Task`, so they no longer spawn a process at all

- fix: **Reveal-in-file-manager was incorrectly enabled on wasm32 when opening a file via "jump to definition"** (demo app)
  - One of the three duplicated tab-creation code paths hardcoded the flag to `true` instead of checking the target platform; unified during the refactor above

- fix: **Matching-bracket highlight could scan the entire document on every redraw**
  - Placing the cursor next to an unmatched opening/closing bracket in a very large file made `scan_forward`/`scan_backward` walk the whole buffer looking for a counterpart that doesn't exist, on every overlay redraw
  - Both scans now give up after 5,000 lines; a genuine match further away is treated the same as "no match"

- fix: **Tab-overflow width estimate overestimated for non-ASCII (e.g. CJK) file names** (demo app)
  - `check_tabs_overflow` measured a tab label's width using its UTF-8 byte length, so a file name with multi-byte characters could trigger the overflow layout well before the tab bar was actually full
  - Width is now estimated from the label's character count instead

- fix: **`is_modified()` could report a modified document as saved after an undo**
  - Marking a document saved records the undo-stack depth at that moment. Undoing past that point and then making a *different* edit could grow the undo stack back to the same depth, so `is_modified()` returned `false` even though the buffer no longer matched what was actually saved on disk — a host application could let the user close the document without a save prompt while real changes were pending
  - The save point is now invalidated as soon as a new command is pushed (or a composite command ends) after the undo stack has been unwound past it, since the exact path back to the saved state has just been discarded

- fix: **Undo of a line-merging Backspace duplicated the merged line**
  - Pressing Backspace at column 0 merges a line into the previous one. Undo split the merged line back at the join point — which already restores both lines — and then re-inserted the line content on top of it, so `hello\nworld` came back as `hello\nworldworld`
  - The redundant re-insertion (and the now-unused `merged_content` field) has been removed; undo now restores the exact original buffer

- fix: **Undo of a line-merging Delete (forward delete) did unnecessary rework, same class of bug as the Backspace fix above**
  - `DeleteForwardCommand::undo` split the merged line back at the join point, then cleared the newly split-off line character by character and re-inserted the same content — the split alone already restores the exact original text, same as `DeleteCharCommand::undo` was already fixed to do
  - The redundant clear/re-insert loop (and the now-unused `next_line_content` field) has been removed

- fix: **Undo of a Vim paste (`p` / `P`) deleted the wrong text, or nothing at all**
  - `InsertTextCommand` removes pasted text by walking backwards from the cursor position it restores. Vim paste overrides that position to rest the caret *on* the pasted text instead of after it, so the backward walk started at the wrong place — `P` on a characterwise register left the paste in place entirely (`abc` → `aabc` → `aabc` after undo)
  - The end of the insertion is now tracked separately from the caret's resting position, so undo always removes exactly the inserted characters regardless of where the caret is left

- fix: **Panic when filtering LSP completions on lines with multi-byte characters** (demo app)
  - The completion filter sliced the current line using character offsets as byte offsets; when the boundary fell inside a character the application panicked (e.g. `aé` or `汉字` with the cursor at end of line), and when it happened to fall on a valid boundary it silently returned a truncated word
  - The word is now rebuilt from `chars()`, so accented, CJK and emoji content is handled correctly

- fix: **`file://` URIs are percent-encoded and decoded correctly** (demo app)
  - Encoding escaped only spaces and decoding escaped nothing, so "go to definition" into a path containing a space, `#`, `?` or a non-ASCII character opened a path that does not exist
  - Both directions now go through the `url` crate; URIs generated for plain ASCII paths are unchanged

- fix: **Bounded allocation for LSP server messages**
  - The LSP client allocated a buffer of whatever size the language server announced in `Content-Length`. Since the server binary is resolved through `PATH` or an environment variable, a malformed or hostile server could announce a multi-gigabyte frame and exhaust memory
  - Frames are now capped at 64 MiB, well above any realistic LSP payload; an oversized frame stops the read loop, because the announced length is precisely what cannot be trusted to resynchronise on
  - Message framing was extracted into a dedicated helper and is now covered by unit tests

- fix: **Case-insensitive search reported the wrong column for a match after a length-changing Unicode character** (e.g. `İ` lowercasing to two characters)
  - The match's byte offset, found in the lowercased line, was re-sliced against the original line; this silently drifted once an earlier character's case-folding changed the character count
  - Column is now computed via an explicit character-boundary map back to the original line instead of re-slicing it

- fix: **Deleting or undoing a large multi-line selection was quadratic in the size of the selection**
  - `DeleteRangeCommand` removed a range by calling `delete_forward` once per character, and its `undo` re-inserted the deleted text one character at a time; each call re-scanned the current line, so a large Select All + Delete (or its undo) in a big file could visibly hang the UI
  - Both directions now splice whole lines in bulk (truncate/merge the boundary lines with `replace_range`, drop or reinsert the fully-consumed lines with `remove_line`/`insert_line`), turning the cost into O(text touched + lines affected) instead of O(n²)

- fix: **LSP "go to definition" could open any file readable by the process, at the language server's request** (demo app)
  - The `Definition` response's target URI comes straight from the language server process, which is untrusted input; a malicious or compromised server could answer a jump request with a path like `~/.ssh/id_ed25519` and the demo would read and display it without confirmation
  - Jump targets are now confined to the current workspace root: `handle_jump_to_file` rejects (and logs) any path that does not canonicalize to somewhere under the working directory

## [0.3.11] - 2026-08-03

### Added

- feat: **Optional Vim mode**
  - Per-editor opt-in API via `set_vim_enabled` / `with_vim_enabled`, with `vim_mode` for status display
  - Normal, Insert, Visual, and Visual Line modes with counts, common motions, operators, paste, and undo/redo
  - Counted line targets and operators, including `5G`, `5gg`, `5yy`, `5dd`, and `5cc`
  - Per-editor character-wise/line-wise unnamed register; system clipboard shortcuts remain available
  - Mode-aware bar/block cursor feedback and a per-tab toggle/status label in the demo app
  - `Ctrl+Alt+V` / `Command+Alt+V` toggles Vim behavior for the focused editor without replacing system paste
  - Fixed bottom status line for modes, pending Normal keys, and active `/` or `:` input
  - Forward `/` search with `n`/`N` repeat and `:N` 1-based line jumps
  - `:w` host save requests plus `:q` and `:wq` commands to leave Vim mode

- feat: **Visible whitespace rendering**
  - Spaces are rendered as `·` and tabs as `→` (followed by `·` fill characters to preserve alignment)
  - Whitespace symbols are drawn in a dedicated dimmed color (`Style::whitespace_color`) to stay non-intrusive
  - Enabled by default; toggle via `set_show_whitespace(bool)` / `show_whitespace()`
  - `Style` gains a `whitespace_color` field, automatically derived from the active Iced theme

- feat: **Toggle comment**
  - `Ctrl+/` comments the current line, or the lines spanned by the selection
  - Toggles back to uncommented when every non-blank line in the range is already commented
  - Indentation-aware (the comment token is inserted after the leading whitespace)
  - Per-language line-comment tokens (`//` for Rust/JS/TS/Go, `#` for Python, `--` for Lua); a no-op for languages without a line comment
  - Fully undoable/redoable through the command history

- feat: **Double/Triple-click selection** ([#22](https://github.com/LuDog71FR/iced-code-editor/issues/22))
  - Double-click: selects the word under the cursor
  - Triple-click: selects the entire line under the cursor

### Fixed

- fix: Stale-anchor bug ([#21](https://github.com/LuDog71FR/iced-code-editor/issues/21))
  - Fixed an issue where clicking to place the cursor and then editing (Backspace, Delete, typing, Tab, or Paste) could leave a stale selection anchor.

## [0.3.10] - 2026-06-24

### Added

- feat: **Move and duplicate lines**
  - `Alt+Up` / `Alt+Down` move the current line (or the selected line range) up/down
  - `Shift+Alt+Up` / `Shift+Alt+Down` duplicate the current line (or the selected line range) above/below
  - Fully undoable/redoable through the command history

- feat: Criterion benchmark suite for performance-critical paths

### Changed

- perf: **Faster syntax highlighting**
  - Each line is highlighted once and cached, then reused across frames, scrolling and wrapped segments; an edit only re-highlights from the changed line onward
  - Optimized release profile (fat LTO, single codegen unit)
  - Removed `O(n)` character-to-byte conversions from the text rendering loop

- docs: Improve DEV.md
  - Add missing functionalities: multi-cursor, line wrapping, code folding, search & replace and auto-indentation
  - Fix divergences between code and documentation

- refactor: Consolidate UTF-8 char/byte conversion helpers into a shared `text_utils` module

- refactor: Deduplicate selection/search highlight rendering and page up/down cursor movement into shared helpers

### Fixed

- fix: Multi-line block comments and strings are now highlighted correctly across their entire span

## [0.3.9] - 2026-06-02

### Added

- feat: add optional dependency 'two-face' for additional syntaxes ([#18](https://github.com/LuDog71FR/iced-code-editor/issues/18))

- feat: **Code folding** for better code navigation
  - Collapse and expand code blocks with a single click or shortcut
  - Collapse and expand all code blocks with shortcuts

## [0.3.8] - 2026-04-11

### Added

- feat: **Multiple cursors** for simultaneous editing at multiple positions

- feat: **auto-indentation on Enter**: copies the leading whitespace of the current line to the new line
  - Toggle on/off via `set_auto_indent_enabled()` / `auto_indent_enabled()`

- feat: configurable indentation style via `IndentStyle` enum
  - `IndentStyle::Spaces(n)` (2, 4 or 8 spaces) or `IndentStyle::Tab`
  - Configure via `set_indent_style()` / `indent_style()`
  - Default: 4 spaces

### Fixed

- fix: Tab key was navigating to the next widget instead of inserting indentation
- fix: Example in README.md have error with focus and Input widget ([#17](https://github.com/LuDog71FR/iced-code-editor/issues/17))

## [0.3.7] - 2026-03-09

### Added

- feat: Handle horizontal scrolling when line wrapping is disabled ([#13](https://github.com/LuDog71FR/iced-code-editor/issues/13))
- feat: Language Server Protocol (LSP) support

## [0.3.6] - 2026-02-25

### Added

- feat: Handle focus without the needs to check if mouse is out of bounds ([#10](https://github.com/LuDog71FR/iced-code-editor/issues/10))
- feat: WASM compatibility optimization
- feat: Improve selection smoothness via layered canvas caching


## [0.3.4] - 2026-01-28

### Added

- feat: automatic syntax highlighting for all file extensions supported by syntect

### Fixed

- fix: crashing when searching for "a" in a file with 99,000 or more entries and only 110,000 matches
- fix: lag when performing a full replacement on a file with 100,000 or more entries
- fix: crashing when searching for Chinese characters
- fix: text disappear when scrolling with mouse ([#7](https://github.com/LuDog71FR/iced-code-editor/issues/7))

## [0.3.3] - 2026-01-22

### Fixed

- fix: 中文 will panicked ([#9](https://github.com/LuDog71FR/iced-code-editor/issues/9))

### Added

- feat: add support for Asian character input in the editor
- feat: Add support for CJK font
- feat: allow changing the font of the editor
  Default font: iced::Font::MONOSPACE

## [0.3.2] - 2026-01-16

- fix: keyboard events are interpreted when editor has no more the focus ([#6](https://github.com/LuDog71FR/iced-code-editor/issues/6))
- fix: reduce gutter for line numbers
- feat: hide/display line numbers ([#5](https://github.com/LuDog71FR/iced-code-editor/issues/5))
- feat: hide cursor if editor don't have the focus.

## [0.3.1] - 2026-01-11

### Fixed

- fix: duplicate char with two widgets on the window ([#4](https://github.com/LuDog71FR/iced-code-editor/issues/4))
- fix: panic with not english chars ([#3](https://github.com/LuDog71FR/iced-code-editor/issues/3))


## [0.3.0] - 2026-01-09

### Changed

- **BREAKING**: Removed `theme::dark()` and `theme::light()` functions
- **BREAKING**: Changed default theme to use `theme::from_iced_theme()` which auto-adapts to any Iced theme

### Added

- feat: Search and replace text

  - Dialog box to search/replace text
  - Pagination thru results
  - Replace one by one or all
  - Undo capability
  - translations file created for en, fr and es (in `locales/` folder)

- feat: line wrapping

  - Long lines are split into multiple visual lines at viewport width
  - Continuation lines display a ↪ indicator in the gutter
  - Toggle feature on/off via checkbox in editor toolbar
  - Cursor navigation and text selection work across wrapped lines

- feat!: native support for all built-in Iced themes

  - New `theme::from_iced_theme()` function that automatically adapts editor colors to any Iced theme palette
  - Color helper functions for optimal code editor appearance (darken, lighten, dim_color, with_alpha)
  - Demo app now uses native Iced theme system with full theme picker

## [0.2.9] - 2026-01-08

### Fixed

fix: prevent visual artifacts when switching to shorter content
Use the new `reset()` function instead of creating again a new code editor !
fix: prevent mouse to capture events when out of bounds

## [0.2.8] - 2026-01-08

### Fixed

fix: prevent editor background overflow when resizing panes

## [0.2.7] - 2026-01-08

### Fixed

fix: scrollable height now respects parent container bounds

## [0.2.6] - 2026-01-07

### Fixed

fix: canvas background now respects viewport height instead of content height

## [0.2.5] - 2026-01-03

### Added

- Add html, xml, css, json and md languages ([#2](https://github.com/LuDog71FR/iced-code-editor/issues/2)).

## [0.2.4] - 2025-12-27

### Fixed

- Key Space not sending to iced-code-editor ([#1](https://github.com/LuDog71FR/iced-code-editor/issues/1))

### Changed

- Better handle keyboard entries

## [0.2.3] - 2025-12-19

### Fixed

- Fix example code in README & lib

## [0.2.2] - 2025-12-19

### Fixed

- Fix GitHub repository link in Cargo.toml

## [0.2.1] - 2025-12-19

### Added

- Add build badge in README.md

### Changed

- Fix GitHub repository link in README.md

## [0.2.0] - 2025-12-19

### Added

- Initial release on crates.io
- Canvas-based high-performance code editor widget
- Syntax highlighting for multiple programming languages (Python, Lua, Rust, JavaScript, etc.)
- Line numbers with styled gutter
- Text selection via mouse drag and keyboard shortcuts
- Clipboard operations (copy, paste)
- Undo/Redo functionality with smart command grouping
- Configurable command history with size limits
- Custom scrollbars with themed styling
- Dark and light themes with customizable colors
- Comprehensive keyboard navigation support:
  - Arrow keys (with Shift for selection)
  - Home/End keys
  - Ctrl+Home/Ctrl+End
  - Page Up/Page Down
- Modified state tracking for file save indicators
- Focus management for multiple editors
- Cursor blinking animation
- Demo application with file operations

### Documentation

- Complete README with examples and usage guide
- Inline documentation for all public APIs
- Working doctests for all examples
- Keyboard shortcuts reference
