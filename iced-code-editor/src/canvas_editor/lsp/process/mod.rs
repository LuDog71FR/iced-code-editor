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
/// use iced_code_editor::LspEvent;
///
/// let (tx, rx) = mpsc::channel::<LspEvent>();
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
/// use iced_code_editor::{LspProcessClient, LspEvent};
///
/// let (tx, rx) = mpsc::channel::<LspEvent>();
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
    events: mpsc::Sender<LspEvent>,
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
    /// use iced_code_editor::{LspProcessClient, LspEvent};
    ///
    /// let (tx, _rx) = mpsc::channel::<LspEvent>();
    /// let client = LspProcessClient::new_with_server(
    ///     "file:///tmp/project",
    ///     tx,
    ///     "lua-language-server",
    /// );
    /// assert!(client.is_ok());
    /// ```
    pub fn new_with_server(
        root_uri: &str,
        events: mpsc::Sender<LspEvent>,
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
        events: mpsc::Sender<LspEvent>,
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
                let _ = events_log.send(LspEvent::Log {
                    server_key: server_key_log.clone(),
                    message: line.to_string(),
                });
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
                let _ = self.events.send(LspEvent::Log {
                    server_key: self.server_key.clone(),
                    message: format!(
                        "Local document mirror for {uri} desynchronized \
                         (a change referenced a line outside the tracked \
                         document); dropping it until the next open."
                    ),
                });
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
