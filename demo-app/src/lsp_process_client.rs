// LSP (Language Server Protocol) Process Client Implementation
// This module provides a client for communicating with LSP servers via stdio.
// It handles document synchronization, hover requests, and completion requests.

// Only compile this module for non-WASM32 targets since it uses process spawning
#![cfg(not(target_arch = "wasm32"))]

use crate::lsp_config::{
    LspCommand, ensure_rust_analyzer_config, lsp_server_config,
    resolve_lsp_command,
};
use iced_code_editor::{LspClient, LspDocument, LspPosition, LspTextChange};
use serde_json::json;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

// =============================================================================
// Text Model - Internal document representation for tracking text changes
// =============================================================================

/// Internal representation of a text document as a vector of lines.
/// This is used to track document state and convert between character and byte indices.
struct TextModel {
    /// The document content stored as a vector of lines (without newline characters)
    lines: Vec<String>,
}

impl TextModel {
    /// Creates a new TextModel from a string.
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
    /// Handles multi-line insertions and deletions by splicing the lines vector.
    fn apply_change(&mut self, change: &LspTextChange) {
        // Extract the range of the change
        let start_line = change.range.start.line as usize;
        let end_line = change.range.end.line as usize;

        // Validate that the range is within bounds
        if start_line >= self.lines.len() || end_line >= self.lines.len() {
            return;
        }

        // Get column positions
        let start_col = change.range.start.character as usize;
        let end_col = change.range.end.character as usize;

        // Convert character indices to byte indices for string slicing
        let start_byte = char_to_byte_index(&self.lines[start_line], start_col);
        let end_byte = char_to_byte_index(&self.lines[end_line], end_col);

        // Extract the prefix (text before the change on the start line)
        let prefix = self.lines[start_line][..start_byte].to_string();
        // Extract the suffix (text after the change on the end line)
        let suffix = self.lines[end_line][end_byte..].to_string();

        // Split the inserted text by newlines to handle multi-line insertions
        let inserted: Vec<&str> = change.text.split('\n').collect();
        let mut replacement: Vec<String> = Vec::new();

        if inserted.len() == 1 {
            // Single line insertion: combine prefix, inserted text, and suffix
            replacement.push(format!("{}{}{}", prefix, inserted[0], suffix));
        } else {
            // Multi-line insertion:
            // First line: prefix + first part of inserted text
            replacement.push(format!("{}{}", prefix, inserted[0]));
            // Middle lines: inserted text as-is
            for mid in inserted.iter().take(inserted.len() - 1).skip(1) {
                replacement.push((*mid).to_string());
            }
            // Last line: last part of inserted text + suffix
            replacement.push(format!(
                "{}{}",
                inserted[inserted.len() - 1],
                suffix
            ));
        }

        // Replace the affected lines with the new lines
        self.lines.splice(start_line..=end_line, replacement);
    }

    /// Converts a UTF-8 character position to a UTF-16 position.
    /// This is necessary because LSP uses UTF-16 for character positions.
    fn to_utf16_position(&self, position: LspPosition) -> LspPosition {
        let line_index = position.line as usize;
        let char_index = position.character as usize;
        let line = self.lines.get(line_index).map_or("", |l| l.as_str());

        // Sum the UTF-16 length of each character up to the target position
        let utf16_col =
            line.chars().take(char_index).map(|c| c.len_utf16() as u32).sum();
        LspPosition { line: position.line, character: utf16_col }
    }
}

/// Converts a character index to a byte index in a string.
/// Returns the length of the string if the index is out of bounds.
fn char_to_byte_index(s: &str, char_index: usize) -> usize {
    s.char_indices().nth(char_index).map_or(s.len(), |(idx, _)| idx)
}

// =============================================================================
// Document State - Tracks the state of an open document
// =============================================================================

/// Represents the state of a single open document
struct DocumentState {
    /// The text content of the document
    text: TextModel,
}

// =============================================================================
// LSP Request Types
// =============================================================================

