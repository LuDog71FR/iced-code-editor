# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- feat: **Command palette** (`Ctrl+Shift+P`)
  - Opens a filtered list of every editor action available right now; typing narrows it, `Up`/`Down` move through the results, `Enter` runs the highlighted one and `Escape` closes. Each row displays that command's own keyboard shortcut, so the palette doubles as a way to discover them — which is the point, since most of the editor's ~20 actions were previously reachable only by already knowing the combination
  - Filtering is a case-insensitive *subsequence* match rather than a substring one, so `tc` finds "Toggle Line Comment" and `fldall` finds "Fold All". Both sides fold through `char::to_lowercase`, so it works beyond ASCII
  - Only usable commands are listed: undo does not appear with an empty history, cut/copy stay out without a selection, and the folding commands are absent while folding is disabled. This deliberately differs from the context menu, which dims unavailable entries instead — a menu with a stable shape supports muscle memory, while a search result list should only offer runnable rows, which also keeps arrow navigation free of unselectable stops
  - **Extensible registry**: a host application registers its own commands with `set_custom_command_palette_entries` / `with_custom_command_palette_entries`, and receives `Message::CommandPaletteAction(id)` when one is run. Entries reuse `ContextMenuItem` rather than a parallel type — the two surfaces describe the same thing (stable `id`, `label`, `shortcut` hint, `enabled` flag), so an action offered in both is declared once and routes through one host handler. The demo app registers eight of its own (open file, save as, new tab, close tab, run, clear log, settings, format document) and now dispatches context-menu and palette actions through a single `handle_app_action`
  - Running a command emits it as a `Task` instead of applying it in place, so it travels back out through the host exactly as it would if the user had pressed the shortcut. That is what lets the actions the editor cannot perform itself — `WriteRequested`, `RevealInFileManager`, and every host-registered command — reach the handler that already intercepts them
  - Toggle the whole feature with `set_command_palette_enabled(bool)` (frees `Ctrl+Shift+P` for a host-provided palette), and hide just the built-ins with `set_default_command_palette_enabled(bool)`; `open_command_palette()` / `close_command_palette()` drive it from a menu item or toolbar button
  - The palette's `text_input` holds focus while open, so `Escape` would merely unfocus it and the arrow keys would move the caret inside the query. A transparent canvas layer stacked over the dialog captures `Escape`/`ArrowUp`/`ArrowDown` first — the same trick the go-to-line dialog already used for `Escape` alone. In `escape_shortcut` the palette is the innermost dialog: it closes before go-to-line, which closes before search
  - The 18 new UI strings are translated into all eight supported languages; the labels shared with the context menu (undo, redo, cut, copy, paste, select all, reveal in file manager) reuse the existing keys rather than duplicating them
  - Covered by 20 unit tests (subsequence matching including non-ASCII, registry ordering, omission of unavailable actions, wrapping navigation, filter-resets-highlight, built-in vs host submission, disabled palette), 2 i18n tests spanning all 18 keys × 8 locales, and 8 interface tests driven through the real widget tree — including one asserting that `ArrowDown` moves through the list *without* moving the caret in the buffer, which is the behavior the canvas key listener exists to provide

- feat: **Inline color previews**
  - Every color literal in the visible text now gets a small filled square drawn just after it, so a palette reads at a glance instead of being decoded by eye. Recognized notations: CSS hexadecimal (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, short forms expanded by digit doubling), the Rust-style `0xrrggbb`/`0xrrggbbaa` spelling, and functional `rgb()`/`rgba()` with integer, float or percentage components
  - Hexadecimal digit runs are taken at their maximum length and *then* validated, so `#12345` and `#1234567` are rejected outright rather than truncated to a shorter valid color, and a literal that would continue an identifier (`raw0xff0000`, `#deadbeefcafe`) is ignored
  - The swatch is drawn as canvas geometry rather than text. Since Iced renders all text above all geometry, a square that extends under the following character (`#fff;`) leaves that character fully readable. Its frame uses `Style::gutter_border` so a color close to the editor background stays visible, and translucent colors are composited over `Style::background` so their opacity reads correctly
  - A literal split by soft wrapping is drawn once, on the visual segment holding its last character
  - Enabled by default; toggle via `set_show_color_previews(bool)` / `show_color_previews()` / `with_show_color_previews(bool)`, with a checkbox in the demo app's editor options panel
  - Detection lives in `features/color_preview.rs` as a pure function over a line of text, covered by 17 unit tests (each notation, short-form expansion, uppercase digits and prefixes, percentages, alpha, several literals per line, character-based columns on non-ASCII lines, clamping of out-of-range components, and every rejection rule)

