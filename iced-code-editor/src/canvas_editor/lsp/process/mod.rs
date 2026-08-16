//! LSP (Language Server Protocol) Process Client implementation.
//!
//! This module provides a client for communicating with LSP servers via stdio.
//! It handles document synchronization, hover requests, and completion requests.
//!
//! Enable with the `lsp-process` Cargo feature. Not available on WASM targets.

pub mod config;
pub mod overlay;

/// JSON-RPC method name for server-push progress notifications.
const METHOD_PROGRESS: &str = "$/progress";
/// JSON-RPC method name sent by the server when it creates a work-done token.
const METHOD_WORK_DONE_PROGRESS_CREATE: &str = "window/workDoneProgress/create";
/// Progress `kind` value that signals the end of a work-done sequence.
const PROGRESS_KIND_END: &str = "end";
/// Maximum accepted `Content-Length` for a single LSP frame (64 MiB).
///
/// The body length is attacker-controlled input: it comes from the language
/// server process, whose binary is resolved through `PATH` or an environment
/// variable. Without a cap, a malformed or hostile server announcing a
/// multi-gigabyte frame would force an unbounded allocation and take the
/// editor down. Real LSP payloads stay orders of magnitude below this.
const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;

use self::config::{
    LspCommand, ensure_rust_analyzer_config, lsp_server_config,
    resolve_lsp_command,
};
use crate::buffer::text_utils::char_to_byte_index;
use crate::canvas_editor::lsp::{
    LspClient, LspDocument, LspPosition, LspRange, LspTextChange,
};
use serde_json::json;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

// =============================================================================
// Text Model - Internal document representation for tracking text changes
// =============================================================================

/// Internal representation of a text document as a vector of lines.
///
/// Used to track document state and convert between character and byte indices.
struct TextModel {
    /// The document content stored as a vector of lines (without newline characters)
    lines: Vec<String>,
}

impl TextModel {
    /// Creates a new `TextModel` from a string.
    ///
    /// Splits the text into lines for easier manipulation.
    /// An empty string creates a single empty line.
    fn from_text(text: &str) -> Self {
        let lines = if text.is_empty() {
            vec![String::new()]
        } else {
            text.lines().map(String::from).collect()
        };
        Self { lines }
    }

    /// Applies a text change (edit) to the document.
    ///
    /// Handles multi-line insertions and deletions by splicing the lines vector.
    ///
    /// Returns `false` without modifying `self` when `change`'s range falls
    /// outside the current line count. That means this mirror has drifted
    /// out of sync with the real document (e.g. a change was computed
    /// against a state this mirror never saw) — the caller must not trust
    /// this `TextModel` for further position translation once that happens.
    fn apply_change(&mut self, change: &LspTextChange) -> bool {
        let start_line = change.range.start.line as usize;
        let end_line = change.range.end.line as usize;

        if start_line >= self.lines.len() || end_line >= self.lines.len() {
            return false;
        }

        let start_col = change.range.start.character as usize;
        let end_col = change.range.end.character as usize;

        let start_byte = char_to_byte_index(&self.lines[start_line], start_col);
        let end_byte = char_to_byte_index(&self.lines[end_line], end_col);

        let prefix = self.lines[start_line][..start_byte].to_string();
        let suffix = self.lines[end_line][end_byte..].to_string();

        let inserted: Vec<&str> = change.text.split('\n').collect();
        let mut replacement: Vec<String> = Vec::new();

        if inserted.len() == 1 {
            replacement.push(format!("{}{}{}", prefix, inserted[0], suffix));
        } else {
            replacement.push(format!("{}{}", prefix, inserted[0]));
            for mid in inserted.iter().take(inserted.len() - 1).skip(1) {
                replacement.push((*mid).to_string());
            }
            replacement.push(format!(
                "{}{}",
                inserted[inserted.len() - 1],
                suffix
            ));
        }

        self.lines.splice(start_line..=end_line, replacement);
        true
    }