/// Enumeration of LSP request types that we track for response handling
enum LspRequestKind {
    /// Hover request - shows type information and documentation
    Hover,
    /// Completion request - provides auto-complete suggestions
    Completion,
}

// =============================================================================
// LSP Events - Events sent back to the main application
// =============================================================================

/// Events that can be sent from the LSP client to the application
pub(crate) enum LspEvent {
    /// Hover information received
    Hover { text: String },
    /// Completion items received
    Completion { items: Vec<String> },
}

// =============================================================================
// LSP Process Client - Main client implementation
// =============================================================================

/// Client for communicating with an LSP server process.
/// Manages the lifecycle of the server process and handles all communication.
pub(crate) struct LspProcessClient {
    /// The child process running the LSP server
    child: Child,
    /// Channel for sending messages to the writer thread
    writer: mpsc::Sender<Vec<u8>>,
    /// Map of URI to document state for all open documents
    documents: Arc<Mutex<HashMap<String, DocumentState>>>,
    /// Counter for generating unique request IDs
    request_id: AtomicU64,
    /// Map of pending request IDs to their types (for response routing)
    pending_requests: Arc<Mutex<HashMap<u64, LspRequestKind>>>,
}

impl LspProcessClient {
    /// Creates a new LSP client with the specified server.
    ///
    /// # Arguments
    /// * `root_uri` - The root URI of the workspace
    /// * `events` - Channel to send LSP events back to the application
    /// * `server_key` - The key identifying which LSP server to use
    ///
    /// # Returns
    /// A new LspProcessClient instance or an error message
    pub(crate) fn new_with_server(
        root_uri: &str,
        events: mpsc::Sender<LspEvent>,
        server_key: &str,
    ) -> Result<Self, String> {
        // Find the configuration for the requested server
        let config = lsp_server_config(server_key)
            .ok_or_else(|| format!("Unsupported LSP server: {}", server_key))?;

        // Special handling for rust-analyzer to ensure config directory exists
        if server_key == "rust-analyzer" {
            ensure_rust_analyzer_config();
        }

        // Resolve the actual command to run
        let command = resolve_lsp_command(config)?;
        Self::new_with_command(root_uri, events, &command)
    }

