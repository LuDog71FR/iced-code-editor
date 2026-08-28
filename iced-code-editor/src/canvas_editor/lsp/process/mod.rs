//! LSP (Language Server Protocol) Process Client implementation.
//!
//! This module provides a client for communicating with LSP servers via stdio.
//! It handles document synchronization, hover requests, and completion requests.
//!
//! Enable with the `lsp-process` Cargo feature. Not available on WASM targets.
//!
//! The transport itself lives in [`protocol`]: framing, bounded reads, message
//! dispatch, and response parsing. [`text_model`] holds the per-document mirror
//! used for UTF-16 position translation, and [`pending`] tracks in-flight
//! requests. What remains here is the process lifecycle and the [`LspClient`]
//! implementation the editor talks to.

pub mod config;
pub mod overlay;

mod pending;
mod protocol;
mod text_model;

use self::config::{
    LspCommand, ensure_rust_analyzer_config, lsp_server_config,
    resolve_lsp_command,
};
use self::pending::{LspRequestKind, PendingRequest, evict_expired_requests};
use self::protocol::{
    frame_message, handle_client_response, handle_server_notification,
    handle_server_request, read_log_line, read_message,
};
use self::text_model::{DocumentState, TextModel, apply_changes_to_document};
use crate::canvas_editor::lsp::{
    LspClient, LspDocument, LspPosition, LspRange, LspTextChange,
};
use serde_json::json;
use std::collections::HashMap;
use std::io::{BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Instant;
// =============================================================================
// LSP Events - Events sent back to the main application
// =============================================================================

/// Recommended capacity for the [`LspEvent`] channel.
///
/// The channel has to be bounded: it is fed by a language server this
/// application does not control, and `rust-analyzer` on a large workspace
/// emits a great deal. An unbounded queue turns that into unbounded memory
/// whenever the host drains more slowly than the server produces.
///
/// Sized well above what a host draining on a timer can fall behind by, so the
/// bound never bites in normal use -- draining
/// `MAX_LSP_EVENTS_PER_TICK`-sized batches at 60 Hz clears far more than this
/// per second. Its job is to put a ceiling on a pathological server, not to
/// shape ordinary traffic.
pub const LSP_EVENT_QUEUE_CAPACITY: usize = 4_096;

/// Hands an event to the application, dropping it if the queue is full.
///
/// Dropping rather than blocking, because the sender is the reader thread: it
/// is what drains the server's stdout, so blocking it would stall that pipe
/// and back-pressure the server for as long as the host took to catch up.
///
/// Dropping is safe for every [`LspEvent`] variant. None of them carries state
/// the client must not lose: `Hover`, `Completion` and `Definition` are each
/// one UI update that the user's next gesture re-requests, `Progress` is
/// superseded by the next notification for the same token, and `Log` is
/// advisory. In particular a dropped response strands nothing --
/// `handle_client_response` removes the entry from `pending_requests` *before*
/// emitting, so the request is already accounted for.
///
/// # Arguments
///
/// * `events` - Channel to the application
/// * `event` - The event to hand over
pub(super) fn emit(events: &mpsc::SyncSender<LspEvent>, event: LspEvent) {
    let _ = events.try_send(event);
}

/// Events that can be sent from the LSP client to the application.
///
/// Receive these by polling the `mpsc::Receiver` you pass to
/// [`LspProcessClient::new_with_server`]. Requests are fire-and-forget, so
/// every server reply arrives here rather than as a return value — drain the
/// receiver on a timer and fold the results into the application state.
///
/// # Examples
///
/// ```
/// use std::sync::mpsc;
///
/// use iced_code_editor::{LspEvent, LSP_EVENT_QUEUE_CAPACITY};
///
/// // Bounded: the queue is fed by a language server, so it needs a ceiling.
/// let (tx, rx) = mpsc::sync_channel::<LspEvent>(LSP_EVENT_QUEUE_CAPACITY);
/// tx.send(LspEvent::Hover { text: "fn main()".to_string() })
///     .expect("the receiver is still alive");
///
/// // Drain everything queued since the last tick.
/// while let Ok(event) = rx.try_recv() {
///     match event {
///         LspEvent::Hover { text } => assert_eq!(text, "fn main()"),
///         LspEvent::Completion { items } => drop(items),
///         LspEvent::Definition { uri, .. } => drop(uri),
///         LspEvent::Progress { done, .. } => drop(done),
///         LspEvent::Log { message, .. } => drop(message),
///     }
/// }
/// ```
pub enum LspEvent {
    /// Hover information received from the LSP server.
    Hover {
        /// Markdown or plain-text hover content.
        text: String,
    },
    /// Completion items received from the LSP server.
    Completion {
        /// List of completion label strings.
        items: Vec<String>,
    },
    /// Definition location received from the LSP server.
    Definition {
        /// Target document URI.
        uri: String,
        /// Target range within that document.
        range: crate::canvas_editor::lsp::LspRange,
    },
    /// Progress notification from the LSP server.
    Progress {
        /// Progress token identifier.
        token: String,
        /// Key of the server that sent this notification.
        server_key: String,
        /// Human-readable title for the progress operation.
        title: String,
        /// Optional status message.
        message: Option<String>,
        /// Optional percentage complete (0–100).
        percentage: Option<u32>,
        /// `true` when this is the final progress notification.
        done: bool,
    },
    /// Log message from the LSP server's stderr.
    Log {
        /// Key of the server that sent this message.
        server_key: String,
        /// The log line.
        message: String,
    },
}

// =============================================================================
// LSP Process Client - Main client implementation
// =============================================================================

/// Client for communicating with an LSP server process.
///
/// Manages the lifecycle of the server process and handles all communication.
/// Implements [`LspClient`] so it can be plugged directly into a [`CodeEditor`].
///
/// # Examples
///
/// ```no_run
/// use std::sync::mpsc;
/// use iced_code_editor::{LspProcessClient, LspEvent, LSP_EVENT_QUEUE_CAPACITY};
///
/// let (tx, rx) = mpsc::sync_channel::<LspEvent>(LSP_EVENT_QUEUE_CAPACITY);
/// let client = LspProcessClient::new_with_server(
///     "file:///home/user/project",
///     tx,
///     "lua-language-server",
/// );
/// ```
///
/// [`CodeEditor`]: crate::CodeEditor
pub struct LspProcessClient {
    /// The child process running the LSP server
    child: Child,
    /// Channel for sending messages to the writer thread
    writer: mpsc::Sender<Vec<u8>>,
    /// Map of URI to document state for all open documents
    documents: Arc<Mutex<HashMap<String, DocumentState>>>,
    /// Channel used to report a document mirror going out of sync (see
    /// [`Self::apply_change_and_convert`]); the reader/stderr threads carry
    /// their own clone of the same sender for server-pushed events.
    events: mpsc::SyncSender<LspEvent>,
    /// Key identifying the connected server, used to label events emitted
    /// directly by the client rather than by the reader/stderr threads.
    server_key: String,
    /// Counter for generating unique request IDs
    request_id: AtomicU64,
    /// Map of pending request IDs to their types and send time (for
    /// response routing and expiry of abandoned requests)
    pending_requests: Arc<Mutex<HashMap<u64, PendingRequest>>>,
    /// Handle to the writer thread (kept alive for the client's lifetime)
    _writer_thread: thread::JoinHandle<()>,
    /// Handle to the reader thread (kept alive for the client's lifetime)
    _reader_thread: thread::JoinHandle<()>,
    /// Handle to the stderr thread (kept alive for the client's lifetime)
    _stderr_thread: thread::JoinHandle<()>,
}

impl LspProcessClient {
    /// Creates a new LSP client connected to the specified server.
    ///
    /// # Arguments
    ///
    /// * `root_uri` — the root URI of the workspace (e.g. `"file:///home/user/project"`)
    /// * `events` — channel to send [`LspEvent`]s back to the application
    /// * `server_key` — key identifying the LSP server (e.g. `"lua-language-server"`)
    ///
    /// # Errors
    ///
    /// Returns an error string when the server key is not recognised, when the
    /// server binary cannot be found, or when the process cannot be spawned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::mpsc;
    /// use iced_code_editor::{
    ///     LSP_EVENT_QUEUE_CAPACITY, LspEvent, LspProcessClient,
    /// };
    ///
    /// let (tx, _rx) = mpsc::sync_channel::<LspEvent>(LSP_EVENT_QUEUE_CAPACITY);
    /// let client = LspProcessClient::new_with_server(
    ///     "file:///tmp/project",
    ///     tx,
    ///     "lua-language-server",
    /// );
    /// assert!(client.is_ok());
    /// ```
    pub fn new_with_server(
        root_uri: &str,
        events: mpsc::SyncSender<LspEvent>,
        server_key: &str,
    ) -> Result<Self, String> {
        let config = lsp_server_config(server_key)
            .ok_or_else(|| format!("Unsupported LSP server: {}", server_key))?;

        if server_key == "rust-analyzer" {
            ensure_rust_analyzer_config();
        }

        let command = resolve_lsp_command(config)?;
        Self::new_with_command(root_uri, events, &command, server_key)
    }

    /// Creates a new LSP client with a specific command.
    ///
    /// This is the internal implementation that spawns the process.
    ///
    /// # Errors
    ///
    /// Returns an error string if the process cannot be spawned or if stdio
    /// handles cannot be acquired.
    fn new_with_command(
        root_uri: &str,
        events: mpsc::SyncSender<LspEvent>,
        command: &LspCommand,
        server_key: &str,
    ) -> Result<Self, String> {
        let mut child = Command::new(&command.program)
            .args(&command.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    if command.program == "rust-analyzer" {
                        "LSP server program rust-analyzer not found. Please install rust-analyzer or set RUST_ANALYZER/RUST_ANALYZER_PATH environment variable".to_string()
                    } else {
                        format!("LSP server program {} not found", command.program)
                    }
                } else {
                    e.to_string()
                }
            })?;

        let stdin = child.stdin.take().ok_or("stdin unavailable")?;
        let stdout = child.stdout.take().ok_or("stdout unavailable")?;
        let stderr = child.stderr.take().ok_or("stderr unavailable")?;

        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let pending_requests = Arc::new(Mutex::new(HashMap::new()));
        let pending_reader = pending_requests.clone();
        let events_reader = events.clone();
        let events_log = events.clone();
        let events_field = events;
        let server_key = server_key.to_string();
        let server_key_reader = server_key.clone();
        let server_key_log = server_key.clone();
        let tx_reader = tx.clone();

        let writer_thread = thread::spawn(move || {
            let mut input = stdin;
            for bytes in rx {
                if input.write_all(&bytes).is_err() {
                    break;
                }
                let _ = input.flush();
            }
        });

        let reader_thread = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while let Some(buf) = read_message(&mut reader) {
                if let Ok(value) =
                    serde_json::from_slice::<serde_json::Value>(&buf)
                {
                    if let Some(id) = value.get("id").and_then(|v| v.as_u64()) {
                        if let Some(method) =
                            value.get("method").and_then(|m| m.as_str())
                        {
                            handle_server_request(id, method, &tx_reader);
                        } else {
                            handle_client_response(
                                id,
                                &value,
                                &pending_reader,
                                &events_reader,
                            );
                        }
                    } else if let Some(method) =
                        value.get("method").and_then(|m| m.as_str())
                        && let Some(params) = value.get("params")
                    {
                        handle_server_notification(
                            method,
                            params,
                            &events_reader,
                            &server_key_reader,
                        );
                    }
                }
            }
        });

        let stderr_thread = thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            while let Some(line) = read_log_line(&mut reader) {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                emit(
                    &events_log,
                    LspEvent::Log {
                        server_key: server_key_log.clone(),
                        message: line.to_string(),
                    },
                );
            }
        });

        let client = Self {
            child,
            writer: tx,
            documents: Arc::new(Mutex::new(HashMap::new())),
            events: events_field,
            server_key,
            request_id: AtomicU64::new(1),
            pending_requests,
            _writer_thread: writer_thread,
            _reader_thread: reader_thread,
            _stderr_thread: stderr_thread,
        };

        let initialize = json!({
            "jsonrpc": "2.0",
            "id": client.next_id(),
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "rootUri": root_uri,
                "capabilities": {
                    "textDocument": {
                        "synchronization": {
                            "dynamicRegistration": false,
                            "willSave": false,
                            "didSave": true
                        }
                    },
                    "window": {
                        "workDoneProgress": true
                    }
                },
                "workspaceFolders": null
            }
        });
        client.send_message(&initialize);

        let initialized = json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        });
        client.send_message(&initialized);

        Ok(client)
    }

    /// Generates the next unique request ID using atomic operations.
    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Sends a JSON-RPC message to the LSP server.
    ///
    /// Formats the message with the required `Content-Length` header.
    fn send_message(&self, value: &serde_json::Value) {
        if let Some(bytes) = frame_message(value) {
            let _ = self.writer.send(bytes);
        }
    }

    /// Applies text changes to a document and converts them to JSON format.
    ///
    /// Also converts positions to UTF-16 as required by LSP.
    ///
    /// If the local document mirror desynchronizes partway through `changes`
    /// (see [`apply_changes_to_document`]), the document is dropped from
    /// [`Self::documents`] — so a later `did_open` reseeds it instead of
    /// this client continuing to serve hover/completion/definition
    /// positions computed from a copy known to be stale — and an
    /// [`LspEvent::Log`] is emitted so the desync is diagnosable instead of
    /// surfacing only as "the language server gives nonsense answers".
    fn apply_change_and_convert(
        &self,
        uri: &str,
        changes: &[LspTextChange],
    ) -> Vec<serde_json::Value> {
        let mut docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
        let Some(state) = docs.get_mut(uri) else { return Vec::new() };

        match apply_changes_to_document(state, changes) {
            Some(out) => out,
            None => {
                docs.remove(uri);
                emit(
                    &self.events,
                    LspEvent::Log {
                        server_key: self.server_key.clone(),
                        message: format!(
                            "Local document mirror for {uri} desynchronized \
                             (a change referenced a line outside the tracked \
                             document); dropping it until the next open."
                        ),
                    },
                );
                Vec::new()
            }
        }
    }
}