    /// Converts a UTF-8 character position to a UTF-16 position.
    ///
    /// This is necessary because LSP uses UTF-16 for character positions.
    fn to_utf16_position(&self, position: LspPosition) -> LspPosition {
        let line_index = position.line as usize;
        let char_index = position.character as usize;
        let line = self.lines.get(line_index).map_or("", |l| l.as_str());

        let utf16_col =
            line.chars().take(char_index).map(|c| c.len_utf16() as u32).sum();
        LspPosition { line: position.line, character: utf16_col }
    }
}

// =============================================================================
// Document State - Tracks the state of an open document
// =============================================================================

/// Represents the state of a single open document.
struct DocumentState {
    /// The text content of the document
    text: TextModel,
}

/// Applies `changes` to `state`'s mirror in order, converting each to the
/// UTF-16 JSON shape LSP's `didChange` notification expects.
///
/// Returns `None` — instead of the changes converted so far — the moment any
/// change's range falls outside the mirror. That means the mirror has
/// already drifted out of sync with the real document, so every change from
/// that point on (their coordinates are relative to the mirror's state after
/// prior changes) is computed against a state the mirror doesn't actually
/// have; forwarding a partial or best-effort batch would tell the server
/// something that isn't true. The caller is responsible for treating the
/// document as stale (see [`LspProcessClient::apply_change_and_convert`]).
fn apply_changes_to_document(
    state: &mut DocumentState,
    changes: &[LspTextChange],
) -> Option<Vec<serde_json::Value>> {
    let mut out = Vec::with_capacity(changes.len());
    for change in changes {
        let start = state.text.to_utf16_position(change.range.start);
        let end = state.text.to_utf16_position(change.range.end);

        if !state.text.apply_change(change) {
            return None;
        }

        out.push(json!({
            "range": {
                "start": { "line": start.line, "character": start.character },
                "end": { "line": end.line, "character": end.character }
            },
            "text": change.text
        }));
    }
    Some(out)
}

// =============================================================================
// LSP Request Types
// =============================================================================

/// Enumeration of LSP request types that we track for response handling.
enum LspRequestKind {
    /// Hover request — shows type information and documentation
    Hover,
    /// Completion request — provides auto-complete suggestions
    Completion,
    /// Definition request — go to definition
    Definition,
}

/// A request awaiting a server response, tracked with the time it was sent.
struct PendingRequest {
    /// Which kind of request this is, used to route the eventual response.
    kind: LspRequestKind,
    /// When the request was sent, used by [`evict_expired_requests`] to
    /// drop it if the server never responds.
    requested_at: Instant,
}

/// Requests older than this are treated as abandoned.
///
/// A hung or crashed language server can leave a hover/completion/definition
/// request pending forever: `pending_requests` only removes an entry when a
/// matching response arrives (see `handle_client_response`), so without a
/// timeout it would grow without bound for the lifetime of the client. Real
/// LSP responses arrive in well under a second; 30s is generous headroom for
/// a slow server while still bounding the leak from one that never replies.
const PENDING_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Drops entries from `pending` older than [`PENDING_REQUEST_TIMEOUT`].
///
/// Called whenever a new request is registered (see
/// [`LspProcessClient::request_hover`] and friends), so a server that stops
/// responding can't grow this map without bound.
fn evict_expired_requests(pending: &mut HashMap<u64, PendingRequest>) {
    let now = Instant::now();
    pending.retain(|_, entry| {
        now.saturating_duration_since(entry.requested_at)
            < PENDING_REQUEST_TIMEOUT
    });
}

// =============================================================================
// LSP Events - Events sent back to the main application
// =============================================================================

/// Events that can be sent from the LSP client to the application.
///
/// Receive these by polling the `mpsc::Receiver` you pass to
/// [`LspProcessClient::new_with_server`].
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
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                let Ok(line) = line else { break };
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