- feat: **Indentation guides**
  - A thin vertical line is drawn at every indentation level, so the nesting of a block is visible without following its braces. Guide spacing follows the editor's configured `IndentStyle`, meaning 2-space, 4-space, 8-space and tab-indented documents each get guides where their own indent levels actually fall, rather than at a fixed stride
  - Blank lines take the *smaller* of the levels of the nearest non-blank line above and below them: a blank line inside a block keeps the block's guides, while a blank line between two blocks does not sprout guides that lead nowhere. The lookup is bounded at 200 blank lines in each direction, so a file that is mostly empty cannot turn every visible line into a full-buffer scan
  - Guides are deliberately not drawn on wrapped continuation segments. Every visual line starts drawing at the same base X, so a guide placed at its original column would land on top of the wrapped text instead of in its indentation
  - Enabled by default; toggle via `set_show_indent_guides(bool)` / `show_indent_guides()` / `with_show_indent_guides(bool)`, with a checkbox in the demo app's editor options panel
  - `Style` gains an `indent_guide_color` field, derived automatically from the active Iced theme. It is fainter than `whitespace_color`, because a guide spans the full line height and would otherwise compete with the code itself
  - The level computation lives in `features/indent_guides.rs` as a pure function over the buffer, covered by 10 unit tests (space and tab indentation, a zero-width indent unit, out-of-bounds lines, blank lines inside/at the end of/between blocks, buffer edges, an indent that is not a multiple of the unit, and the bounded blank-line scan)

### Changed

- refactor: **The context menu's action-availability snapshot and shortcut hints moved to a shared `features/actions.rs`**
  - `context_menu.rs` privately owned `MenuState` ("can undo? has a selection?") and the six platform-dependent shortcut strings, even though neither is a context-menu concept. The command palette needs exactly the same two things, and copying them would have left two lists of shortcut spellings free to drift apart on the next rebinding
  - Both moved to a new `features/actions.rs` as `ActionContext` (extended with `search_replace_enabled` and `folding_enabled`, which the palette needs) and a block of `pub(crate)` hint constants covering all 20 commands. `CodeEditor::action_context()` builds the snapshot once and both surfaces read from it, replacing the four fields `render/view.rs` used to compute inline for the menu alone. No behavior change; the snapshot itself gained 3 unit tests, which it had none of while it was a bare struct filled in at the call site

- refactor: **`indent_width` moved from `features/folding` to `metrics`**
  - Code folding had the only copy of "how wide is this line's indentation, with tabs expanded", as a private function, even though it is a measurement rather than a folding concept and `metrics.rs` already owns `TAB_WIDTH`, `measure_char_width` and `measure_text_width`
  - Promoted to `pub(crate)` and moved there so indentation guides could reuse it instead of growing a second copy that would drift on the next change to tab handling. Its two unit tests moved with it; folding now imports it. No behavior change

## [0.4.1] - 2026-08-20

### Fixed

- fix: **`Ctrl+.` (toggle fold at cursor) was unreachable on layouts where `.` requires Shift**
  - `handle_keyboard_shortcuts` matched the fold-toggle binding against the base `key` only; on French AZERTY, `.` is `Shift+;`, so `key` reports `;` and the shortcut never fired, while `Ctrl+/` (toggle comment) already handled this correctly by also checking `modified_key`
  - Both bindings now share a new `is_key_char(key, modified_key, ch)` helper that checks either key, so `Ctrl+.` works on AZERTY and future symbol shortcuts can't regress the same way