    /// Creates a new LSP client with a specific command.
    /// This is the internal implementation that spawns the process.
    fn new_with_command(
        root_uri: &str,
        events: mpsc::Sender<LspEvent>,
        command: &LspCommand,
    ) -> Result<Self, String> {
        // Spawn the LSP server process with piped stdin/stdout
        let mut child = Command::new(&command.program)
            .args(&command.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| {
                // Provide helpful error messages for common issues
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

        // Take ownership of the process's stdin and stdout
        let stdin = child.stdin.take().ok_or("stdin unavailable")?;
        let stdout = child.stdout.take().ok_or("stdout unavailable")?;

        // Create channel for sending messages to the writer thread
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let pending_requests = Arc::new(Mutex::new(HashMap::new()));
        let pending_reader = pending_requests.clone();
        let events_reader = events;

        // Spawn the writer thread - sends messages to the LSP server
        let writer_thread = thread::spawn(move || {
            let mut input = stdin;
            for bytes in rx {
                if input.write_all(&bytes).is_err() {
                    break;
                }
                let _ = input.flush();
            }
        });

        // Spawn the reader thread - receives messages from the LSP server
        let reader_thread = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                // Parse the LSP message headers
                let mut content_length: Option<usize> = None;
                let mut line = String::new();

                // Read headers until we hit an empty line
                loop {
                    line.clear();
                    if reader
                        .read_line(&mut line)
                        .ok()
                        .filter(|n| *n > 0)
                        .is_none()
                    {
                        // EOF or error, exit the thread
                        return;
                    }
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        // Empty line signals end of headers
                        break;
                    }
                    // Parse Content-Length header
                    if let Some(value) = trimmed.strip_prefix("Content-Length:")
                        && let Ok(len) = value.trim().parse::<usize>()
                    {
                        content_length = Some(len);
                    }
                }

                // Read the message body based on Content-Length
                let Some(len) = content_length else { continue };
                let mut buf = vec![0u8; len];
                if reader.read_exact(&mut buf).is_err() {
                    return;
                }

                // Parse the JSON message and handle responses
                if let Ok(value) =
                    serde_json::from_slice::<serde_json::Value>(&buf)
                    && let Some(id) = value.get("id").and_then(|v| v.as_u64())
                {
                    // Look up the request type for this response
                    let kind = {
                        let mut pending = pending_reader
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        pending.remove(&id)
                    };

                    if let Some(kind) = kind {
                        let result = value
                            .get("result")
                            .unwrap_or(&serde_json::Value::Null);
                        match kind {
                            // Handle hover response
                            LspRequestKind::Hover => {
                                let text = parse_hover_text(result)
                                    .unwrap_or_default();
                                let _ = events_reader
                                    .send(LspEvent::Hover { text });
                            }
                            // Handle completion response
                            LspRequestKind::Completion => {
                                let items = parse_completion_items(result);
                                if !items.is_empty() {
                                    let _ = events_reader
                                        .send(LspEvent::Completion { items });
                                }
                            }
                        }
                    }
                }
            }
        });

        // Create the client instance
        let client = Self {
            child,
            writer: tx,
            documents: Arc::new(Mutex::new(HashMap::new())),
            request_id: AtomicU64::new(1),
            pending_requests,
        };

        // Send the initialize request to the LSP server
        // This is the first message that must be sent to establish the connection
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
                    }
                },
                "workspaceFolders": null
            }
        });
        client.send_message(&initialize);

        // Send the initialized notification
        // This tells the server that we're ready to receive notifications
        let initialized = json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        });
        client.send_message(&initialized);

        // Keep thread handles alive (they will be joined when dropped)
        let _ = writer_thread;
        let _ = reader_thread;

        Ok(client)
    }

    /// Generates the next unique request ID.
    /// Uses atomic operations for thread safety.
    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Sends a JSON-RPC message to the LSP server.
    /// Formats the message with the required Content-Length header.
    fn send_message(&self, value: &serde_json::Value) {
        if let Ok(data) = serde_json::to_vec(&value) {
            // Build the LSP message with Content-Length header
            let mut header =
                format!("Content-Length: {}\r\n\r\n", data.len()).into_bytes();
            header.extend_from_slice(&data);
            let _ = self.writer.send(header);
        }
    }

    /// Applies text changes to a document and converts them to JSON format.
    /// Also converts positions to UTF-16 as required by LSP.
    fn apply_change_and_convert(
        &self,
        uri: &str,
        changes: &[LspTextChange],
    ) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        let mut docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
        let Some(state) = docs.get_mut(uri) else { return out };

        for change in changes {
            // Convert positions to UTF-16 for LSP
            let start = state.text.to_utf16_position(change.range.start);
            let end = state.text.to_utf16_position(change.range.end);

            // Create the JSON representation of the change
            out.push(json!({
                "range": {
                    "start": { "line": start.line, "character": start.character },
                    "end": { "line": end.line, "character": end.character }
                },
                "text": change.text
            }));

            // Apply the change to our local copy
            state.text.apply_change(change);
        }
        out
    }
}