/// Reads one `Content-Length`-framed message body from `reader`.
///
/// Returns `None` when the stream ends, when the body cannot be read in full,
/// or when the announced frame exceeds [`MAX_MESSAGE_BYTES`]. An oversized
/// frame cannot be skipped reliably — the body length is exactly what is not
/// trustworthy — so the caller must stop reading the stream rather than try to
/// resynchronise mid-message.
///
/// Header lines other than `Content-Length` are ignored. A header block
/// carrying no `Content-Length` yields an empty body, which the caller then
/// discards as invalid JSON.
fn read_message(reader: &mut impl BufRead) -> Option<Vec<u8>> {
    let mut content_length: Option<usize> = None;
    let mut line = String::new();

    loop {
        line.clear();
        // A zero-length read means end of stream, not an empty header line.
        reader.read_line(&mut line).ok().filter(|n| *n > 0)?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:")
            && let Ok(len) = value.trim().parse::<usize>()
        {
            content_length = Some(len);
        }
    }

    let len = content_length.unwrap_or(0);
    if len > MAX_MESSAGE_BYTES {
        return None;
    }

    let mut buf = vec![0u8; len];
    if reader.read_exact(&mut buf).is_err() {
        return None;
    }
    Some(buf)
}

/// Frames a JSON-RPC value as `Content-Length: N\r\n\r\n<body>` bytes, ready
/// to be written to an LSP server's stdin.
///
/// Returns `None` if `value` cannot be serialized to JSON.
///
/// # Examples
///
/// ```ignore
/// let framed = frame_message(&serde_json::json!({"jsonrpc": "2.0"})).unwrap();
/// assert!(framed.starts_with(b"Content-Length: "));
/// ```
fn frame_message(value: &serde_json::Value) -> Option<Vec<u8>> {
    let data = serde_json::to_vec(value).ok()?;
    let mut framed =
        format!("Content-Length: {}\r\n\r\n", data.len()).into_bytes();
    framed.extend_from_slice(&data);
    Some(framed)
}

/// Handles an LSP server request that requires a JSON-RPC response.
///
/// Currently handles `window/workDoneProgress/create` by replying with a null
/// result. Unknown methods are silently ignored.
fn handle_server_request(id: u64, method: &str, tx: &mpsc::Sender<Vec<u8>>) {
    if method == METHOD_WORK_DONE_PROGRESS_CREATE {
        let response = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": null
        });
        if let Some(bytes) = frame_message(&response) {
            let _ = tx.send(bytes);
        }
    }
}

/// Dispatches a server response to the appropriate pending request handler.
///
/// Looks up the request kind by `id`, parses the result, and emits a
/// [`LspEvent::Hover`], [`LspEvent::Completion`], or [`LspEvent::Definition`].
fn handle_client_response(
    id: u64,
    value: &serde_json::Value,
    pending: &Arc<Mutex<HashMap<u64, PendingRequest>>>,
    events: &mpsc::Sender<LspEvent>,
) {
    let kind = {
        let mut map = pending.lock().unwrap_or_else(|e| e.into_inner());
        map.remove(&id).map(|entry| entry.kind)
    };

    let Some(kind) = kind else { return };
    let result = value.get("result").unwrap_or(&serde_json::Value::Null);

    match kind {
        LspRequestKind::Hover => {
            let text = parse_hover_text(result).unwrap_or_default();
            let _ = events.send(LspEvent::Hover { text });
        }
        LspRequestKind::Completion => {
            let items = parse_completion_items(result);
            if !items.is_empty() {
                let _ = events.send(LspEvent::Completion { items });
            }
        }
        LspRequestKind::Definition => {
            if let Some((uri, range)) = parse_definition_location(result) {
                let _ = events.send(LspEvent::Definition { uri, range });
            }
        }
    }
}