- fix: **LSP document mirror could desynchronize from the real document and keep serving stale positions silently**
  - `TextModel::apply_change` (the internal per-document copy used to convert cursor positions to LSP's UTF-16 columns) silently ignored a change whose range fell outside the tracked lines instead of reporting the failure; the change was still forwarded to the language server, so the local mirror and the server's copy diverged with no error and no recovery — every later hover, completion, and go-to-definition on that document then silently computed positions against the wrong text
  - `apply_change` now reports whether it applied the change; if a batch desynchronizes partway through, none of it is forwarded to the server, the document is dropped from the client's tracked documents so the next `did_open` reseeds it from scratch, and an `LspEvent::Log` is emitted so the desync shows up in the demo app's log pane instead of only as "the language server gives nonsense answers"

- fix: **`Ctrl+K`/`Ctrl+J` (fold/unfold all) fired even with Alt held**
  - Unlike the `Ctrl+Alt+Up/Down` multi-cursor and `Alt+Up/Down` line-move shortcuts a few lines above, which each explicitly exclude the other modifier, the fold-all/unfold-all bindings only excluded Shift — so `Ctrl+Alt+K`/`Ctrl+Alt+J` folded/unfolded everything, leaving no room for a future `Ctrl+Alt+K`/`J` binding
  - Both now exclude Alt as well, matching the surrounding shortcuts' convention

- fix: **Escape was always captured by the editor, even when it had nothing to do**
  - With no search/goto-line dialog open, Vim disabled, and a single cursor, pressing Escape published a `CloseSearch` message that was a pure no-op, and the key event was still marked captured — a host application embedding the editor could never see an Escape press to close its own modal, leave fullscreen, etc.
  - The editor now leaves the event uncaptured (returns `None`) in exactly that case; Escape still closes the active dialog, forwards to Vim, and — unchanged — collapses a multi-cursor selection down to the primary cursor when more than one cursor is active

- security: **LSP jump-to-definition's workspace-confinement check could accept a path traversal through a nonexistent directory component**
  - `is_lsp_jump_target_allowed` canonicalized the candidate path to compare it against the workspace root, but silently fell back to the *uncanonicalized* path when `canonicalize()` failed (e.g. because a leading path component doesn't exist) — a traversal like `<cwd>/missing/../../etc/passwd` still lexically starts with `<cwd>` and would have passed the `starts_with` check on that fallback
  - Now fails closed: if the candidate can't be canonicalized, it's rejected — a target that can't be canonicalized can't be opened anyway, so nothing legitimate is lost

- security: **LSP client's pending-request map grew without bound if the language server stopped responding**
  - Each hover/completion/definition request added an entry to `pending_requests`, removed only when a matching response arrived; a hung or crashed server left every outstanding entry there for the client's whole lifetime
  - Requests older than 30 seconds are now evicted whenever a new one is registered, bounding the growth

- security: **A panic inside a caller's `Command` implementation permanently bricked `CommandHistory`**
  - `CommandHistory` locked its shared state with `.lock().unwrap()` at all 14 call sites, opted out of the workspace's `unwrap_used` and `missing_panics_doc` lints module-wide, and justified it with "the Mutex cannot be poisoned in our single-threaded context". Both halves of that were wrong: poisoning does not require a second thread, and `undo`/`redo` run `Command::undo`/`Command::execute` *while holding the guard*. Since `Command` is a public trait and `push` accepts `Box<dyn Command>` from library users, a panic in a downstream implementation poisoned the mutex on that very thread — after which every later call panicked, including `can_undo()` on the render path, and on every `Clone` of the handle, since they all share one `Arc<Mutex<_>>`
  - All 14 sites now go through one private `lock_inner()` that recovers the guard with `unwrap_or_else(|error| error.into_inner())`, matching the policy the LSP client already applied to its own shared state at all 10 of its lock sites — the two modules had opposite policies with nothing explaining the split. The only invariant a poisoned `HistoryInner` can break is "the undo stack matches the buffer", which a caller recovers from with `clear()`, so recovering the guard is strictly better than an unrecoverable panic loop
  - Both `#![allow(...)]` lines are gone, so `missing_panics_doc` applies to this module again — with no panic left to document, no `# Panics` section was needed
  - Covered by a regression test that poisons the mutex through a real unwind and asserts the history stays both readable and usable afterwards; it is gated on `#[cfg(panic = "unwind")]` because the release profile sets `panic = "abort"`, where the panic cannot be caught at all

### Added

- feat: **`compute_text_change` is now reachable from outside the crate**
  - The function was already `pub` and documented as part of the LSP integration surface, but it sits in a private module and — unlike `LspClient`, `LspDocument`, `LspPosition`, `LspRange`, and `LspTextChange` next to it — was never re-exported from `lib.rs`, so no downstream crate could name it. Found while writing its doctest, which could not compile
  - Added to the existing LSP re-export group; purely additive

### Changed

- docs: **Every public API item now carries a runnable `# Example`**
  - The project rule is "every public item, with examples", but nothing enforced the example half: `missing_docs = "deny"` only checks that a doc comment exists. 109 of the 178 items reachable from the crate's public API (61%) had prose but no example, and the gap tracked nothing systematic — `config.rs` documented `set_wrap_enabled` thoroughly and its paired getter `wrap_enabled` not at all; `history.rs` had examples on `new`/`clear`/`max_size`/`undo_count` but not on `push`, `undo`, `redo`, `mark_saved`, or `is_modified`, the five methods a user actually calls
  - All 109 filled in, worst-first: `config.rs` (25), `lsp/sync.rs` (15), `editing/history.rs` (14), `metrics.rs` (11), `i18n.rs` (10), `features/context_menu.rs` (9), `lsp/mod.rs` (7), `canvas_editor/mod.rs` (7, including the `CodeEditor` and `Message` types themselves), and 11 across seven other files
  - The examples assert behavior rather than merely compiling, so they are also tests: defaults are pinned (`wrap_enabled()` is true, `indent_style()` is `Spaces(4)`, `reveal_in_file_manager_enabled()` is false), round-trips are checked, and the LSP examples implement a small recording `LspClient` to show what the editor actually sends on `did_open`/`did_change`/`did_save`. Doctests went from 79 to 188
  - Three methods (`CommandHistory::push`/`undo`/`redo`) take types that are `pub` but unreachable from outside the crate (`Command`, `TextBuffer`), so their examples demonstrate the same behavior through `CodeEditor` and the docs now say so explicitly instead of implying a call the reader cannot make

- docs: **The 18 remaining `ignore`d doctests are no longer advertised as suppressed tests**
  - Every one of them documents a private or crate-internal item (`char_to_byte_index`, `line_comment_token`, `insert_text_at`, `Cursor`/`CursorSet`, `bracket_pair`, `is_key_char`, `matching_close`, `frame_message`, `scrollable_rail`, `WrappingCalculator`), which a doctest cannot reach because it compiles as an external crate — so `ignore` was never going to become a real test
  - Switched to `text`, which renders identically but stops claiming to be a test rustdoc declined to run. `cargo test` now reports 188 passing doctests and **0 ignored**, down from 18
  - `WrappingCalculator::new`'s example additionally showed `use iced_code_editor::canvas_editor::wrapping::WrappingCalculator;` — a path that does not exist, since `canvas_editor` is a private module. Same class of error as the `i18n` `t!` example fixed previously, and again hidden by `ignore`

- docs: **`cargo doc` is now clean under `-D warnings`**
  - Three broken intra-doc links, one of them pre-existing: `highlight_line_spans` linked to the private `CodeEditor::highlighted_line_cached`, which made `RUSTDOCFLAGS="-D warnings" cargo doc` fail before any of the changes above

- docs: **`AGENTS.md`/`CLAUDE.md` described a different project**
  - The "Code Style Guidelines" section claimed "async/await with tokio runtime" and told contributors to group imports after "external crates (sqlx, chrono, etc.)". None of `tokio`, `sqlx`, or `chrono` appear in any `Cargo.toml` in the workspace — concurrency here is `iced::Task`, plus `std::thread`/`std::sync::mpsc` in the LSP client. These lines look inherited from a template, and they are the first thing a new contributor or agent reads
  - Corrected, and extended with the two conventions that were previously only discoverable by reading the root `Cargo.toml` or getting it wrong: the full workspace lint set (and the `[lints] workspace = true` opt-in each member needs), and the mutex-poisoning policy

- refactor: **Collapsed the demo app's eleven editor-toggle checkboxes into one data-driven path** (demo app)
  - Each boolean editor setting (line wrapping, folding, auto-indent, auto-close brackets, search/replace, line numbers, show whitespace, bracket-match highlight, bracket-pair colorization, Vim, LSP) had its own `Message` variant, its own near-identical `update` handler, and its own hand-written checkbox; five of the eleven handlers never logged the change while the other six did, an inconsistency visible in the log pane
  - Replaced with a single `EditorToggle` enum (`demo-app/src/types.rs`) that maps each setting to its `CodeEditor` getter/setter pair, one `Message::ToggleEditor(EditorId, EditorToggle, bool)` variant, one `handle_toggle_editor` handler, and one checkbox built per entry in `EditorToggle::ALL` — all eleven toggles now log consistently, and the LSP checkbox label changed from "Enable LSP" to "LSP" to match the other checkboxes' phrasing
  - No other behavior changes; covered by new unit tests in `types.rs`

- docs: **`ArrowDirection`'s four variants were undocumented, and nothing enforced that this couldn't happen again**
  - `ArrowDirection::{Up, Down, Left, Right}` had no doc comments — the only `missing_docs` gap in the crate; added one line per variant
  - Added `missing_docs = "deny"` to `[workspace.lints.rust]` so an undocumented public item is now a compile error across the whole workspace instead of something a reviewer has to notice manually; the crate-root-level `missing_docs` requirement on integration test binaries and the criterion-macro-generated bench entry point are satisfied with a short `//!` header and a scoped `#![allow(missing_docs)]` respectively

- docs: **Two `ignore`d doctests on public API were never compiled, and one of them documented a macro this crate doesn't actually expose**
  - `CodeEditor::reset`'s example used only public API and now compiles as `no_run` on every `cargo test`
  - The `i18n` module doc's example told readers to `use iced_code_editor::t;` and call `t!(...)` directly — but this crate doesn't re-export `rust-i18n`'s `t!` macro, so the example was never valid and `ignore` had been hiding that. Rewritten to show the crate's actual public API (`Translations::new(language).search_placeholder()`, matching every other example in the module) as a normal, fully-run doctest

- refactor: **Split up the two largest files flagged by the project's 1000-line rule** (no behavior change)
  - `handle_keyboard_shortcuts` (`iced-code-editor/src/canvas_editor/input/events.rs`) was a single 316-line function encoding ~25 shortcuts' precedence as statement order, which is exactly the shape that let the AZERTY fold-toggle bug and the Ctrl+Alt+K fold-all bug slip through earlier. Split into named groups (`vim_toggle_shortcut`, `clipboard_shortcut`, `history_shortcut`, `dialog_shortcut`, `escape_shortcut`, `multi_cursor_shortcut`, `editing_shortcut`, `line_move_shortcut`, `navigation_shortcut`, `folding_shortcut`, plus `focus_navigation_shortcut`), each independently documented, with `handle_keyboard_shortcuts` reduced to a 13-line `.or_else()` chain whose doc comment explains the one real ordering dependency (Vim-toggle before paste) and notes every other shared-key pair already excludes itself via its own modifier check
  - `demo-app/src/app.rs` (1653 lines) split into `app/message.rs` (the `Message` enum), `app/tabs.rs` (tab/editor creation, lookup, and event forwarding), `app/files.rs` (open/save/reveal and LSP jump-to-definition), and `app/settings.rs` (font/theme/language/per-editor toggles), following the same pattern already used for `app/app_lsp.rs`; `app.rs` itself is now 610 lines and holds only app state, `new()`, and the `update()`/`subscription()`/`theme()` entry points. Each of the 12 relocated unit tests moved into the file testing the code it covers

- test: **The keyboard-shortcut dispatch table is now covered end to end** (no behavior change)
  - A coverage measurement of the workspace found `input/events.rs` at 67.7% line coverage after the split above: the shortcut precedence rules had been made explicit and testable, but 8 of the 10 shortcut groups had no test at all — the same blind spot that let the AZERTY fold-toggle and `Ctrl+Alt+K` fold-all bugs through
  - Added 17 unit tests covering every group (clipboard, multi-cursor, editing, line-move, navigation, focus-navigation, history, dialog, escape, folding) and, more importantly, the cross-group precedence cases where two groups share a physical key: `Ctrl+Alt+Arrow` must add cursors rather than move lines, `Shift+Tab` must navigate focus unless the search dialog is open, Escape must close the goto-line dialog before the search dialog and must reach a dialog before Vim's modal state machine, and `Ctrl+Shift+K`/`J` must not fold-all
  - Also locked in two silent behaviors that a message-only assertion cannot see: paste is the one clipboard shortcut published *without* capturing the event (the host reads the real clipboard and re-dispatches the text), and the `Ctrl+F`/`Ctrl+H`/`F3` bindings go inert when `search_replace_enabled` is off while `Ctrl+G` does not
  - Every test routes through `handle_keyboard_shortcuts` rather than calling a group directly, since it is the group *ordering* that encodes the precedence rules; `events.rs` went from 67.7% to 83.3% line coverage, and the whole dispatch table is now exercised

- test: **`handle_mouse_event` had no tests at all** (no behavior change)
  - The canvas's entire mouse surface — click, double/triple click, Alt+Click, Ctrl+Click jump-to-definition, right-click context menu, drag, hover, release, fold-chevron click — sat at 0% coverage, including the routing that decides which of those a single left press becomes
  - Added 12 unit tests covering every branch, plus the precedence rule inside the left-press arm: the fold chevron wins over the modifier bindings, so Alt+Click on a chevron toggles the fold instead of dropping a cursor in the gutter
  - The tests use canvas bounds deliberately offset from the origin, because `handle_mouse_event` reports positions *relative* to the bounds: with bounds at (0, 0) a bounds-relative lookup and an absolute one are indistinguishable, and every position assertion would still pass if `position_in` were swapped for `position`
  - Capture behavior is asserted alongside the published message, since the two are independent and invisible to each other: a plain click and a hover stay uncaptured so they can bubble up for focus management, while double/triple click, Alt+Click, drag, release, and the context menu all capture
  - `handle_mouse_event` is now fully exercised (only its signature line is left uncovered); `events.rs` reached 91.1% line coverage and the workspace total moved from 71.0% to 72.4%

- test: **The two Replace handlers had no tests at all** (no behavior change)
  - `features/search/update.rs` sat at 38.3% region coverage, the lowest non-display figure in the library: its four tests covered dialog opening, Find Previous, and Tab focus cycling, while `handle_replace_next_msg` and `handle_replace_all_msg` — the only two handlers in the file that mutate the buffer and push undo commands — had zero coverage
  - Added 10 unit tests. For Replace All: every match across lines, single-undo restoration of the whole document (all replacements are wrapped in one `CompositeCommand`), a replacement that contains the query (`foo` → `foofoo`), a shrinking replacement (`X` → `""`), and the deliberate `MAX_MATCHES` bypass, exercised with a document holding `MAX_MATCHES + 10` matches so a regression that reused the capped display list would be caught
  - For Replace Next: that it replaces only the current match, that each call is its own undo entry (unlike Replace All), and that the cursor lands on the *following* match at the right column — which is what pins the otherwise invisible ordering dependency where the handler re-reads `current_match()` only after `finish_edit_operation` has refreshed the match list
  - Both no-match paths are asserted to push nothing to history, so an empty undo entry cannot creep in
  - The reverse-order iteration in Replace All is a load-bearing invariant with nothing previously guarding it: replacing left-to-right would leave every match after the first pointing at a stale column once the line length changed. Verified by mutation — removing `.rev()` fails exactly two of the new tests
  - `features/search/update.rs` went from 38.3% to 86.0% region coverage

- test: **`move_lines` and `duplicate_lines` had no tests** (no behavior change)
  - `input/update/line_ops.rs` sat at 54.9% region coverage: its three tests all covered `toggle_comment`, leaving the Alt+Up / Alt+Down / Shift+Alt+Up / Shift+Alt+Down handlers — keyboard-reachable, buffer-mutating, and undoable — entirely unexercised. The underlying `MoveLinesCommand`/`DuplicateLinesCommand` were already at 98%; what had no coverage was the handler logic layered on top of them
  - Added 12 unit tests for exactly that layer: the buffer-edge rejection, the cursor/anchor shift that keeps a selection attached to the lines it follows, the block-length (not one-line) shift when duplicating downward, duplication being legal at both edges where moving is not, single-undo restoration of a duplicated block, and the collapse of secondary cursors
  - Also pins `primary_line_range`'s VS Code convention: a selection ending at column 0 does *not* include that trailing line. The test distinguishes the two readings by which line gets moved, so an accidental off-by-one is caught rather than silently changing which lines an Alt+Down affects
  - The buffer-edge guard turns out to prevent a crash, not just a wrong result: `MoveLinesCommand::new` computes `cursor.0 - 1` for an upward move, so without the handler's `start == 0` rejection, Alt+Up on the first line panics on arithmetic underflow. Confirmed by mutation, along with the column-0 convention and the block-length shift — each mutation fails exactly the one test meant to catch it
  - `input/update/line_ops.rs` went from 54.9% to 99.8% region coverage; the workspace total moved from 75.5% to 76.7%

- security: **LSP frame headers and server stderr were read without any size bound**
  - `MAX_MESSAGE_BYTES` capped the frame *body*, and its doc comment states the threat plainly — "the body length is attacker-controlled input: it comes from the language server process". The header loop one call earlier had no such guard: `BufRead::read_line` grows its buffer until it finds a newline, so a server that opened a header line and never terminated it forced an unbounded allocation — the exact failure the body cap exists to prevent, on the path that reaches it. The stderr thread had the same hole via `BufRead::lines`
  - Header lines are now capped at 8 KiB each and 64 per frame. The line cap bounds memory; the line *count* bounds time, since each line is individually capped but an endless stream of well-formed headers would otherwise spin the reader forever without ever yielding a message. Both breaches abandon the stream, matching how an oversized body is already handled: the framing is exactly what is not trustworthy, so there is no safe point to resynchronise from
  - Stderr lines are capped at 8 KiB and, being free-form diagnostics rather than framed protocol, are truncated (marked with a trailing `…`) and reading resumes at the next line instead of tearing the stream down. Resynchronising discards through `fill_buf`/`consume` rather than `read_until`, so recovering from an unbounded line does not itself need unbounded memory, and is budgeted at 1 MiB so a line that never ends cannot hold the thread in the skip loop
  - Invalid UTF-8 on stderr is now replaced rather than fatal. Previously one bad byte ended the log thread and every later line with it
  - Covered by seven tests, including one driven by an endless reader that counts bytes served — a finite `Cursor` would end the line at EOF and so pass even with no cap at all

- security: **The LSP event queue was drained without a per-tick budget** (demo app)
  - `drain_lsp_events` emptied the unbounded event channel with an uncapped `loop`, so a server that floods it (rust-analyzer emits substantial output while indexing a large workspace) held the UI thread there for as long as the backlog took to process
  - Now capped at 256 events per tick, with the remainder picked up on following ticks — frame time is bounded instead of the queue. The receiver is explicitly preserved when the budget is what ends the loop; the previous code only restored it on the "channel empty" branch, so a budgeted exit would have dropped it and lost every later event
  - Note that the channel itself is still unbounded. Bounding it means changing `LspProcessClient::new_with_server` to take an `mpsc::SyncSender`, which breaks every caller — left as a deliberate API decision rather than folded into a hardening pass

- fix: **An environment-set language-server path was used untrimmed**
  - `resolve_program_from_envs_with` tested `!path.trim().is_empty()` but returned the value **untrimmed**, so what was validated and what was returned disagreed. `GOPLS=" /usr/bin/gopls"` — a shell-config typo, or a CI variable carrying a trailing newline — passed the guard and was then handed to `Command::new` verbatim, which fails with a confusing "No such file or directory" rather than pointing at the variable
  - The returned value is now trimmed, matching the emptiness check. Covered by a test that reproduces the old behavior when the trim is removed

- refactor: **`simple-example` inherited none of the workspace lints**
  - `iced-code-editor` and `demo-app` both carry `[lints] workspace = true`, but `simple-example` had no `[lints]` section at all, so the entire `[workspace.lints]` block — `unsafe_code = "forbid"`, `missing_docs`, `unwrap_used`, `panic`, all 25 of them — was silently inapplicable to a crate that `cargo build --workspace` still builds
  - Opted it in. This immediately surfaced what the exemption had been hiding: the crate had no documentation at all, failing `missing_docs`. Added a module header explaining what the example demonstrates (message forwarding, `view()`, and the focus hand-off that stops the editor swallowing keystrokes meant for another widget) plus doc comments on its state and message types

- refactor: **Split `lsp/process/mod.rs`, the last file over the project's 1000-line rule** (no behavior change)
  - The file had grown to 1 819 lines (1 200 of production code) holding four unrelated concerns: the process client, the wire format, the per-document mirror, and in-flight request tracking
  - Split into `protocol.rs` (framing, the bounded reads, message dispatch, response parsing — 432 production lines), `text_model.rs` (the UTF-16 position mirror — 144), and `pending.rs` (request tracking — 53), leaving `mod.rs` at 621 lines holding the process lifecycle and the `LspClient` implementation. The protocol functions were already free functions with no `self`, so the seam was there to be cut. Each of the 35 unit tests moved into the file testing the code it covers
  - No file in the workspace now exceeds 1 000 lines of production code
  - The split immediately exposed something the aggregate had been hiding: what read as one file at 63% coverage is really a well-tested transport and an untested client. `protocol.rs` is at 91.0%, `pending.rs` at 100%, `text_model.rs` at 89.5% — while `mod.rs`, which spawns the process and wires the reader/writer threads, sits at 6.6%. That number was previously averaged away

- refactor: **Every single-character keyboard shortcut now goes through `is_key_char`**
  - The helper was introduced to fix the AZERTY `Ctrl+.` bug, but applied to only the two symbol shortcuts that were actually broken — 2 of 19 call sites. Its own doc comment said the opposite of how it was used ("safe to use uniformly for any single-character shortcut"), leaving two spellings for the same check with the layout-aware one in the minority. Nothing was broken today, since the other 17 are letters; the cost was that the next symbol shortcut would most likely be copied from a raw call site and reintroduce the same bug
  - `modified_key` is now threaded through `vim_toggle_shortcut`, `write_shortcut`, `clipboard_shortcut`, `multi_cursor_shortcut`, `history_shortcut`, and `dialog_shortcut`, and all 17 remaining `matches!(key, Key::Character(c) if c.as_str() == …)` call sites migrated. The only raw form left in the file is the one inside `is_key_char` itself, so there is nothing left to copy and the rule enforces itself
  - Added a table-driven test covering all 15 single-character bindings with the character present *only* in `modified_key`. Letters would pass either way on a real layout, so this is the only thing that would catch one drifting back to a raw `key`-only check — verified by mutation: making `is_key_char` ignore `modified_key` fails it along with the two AZERTY tests

- refactor: **The "jump to current match" block was written three times in the search handlers**
  - `handle_search_query_changed_msg`, `handle_toggle_case_sensitive_msg`, and `handle_find_match` each repeated the same "move the primary cursor onto the current match, clear the selection, scroll it into view" body, and the copies had already drifted: two cleared `overlay_cache` before the block, the third inside it
  - Factored into `focus_current_match`. The overlay clear deliberately stays with the callers rather than moving into the helper: two of the three must clear it *even when nothing matched*, since a query that now matches nothing still has to erase the previous highlights — folding it in would have silently skipped exactly that path. That asymmetry is now stated at each call site instead of being an unexplained difference between three near-identical blocks
  - Behavior-preserving, and safe to do now that the Replace and Find handlers are covered

- refactor: **Removed two stale `#[allow(clippy::unused_self)]` attributes**
  - `handle_character_input` and `handle_mouse_event` (`input/events.rs`) both carry attributes left over from an earlier shape of those functions; both now use `self` extensively. The allows were not inert: `unused_self` is denied workspace-wide precisely to catch a method that has stopped depending on its receiver, and the comment block above the shortcut groups explicitly relies on that lint working. Verified that clippy stays clean without them

- test: **Nothing tested the demo app through its interface** (no behavior change, demo app)
  - Its 40 tests all called `DemoApp::update` with a hand-built `Message`. That proves a handler correct but says nothing about whether any widget emits that message: a button wired to the wrong variant, a shortcut the canvas never receives, or a dialog field that quietly stops accepting input would all have passed. The library's 39 shortcut tests in `input/events.rs` have the same blind spot one level down — they call `handle_keyboard_shortcuts` directly, so they never exercise the focus gate that decides whether the canvas is consulted at all
  - Added `demo-app/src/ui_tests.rs`: 53 tests that render the real `ui::view` in Iced's headless `Simulator` (`iced_test` 0.14, a new dev-dependency), click and type on the actual widgets, feed the messages that come back through `update`, and assert both on the resulting state and on what the next render shows. Covers the toolbar and tab bar, the settings modal, the editor options panel, typing and Enter, auto-indentation, auto-closing brackets, toggle-comment (including the AZERTY `modified_key` path that two earlier bugs came down to), move/duplicate line, undo/redo, cut/copy/paste, and the search & replace dialog end to end — query entry, the match counter, `F3`/`Shift+F3`, and the icon-only navigation and replace buttons
  - Every editing test opens by clicking the canvas, because `CodeEditor` drops keyboard events unless it holds focus. That gate is now covered in its own right, and its process-wide `FOCUSED_EDITOR_ID` is why a `Ui` handle serialises these tests for their whole duration: a test that lost the race would have had its keystrokes silently dropped rather than fail
  - Two limits are load-bearing and documented where they bite. The `Simulator` only sees the widget tree, never `DemoApp::subscription`, so the global Escape handling is covered by four new `update`-level tests in `app/app_lsp.rs` instead. And it builds a fresh widget tree per render, so a `text_input` keeps no focus and accumulates no text across renders — typing into the search field therefore re-clicks and re-renders once per character, as the real runtime does. Where a step needs the runtime itself (the clipboard read behind `Ctrl+V`), the test plays that role explicitly rather than pretending to cover it
  - Added `.cargo/config.toml` pointing `iced_test` at the software rasteriser. These tests never take a screenshot, so the GPU backend buys nothing and costs a great deal: building a wgpu adapter per simulator took the demo-app suite from roughly 5 seconds to 35, and concurrent adapter requests segfaulted the test binary. Not forced, so `ICED_TEST_BACKEND=wgpu cargo test` still works
  - Each group of tests was checked against an injected regression — a disabled `Alt+Up` branch, `is_key_char` stripped of its `modified_key` fallback, auto-indent no longer copying leading whitespace, `handle_replace_all_msg` made inert, the match counter hiding its index — and in every case exactly the tests that should have failed did, and no others


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