/// Clean up the LSP server process when the client is dropped.
/// Sends shutdown and exit notifications, then kills the process if needed.
impl Drop for LspProcessClient {
    fn drop(&mut self) {
        // Send the shutdown request
        let shutdown = json!({
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": "shutdown",
            "params": null
        });
        self.send_message(&shutdown);

        // Send the exit notification
        let exit = json!({
            "jsonrpc": "2.0",
            "method": "exit",
            "params": {}
        });
        self.send_message(&exit);

        // Kill the process if it's still running
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
/// Handles strings, arrays, and objects with a "value" field.
fn hover_text_from_contents(value: &serde_json::Value) -> Option<String> {
    match value {
        // Simple string content
        serde_json::Value::String(text) => Some(text.clone()),

        // Array of content items - combine with newlines
        serde_json::Value::Array(items) => {
            let parts: Vec<String> =
                items.iter().filter_map(hover_text_from_contents).collect();
            if parts.is_empty() { None } else { Some(parts.join("\n")) }
        }

        // Object with "value" field (e.g., MarkupContent)
        serde_json::Value::Object(map) => {
            map.get("value").and_then(|v| v.as_str()).map(String::from)
        }

        // Other types are not supported
        _ => None,
    }
}

/// Parses completion items from an LSP completion response.
/// Handles both array responses and object responses with "items" field.
fn parse_completion_items(result: &serde_json::Value) -> Vec<String> {
    let mut items = Vec::new();

    // Check if result is directly an array of CompletionItem
    if let Some(array) = result.as_array() {
        items.extend(array.iter());
    // Check if result is a CompletionList with an "items" array
    } else if let Some(array) = result.get("items").and_then(|v| v.as_array()) {
        items.extend(array.iter());
    }

    // Extract the "label" field from each completion item
    items
        .iter()
        .filter_map(|item| item.get("label").and_then(|v| v.as_str()))
        .map(String::from)
        .collect()
}

// =============================================================================
// LspClient Trait Implementation
// =============================================================================

/// Implementation of the LspClient trait for the process-based client.
/// This is the main interface used by the code editor to communicate with LSP servers.
impl LspClient for LspProcessClient {
    /// Called when a document is opened in the editor.
    /// Sends the textDocument/didOpen notification to the LSP server.
    fn did_open(&mut self, document: &LspDocument, text: &str) {
        // Store the document state locally for position conversions
        let mut docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
        docs.insert(
            document.uri.clone(),
            DocumentState { text: TextModel::from_text(text) },
        );

        // Send the didOpen notification
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

    /// Called when a document is modified in the editor.
    /// Sends the textDocument/didChange notification to the LSP server.
    fn did_change(
        &mut self,
        document: &LspDocument,
        changes: &[LspTextChange],
    ) {
        // Apply changes and convert to LSP format
        let content_changes =
            self.apply_change_and_convert(&document.uri, changes);
        if content_changes.is_empty() {
            return;
        }

        // Send the didChange notification
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

    /// Called when a document is saved in the editor.
    /// Sends the textDocument/didSave notification to the LSP server.
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

    /// Called when a document is closed in the editor.
    /// Sends the textDocument/didClose notification and removes local state.
    fn did_close(&mut self, document: &LspDocument) {
        // Remove the document from local state
        let mut docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
        docs.remove(&document.uri);

        // Send the didClose notification
        let msg = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": {
                "textDocument": { "uri": document.uri }
            }
        });
        self.send_message(&msg);
    }

    /// Requests hover information at a specific position.
    /// The response will be sent as an LspEvent::Hover via the events channel.
    fn request_hover(&mut self, document: &LspDocument, position: LspPosition) {
        // Get the document state and convert position to UTF-16
        let docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
        let Some(state) = docs.get(&document.uri) else { return };
        let pos = state.text.to_utf16_position(position);

        // Generate a request ID and track it
        let id = self.next_id();
        {
            let mut pending =
                self.pending_requests.lock().unwrap_or_else(|e| e.into_inner());
            pending.insert(id, LspRequestKind::Hover);
        }

        // Send the hover request
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

    /// Requests completion items at a specific position.
    /// The response will be sent as an LspEvent::Completion via the events channel.
    fn request_completion(
        &mut self,
        document: &LspDocument,
        position: LspPosition,
    ) {
        // Get the document state and convert position to UTF-16
        let docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
        let Some(state) = docs.get(&document.uri) else { return };
        let pos = state.text.to_utf16_position(position);

        // Generate a request ID and track it
        let id = self.next_id();
        {
            let mut pending =
                self.pending_requests.lock().unwrap_or_else(|e| e.into_inner());
            pending.insert(id, LspRequestKind::Completion);
        }

        // Send the completion request
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": document.uri },
                "position": { "line": pos.line, "character": pos.character },
                "context": { "triggerKind": 1 }  // Invoked = 1
            }
        });
        self.send_message(&msg);
    }
}