/// Handles a server-initiated notification (e.g. `$/progress`).
///
/// Parses the progress payload and emits a [`LspEvent::Progress`].
/// Notifications for unknown methods are silently ignored.
fn handle_server_notification(
    method: &str,
    params: &serde_json::Value,
    events: &mpsc::Sender<LspEvent>,
    server_key: &str,
) {
    if method != METHOD_PROGRESS {
        return;
    }

    let Some(token) = params.get("token").and_then(|t| {
        t.as_str()
            .map(String::from)
            .or_else(|| t.as_i64().map(|i| i.to_string()))
    }) else {
        return;
    };

    let Some(val) = params.get("value") else { return };

    let kind = val.get("kind").and_then(|k| k.as_str()).unwrap_or("");
    let title = val
        .get("title")
        .and_then(|t| t.as_str())
        .map(String::from)
        .unwrap_or_default();
    let message = val.get("message").and_then(|m| m.as_str()).map(String::from);
    let percentage =
        val.get("percentage").and_then(|p| p.as_u64()).map(|p| p as u32);
    let done = kind == PROGRESS_KIND_END;

    let _ = events.send(LspEvent::Progress {
        token,
        server_key: server_key.to_string(),
        title,
        message,
        percentage,
        done,
    });
}

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
// LSP Response Parsing Functions
// =============================================================================

/// Parses hover text from an LSP hover response.
fn parse_hover_text(result: &serde_json::Value) -> Option<String> {
    let contents = result.get("contents")?;
    hover_text_from_contents(contents)
}

/// Recursively extracts hover text from various content formats.
///
/// Handles strings, arrays, and objects with a `"value"` field.
fn hover_text_from_contents(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Array(items) => {
            let parts: Vec<String> =
                items.iter().filter_map(hover_text_from_contents).collect();
            if parts.is_empty() { None } else { Some(parts.join("\n")) }
        }
        serde_json::Value::Object(map) => {
            map.get("value").and_then(|v| v.as_str()).map(String::from)
        }
        _ => None,
    }
}

/// Parses completion items from an LSP completion response.
///
/// Handles both array responses and object responses with an `"items"` field.
fn parse_completion_items(result: &serde_json::Value) -> Vec<String> {
    let mut items = Vec::new();

    if let Some(array) = result.as_array() {
        items.extend(array.iter());
    } else if let Some(array) = result.get("items").and_then(|v| v.as_array()) {
        items.extend(array.iter());
    }

    items
        .iter()
        .filter_map(|item| item.get("label").and_then(|v| v.as_str()))
        .map(String::from)
        .collect()
}

/// Parses a JSON-RPC `Range` object (`{start: {line, character}, end: {...}}`)
/// into an [`LspRange`].
///
/// Returns `None` if either endpoint is missing or has the wrong shape.
fn extract_range(range_val: &serde_json::Value) -> Option<LspRange> {
    let start = range_val.get("start")?;
    let end = range_val.get("end")?;

    Some(LspRange {
        start: LspPosition {
            line: start.get("line")?.as_u64()? as u32,
            character: start.get("character")?.as_u64()? as u32,
        },
        end: LspPosition {
            line: end.get("line")?.as_u64()? as u32,
            character: end.get("character")?.as_u64()? as u32,
        },
    })
}

/// Extracts `(uri, range)` from an LSP `Location` object.
fn extract_location(loc: &serde_json::Value) -> Option<(String, LspRange)> {
    let uri = loc.get("uri")?.as_str()?.to_string();
    let range = extract_range(loc.get("range")?)?;
    Some((uri, range))
}

/// Extracts `(uri, range)` from an LSP `LocationLink` object.
///
/// Prefers `targetSelectionRange` (the precise symbol range) and falls back
/// to `targetRange` (the enclosing range) when it is absent.
fn extract_link(link: &serde_json::Value) -> Option<(String, LspRange)> {
    let uri = link.get("targetUri")?.as_str()?.to_string();
    let range_val =
        link.get("targetSelectionRange").or(link.get("targetRange"))?;
    let range = extract_range(range_val)?;
    Some((uri, range))
}

