//! LSP synchronization for [`CodeEditor`]: attaching/detaching a client,
//! tracking buffer edits as queued `didChange` notifications, and issuing
//! hover/completion/definition/formatting requests.

use crate::buffer::TextBuffer;
use crate::canvas_editor::CodeEditor;
use crate::canvas_editor::lsp;

impl CodeEditor {
    /// Attaches an LSP client and opens a document for the current buffer.
    ///
    /// This sends an initial `did_open` with the current buffer contents and
    /// resets any pending LSP change state. It is a no-op while LSP support is
    /// disabled — see [`Self::set_lsp_enabled`].
    ///
    /// # Arguments
    ///
    /// * `client` - The LSP client to notify
    /// * `document` - Document metadata describing the buffer
    ///
    /// # Example
    ///
    /// ```
    /// use std::cell::RefCell;
    /// use std::rc::Rc;
    ///
    /// use iced_code_editor::{CodeEditor, LspClient, LspDocument};
    ///
    /// /// Records the text the editor sent on `did_open`.
    /// struct RecordingClient(Rc<RefCell<Option<String>>>);
    ///
    /// impl LspClient for RecordingClient {
    ///     fn did_open(&mut self, _document: &LspDocument, text: &str) {
    ///         *self.0.borrow_mut() = Some(text.to_string());
    ///     }
    /// }
    ///
    /// let opened = Rc::new(RefCell::new(None));
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.attach_lsp(
    ///     Box::new(RecordingClient(Rc::clone(&opened))),
    ///     LspDocument::new("file:///tmp/main.rs", "rust"),
    /// );
    ///
    /// // Attaching opens the document with the buffer's current contents.
    /// assert_eq!(opened.borrow().as_deref(), Some("fn main() {}"));
    /// ```
    pub fn attach_lsp(
        &mut self,
        mut client: Box<dyn lsp::LspClient>,
        document: lsp::LspDocument,
    ) {
        if !self.lsp_enabled {
            return;
        }
        let (document, text) =
            open_lsp_document(client.as_mut(), &self.buffer, document);
        self.lsp_client = Some(client);
        self.lsp_document = Some(document);
        self.reset_lsp_shadow_state(text);
    }

    /// Opens a new document on the attached LSP client.
    ///
    /// If a document is already open, this will close it before opening the new
    /// one and reset pending change tracking. Use this when the same editor
    /// widget is reused for a different file, so the server is told about the
    /// swap rather than seeing the old document mutate into the new one.
    ///
    /// Does nothing when no client is attached.
    ///
    /// # Arguments
    ///
    /// * `document` - Document metadata describing the buffer
    ///
    /// # Example
    ///
    /// ```
    /// use std::cell::RefCell;
    /// use std::rc::Rc;
    ///
    /// use iced_code_editor::{CodeEditor, LspClient, LspDocument};
    ///
    /// /// Records the URIs the editor closed.
    /// struct ClosingClient(Rc<RefCell<Vec<String>>>);
    ///
    /// impl LspClient for ClosingClient {
    ///     fn did_close(&mut self, document: &LspDocument) {
    ///         self.0.borrow_mut().push(document.uri.clone());
    ///     }
    /// }
    ///
    /// let closed = Rc::new(RefCell::new(Vec::new()));
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.attach_lsp(
    ///     Box::new(ClosingClient(Rc::clone(&closed))),
    ///     LspDocument::new("file:///tmp/first.rs", "rust"),
    /// );
    ///
    /// // Switching files closes the previous document first.
    /// editor.lsp_open_document(LspDocument::new("file:///tmp/second.rs", "rust"));
    /// assert_eq!(closed.borrow().as_slice(), ["file:///tmp/first.rs"]);
    /// ```
    pub fn lsp_open_document(&mut self, document: lsp::LspDocument) {
        let Some(client) = self.lsp_client.as_mut() else { return };
        if let Some(current) = self.lsp_document.as_ref() {
            client.did_close(current);
        }
        let (document, text) =
            open_lsp_document(client.as_mut(), &self.buffer, document);
        self.lsp_document = Some(document);
        self.reset_lsp_shadow_state(text);
    }

    /// Resets LSP shadow-sync bookkeeping after (re)opening a document whose
    /// buffer contents are `text`.
    fn reset_lsp_shadow_state(&mut self, text: String) {
        self.lsp_shadow_text = text;
        self.lsp_shadow_is_current = true;
        self.update_lsp_synced_extent();
        self.lsp_edit_snapshot = None;
        self.lsp_pending_changes.clear();
    }

    /// Runs `f` with the attached LSP client and document, if both are
    /// present. Returns `None` (without calling `f`) if either is absent.
    ///
    /// `f` receives only `client`/`document`, not `&mut self`, so callers
    /// needing other `self` state (e.g. `self.buffer`) must compute it
    /// before calling this helper and move it into the closure.
    fn with_lsp<R>(
        &mut self,
        f: impl FnOnce(&mut dyn lsp::LspClient, &lsp::LspDocument) -> R,
    ) -> Option<R> {
        let client = self.lsp_client.as_deref_mut()?;
        let document = self.lsp_document.as_ref()?;
        Some(f(client, document))
    }

    /// Like [`Self::with_lsp`], but gives `f` a mutable reference to the
    /// document (needed to bump `document.version`).
    fn with_lsp_mut_document<R>(
        &mut self,
        f: impl FnOnce(&mut dyn lsp::LspClient, &mut lsp::LspDocument) -> R,
    ) -> Option<R> {
        let client = self.lsp_client.as_deref_mut()?;
        let document = self.lsp_document.as_mut()?;
        Some(f(client, document))
    }

    /// Detaches the current LSP client and closes any open document.
    ///
    /// This clears all LSP-related state on the editor instance, including any
    /// changes queued but not yet flushed. Safe to call when nothing is
    /// attached.
    ///
    /// # Example
    ///
    /// ```
    /// use std::cell::RefCell;
    /// use std::rc::Rc;
    ///
    /// use iced_code_editor::{CodeEditor, LspClient, LspDocument};
    ///
    /// /// Records whether the document was closed.
    /// struct ClosingClient(Rc<RefCell<bool>>);
    ///
    /// impl LspClient for ClosingClient {
    ///     fn did_close(&mut self, _document: &LspDocument) {
    ///         *self.0.borrow_mut() = true;
    ///     }
    /// }
    ///
    /// let closed = Rc::new(RefCell::new(false));
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.attach_lsp(
    ///     Box::new(ClosingClient(Rc::clone(&closed))),
    ///     LspDocument::new("file:///tmp/main.rs", "rust"),
    /// );
    ///
    /// editor.detach_lsp();
    /// assert!(*closed.borrow());
    ///
    /// // Detaching again is harmless.
    /// editor.detach_lsp();
    /// ```
    pub fn detach_lsp(&mut self) {
        self.with_lsp(|client, document| client.did_close(document));
        self.lsp_client = None;
        self.lsp_document = None;
        self.lsp_shadow_text = String::new();
        self.lsp_shadow_is_current = true;
        self.lsp_synced_line_count = 1;
        self.lsp_synced_last_line_len = 0;
        self.lsp_edit_snapshot = None;
        self.lsp_pending_changes.clear();
    }

    /// Sends a `did_save` notification with the current buffer contents.
    ///
    /// Call this after the host application has written the file to disk, so
    /// servers that re-run diagnostics or formatting on save see the new state.
    /// Does nothing when no client and document are attached.
    ///
    /// # Example
    ///
    /// ```
    /// use std::cell::RefCell;
    /// use std::rc::Rc;
    ///
    /// use iced_code_editor::{CodeEditor, LspClient, LspDocument};
    ///
    /// /// Records the text sent on save.
    /// struct SavingClient(Rc<RefCell<Option<String>>>);
    ///
    /// impl LspClient for SavingClient {
    ///     fn did_save(&mut self, _document: &LspDocument, text: &str) {
    ///         *self.0.borrow_mut() = Some(text.to_string());
    ///     }
    /// }
    ///
    /// let saved = Rc::new(RefCell::new(None));
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.attach_lsp(
    ///     Box::new(SavingClient(Rc::clone(&saved))),
    ///     LspDocument::new("file:///tmp/main.rs", "rust"),
    /// );
    ///
    /// editor.lsp_did_save();
    /// assert_eq!(saved.borrow().as_deref(), Some("fn main() {}"));
    /// ```
    pub fn lsp_did_save(&mut self) {
        // Skip serializing the whole buffer when no client/document is
        // attached: `with_lsp` would just discard the closure unused, but the
        // full-document `to_string()` allocation would still happen on every
        // save for hosts that never enable LSP.
        if self.lsp_client.is_none() || self.lsp_document.is_none() {
            return;
        }
        let text = self.buffer.to_string();
        self.with_lsp(|client, document| client.did_save(document, &text));
    }

    /// Requests hover information at the current cursor position.
    ///
    /// The request is fire-and-forget: the reply reaches the host through
    /// whatever channel the [`LspClient`] implementation uses, not through a
    /// return value. Does nothing when no client is attached.
    ///
    /// [`LspClient`]: crate::LspClient
    ///
    /// # Example
    ///
    /// ```
    /// use std::cell::RefCell;
    /// use std::rc::Rc;
    ///
    /// use iced_code_editor::{CodeEditor, LspClient, LspDocument, LspPosition};
    ///
    /// /// Records the position each hover request asked about.
    /// struct HoverClient(Rc<RefCell<Vec<LspPosition>>>);
    ///
    /// impl LspClient for HoverClient {
    ///     fn request_hover(&mut self, _document: &LspDocument, position: LspPosition) {
    ///         self.0.borrow_mut().push(position);
    ///     }
    /// }
    ///
    /// let hovers = Rc::new(RefCell::new(Vec::new()));
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.attach_lsp(
    ///     Box::new(HoverClient(Rc::clone(&hovers))),
    ///     LspDocument::new("file:///tmp/main.rs", "rust"),
    /// );
    ///
    /// editor.lsp_request_hover();
    /// // The cursor starts at the top of the document.
    /// assert_eq!(hovers.borrow().as_slice(), [LspPosition { line: 0, character: 0 }]);
    /// ```
    pub fn lsp_request_hover(&mut self) {
        let position = self.lsp_position_from_cursor();
        self.with_lsp(|client, document| {
            client.request_hover(document, position);
        });
    }

    /// Requests hover information at a canvas point.
    ///
    /// Use this to drive hover-on-mouse-move; [`Self::lsp_position_at_point`]
    /// resolves the point first, so a point over the gutter sends nothing.
    ///
    /// # Arguments
    ///
    /// * `point` - The position in canvas coordinates
    ///
    /// # Returns
    ///
    /// `true` if the point maps to a valid buffer position and the request was
    /// sent; `false` if it does not, or if no client is attached
    ///
    /// # Example
    ///
    /// ```
    /// use iced::Point;
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    ///
    /// // With no client attached there is nothing to send.
    /// assert!(!editor.lsp_request_hover_at(Point::new(50.0, 5.0)));
    /// ```
    pub fn lsp_request_hover_at(&mut self, point: iced::Point) -> bool {
        let Some(position) = self.lsp_position_from_point(point) else {
            return false;
        };
        self.with_lsp(|client, document| {
            client.request_hover(document, position);
        })
        .is_some()
    }

    /// Requests hover information at an explicit LSP position.
    ///
    /// Unlike [`Self::lsp_request_hover_at`], this skips point-to-position
    /// resolution — pair it with a position previously obtained from
    /// [`Self::lsp_hover_anchor_at_point`] so a hover that lingers stays
    /// anchored to the word rather than following the mouse.
    ///
    /// # Arguments
    ///
    /// * `position` - The zero-based document position to query
    ///
    /// # Returns
    ///
    /// `true` if a client is attached and the request was sent, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, LspPosition};
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// let position = LspPosition { line: 0, character: 3 };
    ///
    /// // With no client attached there is nothing to send.
    /// assert!(!editor.lsp_request_hover_at_position(position));
    /// ```
    pub fn lsp_request_hover_at_position(
        &mut self,
        position: lsp::LspPosition,
    ) -> bool {
        self.with_lsp(|client, document| {
            client.request_hover(document, position);
        })
        .is_some()
    }

    /// Converts a canvas point to an LSP position, if possible.
    ///
    /// Requires no attached client — this is pure hit-testing against the
    /// editor's current layout.
    ///
    /// # Arguments
    ///
    /// * `point` - The position in canvas coordinates
    ///
    /// # Returns
    ///
    /// `Some(position)` when the point falls on text, `None` when it lands in
    /// the gutter
    ///
    /// # Example
    ///
    /// ```
    /// use iced::Point;
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    ///
    /// // The origin sits in the line-number gutter, which maps to nothing.
    /// assert!(editor.lsp_position_at_point(Point::ORIGIN).is_none());
    ///
    /// // A point past the end of the document clamps to its last position.
    /// let position = editor
    ///     .lsp_position_at_point(Point::new(5_000.0, 5_000.0))
    ///     .expect("a point past the text clamps rather than failing");
    /// assert_eq!(position.line, 0);
    /// assert_eq!(position.character, 12);
    /// ```
    pub fn lsp_position_at_point(
        &self,
        point: iced::Point,
    ) -> Option<lsp::LspPosition> {
        self.lsp_position_from_point(point)
    }

    /// Returns the hover anchor position and its canvas point for a given
    /// cursor location.
    ///
    /// The anchor is the start of the word under the cursor. Anchoring to the
    /// word start rather than the exact mouse position keeps a hover popup from
    /// jittering as the pointer moves within one identifier, and gives the host
    /// a stable canvas point to position the popup against.
    ///
    /// # Arguments
    ///
    /// * `point` - The position in canvas coordinates
    ///
    /// # Returns
    ///
    /// `Some((position, anchor_point))` when the point falls on text, `None`
    /// when it lands in the gutter
    ///
    /// # Example
    ///
    /// ```
    /// use iced::Point;
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    ///
    /// // A gutter point has no word to anchor to.
    /// assert!(editor.lsp_hover_anchor_at_point(Point::ORIGIN).is_none());
    /// ```
    pub fn lsp_hover_anchor_at_point(
        &self,
        point: iced::Point,
    ) -> Option<(lsp::LspPosition, iced::Point)> {
        let (line, col) = self.calculate_cursor_from_point(point)?;
        let line_content = self.buffer.line(line);
        let anchor_col = Self::word_start_in_line(line_content, col);
        let anchor_point =
            self.point_from_position(line, anchor_col).unwrap_or(point);
        let line = u32::try_from(line).unwrap_or(u32::MAX);
        let character = u32::try_from(anchor_col).unwrap_or(u32::MAX);
        Some((lsp::LspPosition { line, character }, anchor_point))
    }

    /// Requests completion items at the current cursor position.
    ///
    /// Like the hover requests, this is fire-and-forget; the items arrive
    /// through the [`LspClient`] implementation. Does nothing when no client is
    /// attached.
    ///
    /// [`LspClient`]: crate::LspClient
    ///
    /// # Example
    ///
    /// ```
    /// use std::cell::RefCell;
    /// use std::rc::Rc;
    ///
    /// use iced_code_editor::{CodeEditor, LspClient, LspDocument, LspPosition};
    ///
    /// /// Counts completion requests.
    /// struct CompletionClient(Rc<RefCell<usize>>);
    ///
    /// impl LspClient for CompletionClient {
    ///     fn request_completion(&mut self, _document: &LspDocument, _position: LspPosition) {
    ///         *self.0.borrow_mut() += 1;
    ///     }
    /// }
    ///
    /// let requests = Rc::new(RefCell::new(0));
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.attach_lsp(
    ///     Box::new(CompletionClient(Rc::clone(&requests))),
    ///     LspDocument::new("file:///tmp/main.rs", "rust"),
    /// );
    ///
    /// editor.lsp_request_completion();
    /// assert_eq!(*requests.borrow(), 1);
    /// ```
    pub fn lsp_request_completion(&mut self) {
        let position = self.lsp_position_from_cursor();
        self.with_lsp(|client, document| {
            client.request_completion(document, position);
        });
    }

    /// Flushes pending LSP text changes to the attached client.
    ///
    /// This increments the document version and sends `did_change` with all
    /// queued changes. Changes stay queued while no client is attached, so a
    /// client attached later still receives a consistent document.
    ///
    /// Only needed when automatic flushing is off; see
    /// [`Self::set_lsp_auto_flush`].
    ///
    /// # Example
    ///
    /// ```
    /// use std::cell::RefCell;
    /// use std::rc::Rc;
    ///
    /// use iced_code_editor::{CodeEditor, LspClient, LspDocument, LspTextChange};
    ///
    /// /// Records how many changes each batch carried.
    /// struct BatchClient(Rc<RefCell<Vec<usize>>>);
    ///
    /// impl LspClient for BatchClient {
    ///     fn did_change(&mut self, _document: &LspDocument, changes: &[LspTextChange]) {
    ///         self.0.borrow_mut().push(changes.len());
    ///     }
    /// }
    ///
    /// let batches = Rc::new(RefCell::new(Vec::new()));
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    ///
    /// // Take over flushing, then drive it from a debounce timer.
    /// editor.set_lsp_auto_flush(false);
    /// editor.attach_lsp(
    ///     Box::new(BatchClient(Rc::clone(&batches))),
    ///     LspDocument::new("file:///tmp/main.rs", "rust"),
    /// );
    ///
    /// // Flushing an empty queue is a no-op, so a timer can call this
    /// // unconditionally without sending empty notifications.
    /// editor.lsp_flush_pending_changes();
    /// assert!(batches.borrow().is_empty());
    /// ```
    pub fn lsp_flush_pending_changes(&mut self) {
        if self.lsp_pending_changes.is_empty() {
            return;
        }
        // Only drain the queue once a client and document are confirmed
        // attached — otherwise the changes must stay queued for a future
        // `attach_lsp`/`lsp_open_document` flush.
        if self.lsp_client.is_none() || self.lsp_document.is_none() {
            return;
        }

        let changes = std::mem::take(&mut self.lsp_pending_changes);
        self.with_lsp_mut_document(|client, document| {
            document.version = document.version.saturating_add(1);
            client.did_change(document, &changes);
        });
    }

    /// Sets whether LSP changes are flushed automatically after edits.
    ///
    /// On by default, which is what most hosts want. Turn it off to batch
    /// rapid typing behind a debounce timer and call
    /// [`Self::lsp_flush_pending_changes`] yourself, so a chatty server isn't
    /// sent one notification per keystroke.
    ///
    /// # Arguments
    ///
    /// * `auto_flush` - `true` to flush after every edit, `false` to flush manually
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    ///
    /// // Take over flushing so edits can be debounced by the host.
    /// editor.set_lsp_auto_flush(false);
    /// ```
    pub fn set_lsp_auto_flush(&mut self, auto_flush: bool) {
        self.lsp_auto_flush = auto_flush;
    }

    /// Converts the current cursor position into an LSP position.
    fn lsp_position_from_cursor(&self) -> lsp::LspPosition {
        let pos = self.cursors.primary_position();
        let line = u32::try_from(pos.0).unwrap_or(u32::MAX);
        let character = u32::try_from(pos.1).unwrap_or(u32::MAX);
        lsp::LspPosition { line, character }
    }

    /// Converts a canvas point into an LSP position, if it hits the buffer.
    fn lsp_position_from_point(
        &self,
        point: iced::Point,
    ) -> Option<lsp::LspPosition> {
        let (line, col) = self.calculate_cursor_from_point(point)?;
        let line = u32::try_from(line).unwrap_or(u32::MAX);
        let character = u32::try_from(col).unwrap_or(u32::MAX);
        Some(lsp::LspPosition { line, character })
    }

    /// Computes and queues the latest LSP text change for the buffer.
    ///
    /// When auto-flush is enabled, this immediately sends changes.
    pub(crate) fn enqueue_lsp_change(&mut self) {
        if self.lsp_document.is_none() {
            return;
        }

        let new_text = self.buffer.to_string();
        let change = if self.lsp_shadow_is_current {
            lsp::compute_text_change(&self.lsp_shadow_text, &new_text)
        } else {
            let end_line = self.lsp_synced_line_count.saturating_sub(1);
            Some(lsp::LspTextChange {
                range: lsp::LspRange {
                    start: lsp::LspPosition { line: 0, character: 0 },
                    end: lsp::LspPosition {
                        line: u32::try_from(end_line).unwrap_or(u32::MAX),
                        character: u32::try_from(self.lsp_synced_last_line_len)
                            .unwrap_or(u32::MAX),
                    },
                },
                text: new_text.clone(),
            })
        };
        if let Some(change) = change {
            self.lsp_pending_changes.push(change);
        }
        self.lsp_shadow_text = new_text;
        self.lsp_shadow_is_current = true;
        self.update_lsp_synced_extent();
        if self.lsp_auto_flush {
            self.lsp_flush_pending_changes();
        }
    }

    /// Queues the bounded range replacement captured before a normal editor
    /// command. Unlike `enqueue_lsp_change`, this never serializes or diffs the
    /// complete document.
    pub(crate) fn enqueue_incremental_lsp_change(&mut self) {
        if self.lsp_document.is_none() {
            self.lsp_edit_snapshot = None;
            return;
        }

        let Some(snapshot) = self.lsp_edit_snapshot.take() else {
            self.enqueue_lsp_change();
            return;
        };

        let new_line_count = self.buffer.line_count();
        let start_line =
            snapshot.start_line.min(new_line_count.saturating_sub(1));
        let new_end_exclusive = if new_line_count >= snapshot.old_line_count {
            snapshot
                .old_end_exclusive
                .saturating_add(new_line_count - snapshot.old_line_count)
                .min(new_line_count)
        } else {
            snapshot
                .old_end_exclusive
                .saturating_sub(snapshot.old_line_count - new_line_count)
                .max(start_line.saturating_add(1))
                .min(new_line_count)
        };
        let text =
            self.buffer.line_range_to_string(start_line, new_end_exclusive);
        self.lsp_pending_changes.push(lsp::LspTextChange {
            range: lsp::LspRange {
                start: lsp::LspPosition {
                    line: u32::try_from(snapshot.start_line)
                        .unwrap_or(u32::MAX),
                    character: 0,
                },
                end: snapshot.old_end,
            },
            text,
        });

        // The shadow string is intentionally not rewritten here: doing so
        // would reintroduce an O(document size) copy. The compact extent below
        // is sufficient for a rare future full-document fallback.
        self.lsp_shadow_text = String::new();
        self.lsp_shadow_is_current = false;
        self.update_lsp_synced_extent();
        if self.lsp_auto_flush {
            self.lsp_flush_pending_changes();
        }
    }

    /// Updates the compact extent of the document state represented by queued
    /// and already-flushed LSP changes.
    fn update_lsp_synced_extent(&mut self) {
        self.lsp_synced_line_count = self.buffer.line_count();
        self.lsp_synced_last_line_len =
            self.buffer.line_len(self.lsp_synced_line_count.saturating_sub(1));
    }

    /// Sets whether LSP support is enabled.
    ///
    /// When set to `false`, any attached LSP client is detached automatically.
    /// Calling [`attach_lsp`] while disabled is a no-op.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_lsp_enabled(false);
    /// ```
    ///
    /// [`attach_lsp`]: CodeEditor::attach_lsp
    pub fn set_lsp_enabled(&mut self, enabled: bool) {
        self.lsp_enabled = enabled;
        if !enabled {
            self.detach_lsp();
        }
    }

    /// Returns whether LSP support is enabled.
    ///
    /// # Returns
    ///
    /// `true` if LSP is enabled, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// // Enabled by default, though no client is attached until you attach one.
    /// assert!(editor.lsp_enabled());
    ///
    /// editor.set_lsp_enabled(false);
    /// assert!(!editor.lsp_enabled());
    /// ```
    pub fn lsp_enabled(&self) -> bool {
        self.lsp_enabled
    }

    /// Initiates a "Go to Definition" request for the symbol at the current cursor position.
    ///
    /// This method converts the current cursor coordinates into an LSP-compatible position
    /// and delegates the request to the active `LspClient`, if one is attached.
    ///
    /// Fire-and-forget: the resolved location arrives through the client
    /// implementation, and the host decides whether to follow it.
    ///
    /// # Example
    ///
    /// ```
    /// use std::cell::RefCell;
    /// use std::rc::Rc;
    ///
    /// use iced_code_editor::{CodeEditor, LspClient, LspDocument, LspPosition};
    ///
    /// /// Records the position each definition request asked about.
    /// struct DefinitionClient(Rc<RefCell<Vec<LspPosition>>>);
    ///
    /// impl LspClient for DefinitionClient {
    ///     fn request_definition(&mut self, _document: &LspDocument, position: LspPosition) {
    ///         self.0.borrow_mut().push(position);
    ///     }
    /// }
    ///
    /// let requests = Rc::new(RefCell::new(Vec::new()));
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.attach_lsp(
    ///     Box::new(DefinitionClient(Rc::clone(&requests))),
    ///     LspDocument::new("file:///tmp/main.rs", "rust"),
    /// );
    ///
    /// editor.lsp_request_definition();
    /// assert_eq!(requests.borrow().len(), 1);
    /// ```
    pub fn lsp_request_definition(&mut self) {
        let position = self.lsp_position_from_cursor();
        self.with_lsp(|client, document| {
            client.request_definition(document, position);
        });
    }

    /// Initiates a "Go to Definition" request for the symbol at the specified screen coordinates.
    ///
    /// This is typically used for mouse interactions (e.g., Ctrl+Click). It first resolves
    /// the screen coordinates to a text position and then sends the request.
    ///
    /// # Arguments
    ///
    /// * `point` - The click position in canvas coordinates
    ///
    /// # Returns
    ///
    /// `true` if the request was successfully sent (i.e., a valid position was found and an LSP client is active),
    /// `false` otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// use iced::Point;
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    ///
    /// // With no client attached there is nothing to send.
    /// assert!(!editor.lsp_request_definition_at(Point::new(50.0, 5.0)));
    /// ```
    pub fn lsp_request_definition_at(&mut self, point: iced::Point) -> bool {
        let Some(position) = self.lsp_position_from_point(point) else {
            return false;
        };
        self.with_lsp(|client, document| {
            client.request_definition(document, position);
        })
        .is_some()
    }

    /// Requests formatting edits for the whole document.
    ///
    /// The formatting options are derived from the editor's own
    /// [`IndentStyle`], so the server formats the way this editor indents.
    /// Queued changes are flushed first: the server formats the document it
    /// has, so an unflushed keystroke would come back reformatted away.
    ///
    /// Like every other request this is fire-and-forget. The reply is a batch
    /// of edits, which the host applies with
    /// [`Self::apply_lsp_text_edits`].
    ///
    /// # Returns
    ///
    /// `true` if the request was sent; `false` when no client and document are
    /// attached
    ///
    /// [`IndentStyle`]: crate::IndentStyle
    ///
    /// # Example
    ///
    /// ```
    /// use std::cell::RefCell;
    /// use std::rc::Rc;
    ///
    /// use iced_code_editor::{
    ///     CodeEditor, IndentStyle, LspClient, LspDocument, LspFormattingOptions,
    /// };
    ///
    /// /// Records the options each formatting request carried.
    /// struct FormattingClient(Rc<RefCell<Vec<LspFormattingOptions>>>);
    ///
    /// impl LspClient for FormattingClient {
    ///     fn request_formatting(
    ///         &mut self,
    ///         _document: &LspDocument,
    ///         options: LspFormattingOptions,
    ///     ) {
    ///         self.0.borrow_mut().push(options);
    ///     }
    /// }
    ///
    /// let requests = Rc::new(RefCell::new(Vec::new()));
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    ///
    /// // Nothing is attached yet, so there is nobody to ask.
    /// assert!(!editor.lsp_request_formatting());
    ///
    /// editor.set_indent_style(IndentStyle::Spaces(2));
    /// editor.attach_lsp(
    ///     Box::new(FormattingClient(Rc::clone(&requests))),
    ///     LspDocument::new("file:///tmp/main.rs", "rust"),
    /// );
    ///
    /// assert!(editor.lsp_request_formatting());
    /// assert_eq!(requests.borrow()[0].tab_size, 2);
    /// ```
    pub fn lsp_request_formatting(&mut self) -> bool {
        self.lsp_flush_pending_changes();
        let options = lsp::LspFormattingOptions::from(self.indent_style());
        self.with_lsp(|client, document| {
            client.request_formatting(document, options);
        })
        .is_some()
    }
}

/// Sends `did_open` for `document` (after stamping `version = 1`) on
/// `client`, using `buffer`'s current contents.
///
/// Returns the stamped document and the serialized buffer text, both needed
/// by the caller to update its LSP shadow-sync bookkeeping.
///
/// Free function (not a method) so callers can hold a mutable borrow of
/// `self.lsp_client` while still passing `&self.buffer` — a method taking
/// `&mut self` here would conflict with that borrow in `lsp_open_document`.
fn open_lsp_document(
    client: &mut dyn lsp::LspClient,
    buffer: &TextBuffer,
    mut document: lsp::LspDocument,
) -> (lsp::LspDocument, String) {
    document.version = 1;
    let text = buffer.to_string();
    client.did_open(&document, &text);
    (document, text)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::canvas_editor::Message;

    use super::*;

    #[derive(Default)]
    struct TestLspClient {
        changes: Rc<RefCell<Vec<Vec<lsp::LspTextChange>>>>,
    }

    impl lsp::LspClient for TestLspClient {
        fn did_change(
            &mut self,
            _document: &lsp::LspDocument,
            changes: &[lsp::LspTextChange],
        ) {
            self.changes.borrow_mut().push(changes.to_vec());
        }
    }

    /// Records every [`lsp::LspClient`] method invoked on it, by name.
    #[derive(Default)]
    struct RecordingLspClient {
        calls: Rc<RefCell<Vec<String>>>,
    }

    impl lsp::LspClient for RecordingLspClient {
        fn did_open(&mut self, _document: &lsp::LspDocument, _text: &str) {
            self.calls.borrow_mut().push("did_open".to_string());
        }
        fn did_close(&mut self, _document: &lsp::LspDocument) {
            self.calls.borrow_mut().push("did_close".to_string());
        }
        fn did_save(&mut self, _document: &lsp::LspDocument, _text: &str) {
            self.calls.borrow_mut().push("did_save".to_string());
        }
        fn request_hover(
            &mut self,
            _document: &lsp::LspDocument,
            _position: lsp::LspPosition,
        ) {
            self.calls.borrow_mut().push("request_hover".to_string());
        }
        fn did_change(
            &mut self,
            _document: &lsp::LspDocument,
            _changes: &[lsp::LspTextChange],
        ) {
            self.calls.borrow_mut().push("did_change".to_string());
        }
        fn request_formatting(
            &mut self,
            _document: &lsp::LspDocument,
            _options: lsp::LspFormattingOptions,
        ) {
            self.calls.borrow_mut().push("request_formatting".to_string());
        }
    }

    #[test]
    fn test_enqueue_lsp_change_auto_flush() {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let client = TestLspClient { changes: Rc::clone(&changes) };
        let mut editor = CodeEditor::new("hello", "rs");
        editor.attach_lsp(
            Box::new(client),
            lsp::LspDocument::new("file:///test.rs", "rust"),
        );
        editor.set_lsp_auto_flush(true);

        editor.buffer.insert_char(0, 5, '!');
        editor.enqueue_lsp_change();

        let changes = changes.borrow();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].len(), 1);
        let change = &changes[0][0];
        assert_eq!(change.text, "!");
        assert_eq!(change.range.start.line, 0);
        assert_eq!(change.range.start.character, 5);
        assert_eq!(change.range.end.line, 0);
        assert_eq!(change.range.end.character, 5);
    }

    #[test]
    fn test_editor_update_sends_bounded_incremental_lsp_change() {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let client = TestLspClient { changes: Rc::clone(&changes) };
        let content = (0..10)
            .map(|line| format!("line{line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut editor = CodeEditor::new(&content, "rs");
        editor.attach_lsp(
            Box::new(client),
            lsp::LspDocument::new("file:///large.rs", "rust"),
        );
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;
        editor.cursors.primary_mut().position = (5, 2);

        let _ = editor.update(&Message::CharacterInput('X'));

        let changes = changes.borrow();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].len(), 1);
        let change = &changes[0][0];
        assert_eq!(change.range.start.line, 4);
        assert_eq!(change.range.start.character, 0);
        assert_eq!(change.range.end.line, 7);
        assert_eq!(change.range.end.character, 0);
        assert_eq!(change.text, "line4\nliXne5\nline6\n");
        assert!(!editor.lsp_shadow_is_current);
        assert!(editor.lsp_shadow_text.is_empty());
    }

    #[test]
    fn test_open_lsp_document_stamps_version_and_sends_did_open() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut client = RecordingLspClient { calls: Rc::clone(&calls) };
        let buffer = TextBuffer::new("hello");
        let document = lsp::LspDocument::new("file:///test.rs", "rust");

        let (document, text) =
            open_lsp_document(&mut client, &buffer, document);

        assert_eq!(document.version, 1);
        assert_eq!(text, "hello");
        assert_eq!(calls.borrow().as_slice(), ["did_open"]);
    }

    #[test]
    fn test_with_lsp_runs_closure_when_client_and_document_present() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let client = RecordingLspClient { calls: Rc::clone(&calls) };
        let mut editor = CodeEditor::new("hello", "rs");
        editor.attach_lsp(
            Box::new(client),
            lsp::LspDocument::new("file:///test.rs", "rust"),
        );
        calls.borrow_mut().clear(); // attach_lsp itself already logged did_open

        let ran = editor.with_lsp(|client, document| {
            client.did_save(document, "hello");
        });

        assert!(ran.is_some());
        assert_eq!(calls.borrow().as_slice(), ["did_save"]);
    }

    #[test]
    fn test_with_lsp_returns_none_when_client_absent() {
        let mut editor = CodeEditor::new("hello", "rs");
        let ran = editor.with_lsp(|_client, _document| {});
        assert!(ran.is_none());
    }

    #[test]
    fn test_lsp_did_save_sends_current_buffer_contents() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let client = RecordingLspClient { calls: Rc::clone(&calls) };
        let mut editor = CodeEditor::new("hello", "rs");
        editor.attach_lsp(
            Box::new(client),
            lsp::LspDocument::new("file:///test.rs", "rust"),
        );
        calls.borrow_mut().clear(); // attach_lsp itself already logged did_open

        editor.lsp_did_save();

        assert_eq!(calls.borrow().as_slice(), ["did_save"]);
    }

    #[test]
    fn test_lsp_did_save_is_a_noop_without_an_attached_client() {
        // Regression: this used to serialize the whole buffer via
        // `to_string()` even with no client/document attached.
        let mut editor = CodeEditor::new("hello", "rs");
        editor.lsp_did_save(); // must not panic
    }

    #[test]
    fn test_with_lsp_returns_none_when_document_absent() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let client = RecordingLspClient { calls: Rc::clone(&calls) };
        let mut editor = CodeEditor::new("hello", "rs");
        editor.attach_lsp(
            Box::new(client),
            lsp::LspDocument::new("file:///test.rs", "rust"),
        );
        editor.lsp_document = None;

        let ran = editor.with_lsp(|_client, _document| {});
        assert!(ran.is_none());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_with_lsp_mut_document_allows_version_bump() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let client = RecordingLspClient { calls: Rc::clone(&calls) };
        let mut editor = CodeEditor::new("hello", "rs");
        editor.attach_lsp(
            Box::new(client),
            lsp::LspDocument::new("file:///test.rs", "rust"),
        );

        editor.with_lsp_mut_document(|_client, document| {
            document.version = document.version.saturating_add(1);
        });

        assert_eq!(editor.lsp_document.as_ref().unwrap().version, 2);
    }

    #[test]
    fn test_lsp_flush_pending_changes_preserves_queue_when_no_client_attached()
    {
        let mut editor = CodeEditor::new("hello", "rs");
        editor.lsp_pending_changes.push(lsp::LspTextChange {
            range: lsp::LspRange {
                start: lsp::LspPosition { line: 0, character: 0 },
                end: lsp::LspPosition { line: 0, character: 0 },
            },
            text: "x".to_string(),
        });

        editor.lsp_flush_pending_changes();

        assert_eq!(editor.lsp_pending_changes.len(), 1);
    }

    #[test]
    fn test_lsp_request_formatting_flushes_queued_changes_first() {
        // The server formats the document it holds, so a keystroke still
        // sitting in the queue has to reach it before the request does.
        let calls = Rc::new(RefCell::new(Vec::new()));
        let client = RecordingLspClient { calls: Rc::clone(&calls) };
        let mut editor = CodeEditor::new("hello", "rs");
        editor.set_lsp_auto_flush(false);
        editor.attach_lsp(
            Box::new(client),
            lsp::LspDocument::new("file:///test.rs", "rust"),
        );

        editor.buffer.insert_char(0, 5, '!');
        editor.enqueue_lsp_change();
        assert!(editor.lsp_request_formatting());

        assert_eq!(
            calls.borrow().as_slice(),
            ["did_open", "did_change", "request_formatting"]
        );
    }

    #[test]
    fn test_lsp_request_formatting_is_a_noop_without_an_attached_client() {
        let mut editor = CodeEditor::new("hello", "rs");
        assert!(!editor.lsp_request_formatting());
    }
}