// =============================================================================
// Reader thread helper functions
// =============================================================================

/// Sends shutdown/exit notifications and kills the process on drop.
impl Drop for LspProcessClient {
    fn drop(&mut self) {
        let shutdown = json!({
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": "shutdown",
            "params": null
        });
        self.send_message(&shutdown);

        let exit = json!({
            "jsonrpc": "2.0",
            "method": "exit",
            "params": {}
        });
        self.send_message(&exit);

        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
    }
}
// =============================================================================
// LspClient Trait Implementation
// =============================================================================

impl LspClient for LspProcessClient {
    fn did_open(&mut self, document: &LspDocument, text: &str) {
        let mut docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
        docs.insert(
            document.uri.clone(),
            DocumentState { text: TextModel::from_text(text) },
        );

        let msg = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": document.uri,
                    "languageId": document.language_id,
                    "version": document.version,
                    "text": text
                }
            }
        });
        self.send_message(&msg);
    }

    fn did_change(
        &mut self,
        document: &LspDocument,
        changes: &[LspTextChange],
    ) {
        let content_changes =
            self.apply_change_and_convert(&document.uri, changes);
        if content_changes.is_empty() {
            return;
        }

        let msg = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": {
                    "uri": document.uri,
                    "version": document.version
                },
                "contentChanges": content_changes
            }
        });
        self.send_message(&msg);
    }

    fn did_save(&mut self, document: &LspDocument, text: &str) {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didSave",
            "params": {
                "textDocument": { "uri": document.uri },
                "text": text
            }
        });
        self.send_message(&msg);
    }

    fn did_close(&mut self, document: &LspDocument) {
        let mut docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
        docs.remove(&document.uri);

        let msg = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": {
                "textDocument": { "uri": document.uri }
            }
        });
        self.send_message(&msg);
    }

    fn request_hover(&mut self, document: &LspDocument, position: LspPosition) {
        let docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
        let Some(state) = docs.get(&document.uri) else { return };
        let pos = state.text.to_utf16_position(position);

        let id = self.next_id();
        {
            let mut pending =
                self.pending_requests.lock().unwrap_or_else(|e| e.into_inner());
            evict_expired_requests(&mut pending);
            pending.insert(
                id,
                PendingRequest {
                    kind: LspRequestKind::Hover,
                    requested_at: Instant::now(),
                },
            );
        }

        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": document.uri },
                "position": { "line": pos.line, "character": pos.character }
            }
        });
        self.send_message(&msg);
    }

    fn request_completion(
        &mut self,
        document: &LspDocument,
        position: LspPosition,
    ) {
        let docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
        let Some(state) = docs.get(&document.uri) else { return };
        let pos = state.text.to_utf16_position(position);

        let id = self.next_id();
        {
            let mut pending =
                self.pending_requests.lock().unwrap_or_else(|e| e.into_inner());
            evict_expired_requests(&mut pending);
            pending.insert(
                id,
                PendingRequest {
                    kind: LspRequestKind::Completion,
                    requested_at: Instant::now(),
                },
            );
        }

        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": document.uri },
                "position": { "line": pos.line, "character": pos.character },
                "context": { "triggerKind": 1 }
            }
        });
        self.send_message(&msg);
    }

    fn request_definition(
        &mut self,
        document: &LspDocument,
        position: LspPosition,
    ) {
        let docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
        let Some(state) = docs.get(&document.uri) else { return };
        let pos = state.text.to_utf16_position(position);

        let id = self.next_id();
        {
            let mut pending =
                self.pending_requests.lock().unwrap_or_else(|e| e.into_inner());
            evict_expired_requests(&mut pending);
            pending.insert(
                id,
                PendingRequest {
                    kind: LspRequestKind::Definition,
                    requested_at: Instant::now(),
                },
            );
        }

        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": document.uri },
                "position": { "line": pos.line, "character": pos.character }
            }
        });
        self.send_message(&msg);
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    // Several helpers and tests below carry an `#[allow]` for `expect_used`,
    // `unwrap_used`, or `panic`. In test code a panic *is* the failure
    // report, so these mirror the existing per-test allows in `protocol.rs`.
    use super::*;
    use crate::canvas_editor::lsp::{LspPosition, LspRange, LspTextChange};

    /// Builds an `LspProcessClient` without a real LSP server: `child` still
    /// needs a genuine `Child`, so a process that exits immediately stands in
    /// for it, and the writer/reader/stderr threads are no-ops since these
    /// tests exercise the client's own methods directly rather than the wire
    /// protocol those background threads drive.
    #[allow(clippy::expect_used)]
    fn test_client()
    -> (LspProcessClient, mpsc::Receiver<Vec<u8>>, mpsc::Receiver<LspEvent>)
    {
        let child = Command::new("true").spawn().expect("spawn stub process");
        let (writer_tx, writer_rx) = mpsc::channel::<Vec<u8>>();
        let (events_tx, events_rx) = mpsc::sync_channel::<LspEvent>(16);
        let client = LspProcessClient {
            child,
            writer: writer_tx,
            documents: Arc::new(Mutex::new(HashMap::new())),
            events: events_tx,
            server_key: "test-server".to_string(),
            request_id: AtomicU64::new(1),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            _writer_thread: thread::spawn(|| {}),
            _reader_thread: thread::spawn(|| {}),
            _stderr_thread: thread::spawn(|| {}),
        };
        (client, writer_rx, events_rx)
    }

    /// Returns the JSON body of a `Content-Length`-framed message taken from
    /// the writer channel.
    #[allow(clippy::expect_used)]
    fn decode_sent(data: &[u8]) -> serde_json::Value {
        let header_end = data
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .expect("missing header separator");
        let body = &data[header_end + 4..];
        serde_json::from_slice(body).expect("invalid JSON body")
    }

    fn document(uri: &str) -> LspDocument {
        LspDocument {
            uri: uri.to_string(),
            language_id: "rust".to_string(),
            version: 1,
        }
    }

    fn range(sl: u32, sc: u32, el: u32, ec: u32) -> LspRange {
        LspRange {
            start: LspPosition { line: sl, character: sc },
            end: LspPosition { line: el, character: ec },
        }
    }

    // -------------------------------------------------------------------------
    // next_id / send_message
    // -------------------------------------------------------------------------

    #[test]
    fn test_emit_drops_events_once_the_queue_is_full() {
        // The point of the bound: past capacity the queue stops growing. Note
        // that a blocking `send` here would not fail this test, it would hang
        // it -- there is no receiver draining, which is exactly the situation
        // `emit` must never wait on, since the sender is the reader thread
        // feeding on the server's stdout.
        let (tx, rx) = mpsc::sync_channel::<LspEvent>(2);

        for index in 0..5 {
            emit(&tx, LspEvent::Hover { text: index.to_string() });
        }

        let queued: Vec<String> = std::iter::from_fn(|| rx.try_recv().ok())
            .map(|event| match event {
                LspEvent::Hover { text } => text,
                _ => String::new(),
            })
            .collect();

        assert_eq!(
            queued,
            ["0", "1"],
            "the queue keeps what fits and drops the rest"
        );
    }

    #[test]
    fn test_emit_accepts_events_again_once_the_queue_drains() {
        // Dropping is not a latch: a host that catches up starts receiving
        // again, so a burst costs the events in it and nothing after.
        let (tx, rx) = mpsc::sync_channel::<LspEvent>(1);
        emit(&tx, LspEvent::Hover { text: "dropped-into".to_string() });
        emit(&tx, LspEvent::Hover { text: "dropped".to_string() });

        let _ = rx.try_recv();
        emit(&tx, LspEvent::Hover { text: "accepted".to_string() });

        let accepted = rx.try_recv().ok().and_then(|event| match event {
            LspEvent::Hover { text } => Some(text),
            _ => None,
        });

        assert_eq!(
            accepted.as_deref(),
            Some("accepted"),
            "the drained queue must accept a new event"
        );
    }

    #[test]
    fn test_next_id_increments() {
        let (client, _writer_rx, _events_rx) = test_client();
        assert_eq!(client.next_id(), 1);
        assert_eq!(client.next_id(), 2);
        assert_eq!(client.next_id(), 3);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_send_message_frames_and_forwards() {
        let (client, writer_rx, _events_rx) = test_client();
        client.send_message(&json!({"jsonrpc": "2.0", "method": "ping"}));

        let bytes = writer_rx.try_recv().expect("message forwarded to writer");
        assert_eq!(decode_sent(&bytes)["method"], "ping");
    }

    // -------------------------------------------------------------------------
    // did_open / did_change / did_save / did_close
    // -------------------------------------------------------------------------

    #[test]
    #[allow(clippy::expect_used, clippy::unwrap_used)]
    fn test_did_open_registers_document_and_notifies_server() {
        let (mut client, writer_rx, _events_rx) = test_client();
        let doc = document("file:///a.rs");

        client.did_open(&doc, "fn main() {}");

        assert!(client.documents.lock().unwrap().contains_key("file:///a.rs"));
        let bytes = writer_rx.try_recv().expect("didOpen sent");
        let value = decode_sent(&bytes);
        assert_eq!(value["method"], "textDocument/didOpen");
        assert_eq!(value["params"]["textDocument"]["text"], "fn main() {}");
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_did_change_converts_and_sends_content_changes() {
        let (mut client, writer_rx, _events_rx) = test_client();
        let doc = document("file:///a.rs");
        client.did_open(&doc, "hello");
        writer_rx.try_recv().expect("drain didOpen");

        let changes = vec![LspTextChange {
            range: range(0, 0, 0, 5),
            text: "hi".to_string(),
        }];
        client.did_change(&doc, &changes);

        let bytes = writer_rx.try_recv().expect("didChange sent");
        let value = decode_sent(&bytes);
        assert_eq!(value["method"], "textDocument/didChange");
        assert_eq!(value["params"]["contentChanges"][0]["text"], "hi");
    }

    #[test]
    #[allow(clippy::assertions_on_result_states)]
    fn test_did_change_without_open_document_sends_nothing() {
        let (mut client, writer_rx, _events_rx) = test_client();
        let doc = document("file:///missing.rs");
        let changes = vec![LspTextChange {
            range: range(0, 0, 0, 0),
            text: "x".to_string(),
        }];

        client.did_change(&doc, &changes);

        assert!(writer_rx.try_recv().is_err());
    }

    #[test]
    #[allow(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::panic,
        clippy::assertions_on_result_states
    )]
    fn test_did_change_desync_drops_document_and_logs() {
        let (mut client, writer_rx, events_rx) = test_client();
        let doc = document("file:///a.rs");
        client.did_open(&doc, "hello");
        writer_rx.try_recv().expect("drain didOpen");

        // Line 5 doesn't exist in a single-line document, forcing a desync.
        let changes = vec![LspTextChange {
            range: range(5, 0, 5, 0),
            text: "x".to_string(),
        }];
        client.did_change(&doc, &changes);

        assert!(writer_rx.try_recv().is_err(), "no didChange after desync");
        assert!(!client.documents.lock().unwrap().contains_key("file:///a.rs"));

        match events_rx.try_recv().expect("desync logged") {
            LspEvent::Log { message, .. } => {
                assert!(message.contains("desynchronized"));
            }
            _ => panic!("expected LspEvent::Log"),
        }
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_did_save_sends_notification() {
        let (mut client, writer_rx, _events_rx) = test_client();
        let doc = document("file:///a.rs");

        client.did_save(&doc, "fn main() {}");

        let bytes = writer_rx.try_recv().expect("didSave sent");
        let value = decode_sent(&bytes);
        assert_eq!(value["method"], "textDocument/didSave");
        assert_eq!(value["params"]["text"], "fn main() {}");
    }

    #[test]
    #[allow(clippy::expect_used, clippy::unwrap_used)]
    fn test_did_close_removes_document_and_notifies_server() {
        let (mut client, writer_rx, _events_rx) = test_client();
        let doc = document("file:///a.rs");
        client.did_open(&doc, "hello");
        writer_rx.try_recv().expect("drain didOpen");

        client.did_close(&doc);

        assert!(!client.documents.lock().unwrap().contains_key("file:///a.rs"));
        let bytes = writer_rx.try_recv().expect("didClose sent");
        assert_eq!(decode_sent(&bytes)["method"], "textDocument/didClose");
    }

    // -------------------------------------------------------------------------
    // request_hover / request_completion / request_definition
    // -------------------------------------------------------------------------

    #[test]
    #[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    fn test_request_hover_registers_pending_and_sends_request() {
        let (mut client, writer_rx, _events_rx) = test_client();
        let doc = document("file:///a.rs");
        client.did_open(&doc, "hello");
        writer_rx.try_recv().expect("drain didOpen");

        client.request_hover(&doc, LspPosition { line: 0, character: 2 });

        let bytes = writer_rx.try_recv().expect("hover request sent");
        let value = decode_sent(&bytes);
        assert_eq!(value["method"], "textDocument/hover");
        let id = value["id"].as_u64().expect("id present");

        let pending = client.pending_requests.lock().unwrap();
        match pending.get(&id).map(|p| &p.kind) {
            Some(LspRequestKind::Hover) => {}
            _ => panic!("expected a pending Hover request"),
        }
    }

    #[test]
    #[allow(clippy::unwrap_used, clippy::assertions_on_result_states)]
    fn test_request_hover_without_open_document_does_nothing() {
        let (mut client, writer_rx, _events_rx) = test_client();
        let doc = document("file:///missing.rs");

        client.request_hover(&doc, LspPosition { line: 0, character: 0 });

        assert!(writer_rx.try_recv().is_err());
        assert!(client.pending_requests.lock().unwrap().is_empty());
    }

    #[test]
    #[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    fn test_request_completion_registers_pending_and_sends_request() {
        let (mut client, writer_rx, _events_rx) = test_client();
        let doc = document("file:///a.rs");
        client.did_open(&doc, "hello");
        writer_rx.try_recv().expect("drain didOpen");

        client.request_completion(&doc, LspPosition { line: 0, character: 2 });

        let bytes = writer_rx.try_recv().expect("completion request sent");
        let value = decode_sent(&bytes);
        assert_eq!(value["method"], "textDocument/completion");
        let id = value["id"].as_u64().expect("id present");

        let pending = client.pending_requests.lock().unwrap();
        match pending.get(&id).map(|p| &p.kind) {
            Some(LspRequestKind::Completion) => {}
            _ => panic!("expected a pending Completion request"),
        }
    }

    #[test]
    #[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    fn test_request_definition_registers_pending_and_sends_request() {
        let (mut client, writer_rx, _events_rx) = test_client();
        let doc = document("file:///a.rs");
        client.did_open(&doc, "hello");
        writer_rx.try_recv().expect("drain didOpen");

        client.request_definition(&doc, LspPosition { line: 0, character: 2 });

        let bytes = writer_rx.try_recv().expect("definition request sent");
        let value = decode_sent(&bytes);
        assert_eq!(value["method"], "textDocument/definition");
        let id = value["id"].as_u64().expect("id present");

        let pending = client.pending_requests.lock().unwrap();
        match pending.get(&id).map(|p| &p.kind) {
            Some(LspRequestKind::Definition) => {}
            _ => panic!("expected a pending Definition request"),
        }
    }

    // -------------------------------------------------------------------------
    // Drop
    // -------------------------------------------------------------------------

    #[test]
    #[allow(clippy::expect_used)]
    fn test_drop_sends_shutdown_then_exit() {
        let (client, writer_rx, _events_rx) = test_client();
        drop(client);

        let shutdown = writer_rx.try_recv().expect("shutdown sent");
        assert_eq!(decode_sent(&shutdown)["method"], "shutdown");

        let exit = writer_rx.try_recv().expect("exit sent");
        assert_eq!(decode_sent(&exit)["method"], "exit");
    }
}