/// Parses definition location from an LSP definition response.
///
/// Handles `Location`, `Location[]`, and `LocationLink[]` responses.
fn parse_definition_location(
    result: &serde_json::Value,
) -> Option<(String, LspRange)> {
    if let Some(array) = result.as_array() {
        if let Some(first) = array.first() {
            if first.get("targetUri").is_some() {
                extract_link(first)
            } else {
                extract_location(first)
            }
        } else {
            None
        }
    } else if result.is_object() {
        extract_location(result)
    } else {
        None
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

#[cfg(test)]
mod tests {
    // Several helpers and tests below carry an `#[allow]` for `expect_used`,
    // `panic` or `unwrap_used`. In test code a panic *is* the failure report:
    // `expect("expected a Hover event")` names the broken expectation far more
    // precisely than an `assert!(false)` workaround would. The workspace denies
    // these lints to protect production code, not tests — this mirrors the
    // existing per-test allows in `update.rs` and `selection.rs`.
    use super::*;

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

    /// Builds a [`PendingRequest`] of `kind`, sent "now" for test purposes.
    fn pending_request(kind: LspRequestKind) -> PendingRequest {
        PendingRequest { kind, requested_at: Instant::now() }
    }

    // -------------------------------------------------------------------------
    // evict_expired_requests
    // -------------------------------------------------------------------------

    #[test]
    fn test_evict_expired_requests_drops_only_stale_entries() {
        let mut pending = HashMap::new();
        pending.insert(
            1u64,
            PendingRequest {
                kind: LspRequestKind::Hover,
                requested_at: Instant::now()
                    - PENDING_REQUEST_TIMEOUT
                    - Duration::from_secs(1),
            },
        );
        pending.insert(2u64, pending_request(LspRequestKind::Completion));

        evict_expired_requests(&mut pending);

        assert!(!pending.contains_key(&1));
        assert!(pending.contains_key(&2));
    }

    #[test]
    fn test_evict_expired_requests_keeps_fresh_entries() {
        let mut pending = HashMap::new();
        pending.insert(1u64, pending_request(LspRequestKind::Hover));
        pending.insert(2u64, pending_request(LspRequestKind::Definition));

        evict_expired_requests(&mut pending);

        assert_eq!(pending.len(), 2);
    }

    // -------------------------------------------------------------------------
    // TextModel::apply_change
    // -------------------------------------------------------------------------

    fn change(
        start_line: u32,
        start_char: u32,
        end_line: u32,
        end_char: u32,
        text: &str,
    ) -> LspTextChange {
        LspTextChange {
            range: LspRange {
                start: LspPosition { line: start_line, character: start_char },
                end: LspPosition { line: end_line, character: end_char },
            },
            text: text.to_string(),
        }
    }

    #[test]
    fn test_text_model_apply_change_in_range_succeeds() {
        let mut model = TextModel::from_text("hello\nworld");
        let applied = model.apply_change(&change(0, 0, 0, 5, "goodbye"));
        assert!(applied);
        assert_eq!(
            model.lines,
            vec!["goodbye".to_string(), "world".to_string()]
        );
    }

    #[test]
    fn test_text_model_apply_change_out_of_range_line_fails_without_mutating() {
        let mut model = TextModel::from_text("hello\nworld");
        let original = model.lines.clone();

        // Line 5 does not exist in a 2-line document.
        let applied = model.apply_change(&change(5, 0, 5, 0, "x"));

        assert!(!applied);
        assert_eq!(model.lines, original);
    }

    // -------------------------------------------------------------------------
    // apply_changes_to_document
    // -------------------------------------------------------------------------

    #[test]
    #[allow(clippy::panic)]
    fn test_apply_changes_to_document_converts_every_change() {
        let mut state =
            DocumentState { text: TextModel::from_text("hello\nworld") };

        let changes =
            vec![change(0, 0, 0, 5, "hi"), change(1, 0, 1, 5, "earth")];
        let Some(out) = apply_changes_to_document(&mut state, &changes) else {
            panic!("in-range changes must convert");
        };

        assert_eq!(out.len(), 2);
        assert_eq!(out[0]["text"], "hi");
        assert_eq!(out[1]["text"], "earth");
        assert_eq!(
            state.text.lines,
            vec!["hi".to_string(), "earth".to_string()]
        );
    }

    #[test]
    fn test_apply_changes_to_document_stops_at_first_desync() {
        let mut state = DocumentState { text: TextModel::from_text("hello") };

        // The first change is valid and does get applied to the mirror; the
        // second references a line that doesn't exist. The whole batch must
        // report `None` rather than a partial result, since every change
        // after the desync point is computed against a mirror state that no
        // longer reflects reality.
        let changes =
            vec![change(0, 0, 0, 5, "hi"), change(9, 0, 9, 0, "unreachable")];
        let out = apply_changes_to_document(&mut state, &changes);

        assert!(out.is_none());
        // The first change was still applied before the desync was found —
        // documenting why the caller must discard the whole `DocumentState`
        // rather than trying to salvage it.
        assert_eq!(state.text.lines, vec!["hi".to_string()]);
    }

    // -------------------------------------------------------------------------
    // read_message
    // -------------------------------------------------------------------------

    #[test]
    fn test_read_message_reads_framed_body() {
        let mut stream = std::io::Cursor::new(
            b"Content-Length: 9\r\n\r\n{\"id\":42}".to_vec(),
        );
        assert_eq!(
            read_message(&mut stream).as_deref(),
            Some(&b"{\"id\":42}"[..])
        );
    }

    #[test]
    fn test_read_message_ignores_other_headers() {
        let mut stream = std::io::Cursor::new(
            b"Content-Type: application/vscode-jsonrpc\r\nContent-Length: 2\r\n\r\n{}"
                .to_vec(),
        );
        assert_eq!(read_message(&mut stream).as_deref(), Some(&b"{}"[..]));
    }

    #[test]
    fn test_read_message_reads_consecutive_frames() {
        let mut stream = std::io::Cursor::new(
            b"Content-Length: 2\r\n\r\n{}Content-Length: 4\r\n\r\n[1,]"
                .to_vec(),
        );
        assert_eq!(read_message(&mut stream).as_deref(), Some(&b"{}"[..]));
        assert_eq!(read_message(&mut stream).as_deref(), Some(&b"[1,]"[..]));
        assert!(read_message(&mut stream).is_none(), "stream is exhausted");
    }

    #[test]
    fn test_read_message_rejects_oversized_frame() {
        // The announced length exceeds MAX_MESSAGE_BYTES: the frame must be
        // refused without allocating it.
        let header =
            format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE_BYTES + 1);
        let mut stream = std::io::Cursor::new(header.into_bytes());
        assert!(read_message(&mut stream).is_none());
    }

    #[test]
    fn test_read_message_rejects_truncated_body() {
        let mut stream =
            std::io::Cursor::new(b"Content-Length: 10\r\n\r\nabc".to_vec());
        assert!(read_message(&mut stream).is_none());
    }

    #[test]
    fn test_read_message_ends_on_empty_stream() {
        let mut stream = std::io::Cursor::new(Vec::new());
        assert!(read_message(&mut stream).is_none());
    }

    #[test]
    fn test_read_message_without_content_length_yields_empty_body() {
        // No Content-Length: the caller gets an empty body and discards it as
        // invalid JSON, then keeps reading.
        let mut stream = std::io::Cursor::new(b"X-Unknown: 1\r\n\r\n".to_vec());
        assert_eq!(read_message(&mut stream).as_deref(), Some(&b""[..]));
    }

    // -------------------------------------------------------------------------
    // frame_message
    // -------------------------------------------------------------------------

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_frame_message_builds_content_length_header() {
        let value = serde_json::json!({"jsonrpc": "2.0", "id": 1});
        let data = serde_json::to_vec(&value).unwrap();
        let framed = frame_message(&value).unwrap();

        let expected_header = format!("Content-Length: {}\r\n\r\n", data.len());
        assert!(framed.starts_with(expected_header.as_bytes()));
        assert_eq!(&framed[expected_header.len()..], data.as_slice());
    }

    // -------------------------------------------------------------------------
    // extract_range / extract_location / extract_link
    // -------------------------------------------------------------------------

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_extract_range_parses_start_and_end() {
        let range_val = serde_json::json!({
            "start": { "line": 1, "character": 2 },
            "end": { "line": 3, "character": 4 }
        });
        let range = extract_range(&range_val).unwrap();
        assert_eq!(range.start, LspPosition { line: 1, character: 2 });
        assert_eq!(range.end, LspPosition { line: 3, character: 4 });
    }

    #[test]
    fn test_extract_range_missing_end_returns_none() {
        let range_val = serde_json::json!({
            "start": { "line": 1, "character": 2 }
        });
        assert!(extract_range(&range_val).is_none());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_extract_location_reads_uri_and_range() {
        let loc = serde_json::json!({
            "uri": "file:///a.rs",
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 1 }
            }
        });
        let (uri, range) = extract_location(&loc).unwrap();
        assert_eq!(uri, "file:///a.rs");
        assert_eq!(range.start, LspPosition { line: 0, character: 0 });
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_extract_link_prefers_target_selection_range() {
        let link = serde_json::json!({
            "targetUri": "file:///b.rs",
            "targetRange": {
                "start": { "line": 10, "character": 0 },
                "end": { "line": 20, "character": 0 }
            },
            "targetSelectionRange": {
                "start": { "line": 12, "character": 4 },
                "end": { "line": 12, "character": 8 }
            }
        });
        let (uri, range) = extract_link(&link).unwrap();
        assert_eq!(uri, "file:///b.rs");
        assert_eq!(range.start, LspPosition { line: 12, character: 4 });
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_extract_link_falls_back_to_target_range() {
        let link = serde_json::json!({
            "targetUri": "file:///c.rs",
            "targetRange": {
                "start": { "line": 5, "character": 0 },
                "end": { "line": 6, "character": 0 }
            }
        });
        let (uri, range) = extract_link(&link).unwrap();
        assert_eq!(uri, "file:///c.rs");
        assert_eq!(range.start, LspPosition { line: 5, character: 0 });
    }

    // -------------------------------------------------------------------------
    // handle_server_request
    // -------------------------------------------------------------------------

    #[test]
    #[allow(clippy::expect_used)]
    fn test_handle_server_request_work_done_progress_create() {
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        handle_server_request(42, METHOD_WORK_DONE_PROGRESS_CREATE, &tx);

        let bytes = rx.try_recv().expect("expected a response on the channel");
        let value = decode_sent(&bytes);
        assert_eq!(value["id"], 42);
        assert_eq!(value["jsonrpc"], "2.0");
        assert!(value["result"].is_null());
    }

    #[test]
    fn test_handle_server_request_unknown_method_ignored() {
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        handle_server_request(1, "unknown/method", &tx);
        assert!(
            rx.try_recv().is_err(),
            "unknown methods must not send a reply"
        );
    }

    // -------------------------------------------------------------------------
    // handle_client_response
    // -------------------------------------------------------------------------

    #[test]
    #[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    fn test_handle_client_response_hover() {
        let (events_tx, events_rx) = mpsc::channel::<LspEvent>();
        let pending = Arc::new(Mutex::new(HashMap::new()));
        pending
            .lock()
            .unwrap()
            .insert(1u64, pending_request(LspRequestKind::Hover));

        let value = serde_json::json!({
            "id": 1,
            "result": { "contents": { "value": "hover info" } }
        });
        handle_client_response(1, &value, &pending, &events_tx);

        match events_rx.try_recv().expect("expected a Hover event") {
            LspEvent::Hover { text } => assert_eq!(text, "hover info"),
            _ => panic!("expected LspEvent::Hover"),
        }
        assert!(pending.lock().unwrap().is_empty());
    }

    #[test]
    #[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    fn test_handle_client_response_completion() {
        let (events_tx, events_rx) = mpsc::channel::<LspEvent>();
        let pending = Arc::new(Mutex::new(HashMap::new()));
        pending
            .lock()
            .unwrap()
            .insert(2u64, pending_request(LspRequestKind::Completion));

        let value = serde_json::json!({
            "id": 2,
            "result": { "items": [{ "label": "foo" }, { "label": "bar" }] }
        });
        handle_client_response(2, &value, &pending, &events_tx);

        match events_rx.try_recv().expect("expected a Completion event") {
            LspEvent::Completion { items } => {
                assert_eq!(items, vec!["foo", "bar"]);
            }
            _ => panic!("expected LspEvent::Completion"),
        }
    }

    #[test]
    #[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    fn test_handle_client_response_definition() {
        let (events_tx, events_rx) = mpsc::channel::<LspEvent>();
        let pending = Arc::new(Mutex::new(HashMap::new()));
        pending
            .lock()
            .unwrap()
            .insert(3u64, pending_request(LspRequestKind::Definition));

        let value = serde_json::json!({
            "id": 3,
            "result": {
                "uri": "file:///foo/bar.rs",
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 5 }
                }
            }
        });
        handle_client_response(3, &value, &pending, &events_tx);

        match events_rx.try_recv().expect("expected a Definition event") {
            LspEvent::Definition { uri, .. } => {
                assert_eq!(uri, "file:///foo/bar.rs");
            }
            _ => panic!("expected LspEvent::Definition"),
        }
    }

    #[test]
    fn test_handle_client_response_unknown_id_ignored() {
        let (events_tx, events_rx) = mpsc::channel::<LspEvent>();
        let pending = Arc::new(Mutex::new(HashMap::new()));

        let value = serde_json::json!({ "id": 99, "result": null });
        handle_client_response(99, &value, &pending, &events_tx);
        assert!(
            events_rx.try_recv().is_err(),
            "unknown IDs must not emit events"
        );
    }

    // -------------------------------------------------------------------------
    // handle_server_notification
    // -------------------------------------------------------------------------

    #[test]
    #[allow(clippy::expect_used, clippy::panic)]
    fn test_handle_server_notification_progress_done() {
        let (events_tx, events_rx) = mpsc::channel::<LspEvent>();
        let params = serde_json::json!({
            "token": "my-token",
            "value": {
                "kind": "end",
                "title": "Indexing",
                "message": "done"
            }
        });

        handle_server_notification(
            METHOD_PROGRESS,
            &params,
            &events_tx,
            "lua-ls",
        );

        match events_rx.try_recv().expect("expected a Progress event") {
            LspEvent::Progress { token, done, server_key, .. } => {
                assert_eq!(token, "my-token");
                assert!(done);
                assert_eq!(server_key, "lua-ls");
            }
            _ => panic!("expected LspEvent::Progress"),
        }
    }

    #[test]
    #[allow(clippy::expect_used, clippy::panic)]
    fn test_handle_server_notification_progress_not_done() {
        let (events_tx, events_rx) = mpsc::channel::<LspEvent>();
        let params = serde_json::json!({
            "token": "tok",
            "value": { "kind": "report", "title": "Building" }
        });

        handle_server_notification(
            METHOD_PROGRESS,
            &params,
            &events_tx,
            "rust-analyzer",
        );

        match events_rx.try_recv().expect("expected a Progress event") {
            LspEvent::Progress { done, .. } => assert!(!done),
            _ => panic!("expected LspEvent::Progress"),
        }
    }

    #[test]
    fn test_handle_server_notification_unknown_method_ignored() {
        let (events_tx, events_rx) = mpsc::channel::<LspEvent>();
        let params = serde_json::json!({});
        handle_server_notification(
            "$/somethingElse",
            &params,
            &events_tx,
            "server",
        );
        assert!(events_rx.try_recv().is_err());
    }
}
