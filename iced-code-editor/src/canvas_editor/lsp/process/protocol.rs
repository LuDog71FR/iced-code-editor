//! Wire format for the LSP stdio transport.
//!
//! Everything between the raw byte stream and the editor-facing
//! [`LspEvent`](super::LspEvent): `Content-Length` framing, the bounded reads
//! that keep a malformed or hostile server from exhausting memory, dispatch of
//! incoming JSON-RPC messages, and parsing of the three response shapes the
//! editor asks for.
//!
//! These are free functions with no client state: the reader thread owns the
//! stream and calls into here, so nothing in this module needs to know the
//! process exists.

use std::collections::HashMap;
use std::io::{BufRead, Read};
use std::sync::{Arc, Mutex, mpsc};

use serde_json::json;

use super::pending::{LspRequestKind, PendingRequest};
use super::text_model::DocumentState;
use super::{LspEvent, LspPosition, LspRange};
use crate::canvas_editor::lsp::LspTextChange;

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
/// Maximum accepted length for a single header line (8 KiB).
///
/// `BufRead::read_line` grows its buffer until it finds a newline, so a server
/// that opens a header line and never terminates it would force an unbounded
/// allocation — the same hazard [`MAX_MESSAGE_BYTES`] guards on the body, on
/// the path that reaches it. Real LSP headers are a few dozen bytes.
const MAX_HEADER_LINE_BYTES: u64 = 8 * 1024;
/// Maximum accepted number of header lines before the blank separator.
///
/// Each line is individually bounded by [`MAX_HEADER_LINE_BYTES`], so an
/// endless stream of well-formed headers is not a memory hazard — but without
/// a cap the reader would spin on it forever and never yield a message. Real
/// LSP frames send one or two headers.
const MAX_HEADER_LINES: usize = 64;
/// Maximum bytes kept from a single language-server stderr line (8 KiB).
///
/// `BufRead::lines` grows its buffer until it finds a newline, so one
/// unterminated line from a chatty or hostile server would grow without bound
/// — the same hazard [`MAX_HEADER_LINE_BYTES`] guards on the protocol stream.
/// Diagnostics are free-form text rather than framed protocol, so an oversized
/// line is truncated and reading resumes at the next line, instead of tearing
/// down the stream the way an unparseable frame must.
const MAX_LOG_LINE_BYTES: u64 = 8 * 1024;
/// Maximum bytes discarded while resynchronising after an oversized log line.
///
/// Bounds the *time* spent skipping, where [`MAX_LOG_LINE_BYTES`] bounds the
/// memory kept: a line that never ends has no newline to find, so an
/// unbudgeted skip would never return. See [`skip_to_newline`].
const MAX_LOG_SKIP_BYTES: u64 = 1024 * 1024;

/// Reads one `Content-Length`-framed message body from `reader`.
///
/// Returns `None` when the stream ends, when the body cannot be read in full,
/// when the announced frame exceeds [`MAX_MESSAGE_BYTES`], or when the header
/// block breaches [`MAX_HEADER_LINE_BYTES`] or [`MAX_HEADER_LINES`]. None of
/// these can be skipped reliably — the framing is exactly what is not
/// trustworthy — so the caller must stop reading the stream rather than try to
/// resynchronise mid-message.
///
/// Header lines other than `Content-Length` are ignored. A header block
/// carrying no `Content-Length` yields an empty body, which the caller then
/// discards as invalid JSON.
pub(super) fn read_message(reader: &mut impl BufRead) -> Option<Vec<u8>> {
    let mut content_length: Option<usize> = None;
    let mut line = String::new();

    for _ in 0..MAX_HEADER_LINES {
        line.clear();
        // Read through a `take` rather than calling `read_line` directly:
        // `read_line` grows `line` until it finds a newline, so a server that
        // never sends one would force an unbounded allocation here — the same
        // hazard `MAX_MESSAGE_BYTES` guards on the body, one step earlier.
        // Called through UFCS so `Self` is `&mut R`: written as a method call,
        // the probe derefs to `R` and `take` moves the reader out from under
        // the borrow.
        let read = Read::take(&mut *reader, MAX_HEADER_LINE_BYTES)
            .read_line(&mut line)
            .ok()?;

        // A zero-length read means end of stream, not an empty header line.
        if read == 0 {
            return None;
        }

        // Hitting the cap with no newline means the line was oversized. The
        // remainder is still queued in the stream with no way to tell where
        // the next frame begins, so give up instead of resynchronising.
        if !line.ends_with('\n') {
            return None;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            return read_body(reader, content_length);
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:")
            && let Ok(len) = value.trim().parse::<usize>()
        {
            content_length = Some(len);
        }
    }

    // The header block never terminated. Each line is individually bounded, so
    // this is not a memory hazard, but without a cap the loop would spin on a
    // server emitting headers forever and never yield a message.
    None
}

/// Reads one line of language-server stderr, truncated to
/// [`MAX_LOG_LINE_BYTES`].
///
/// Returns `None` at end of stream. A line longer than the cap is cut at the
/// cap, marked with a trailing `…`, and the remainder discarded (without being
/// buffered) up to the next newline, so the following lines still start on a
/// real boundary.
///
/// Invalid UTF-8 is replaced rather than treated as an error: server
/// diagnostics are not worth tearing the log stream down over.
pub(super) fn read_log_line(reader: &mut impl BufRead) -> Option<String> {
    let mut buf = Vec::new();
    // Bounded like the protocol header lines, and for the same reason; see
    // `read_message`.
    let read = Read::take(&mut *reader, MAX_LOG_LINE_BYTES)
        .read_until(b'\n', &mut buf)
        .ok()?;
    if read == 0 {
        return None;
    }

    let truncated = !buf.ends_with(b"\n");
    let mut line = String::from_utf8_lossy(&buf).into_owned();
    if truncated {
        skip_to_newline(reader);
        line.push('…');
    }
    Some(line)
}

/// Advances `reader` past the next newline, discarding at most
/// [`MAX_LOG_SKIP_BYTES`] on the way.
///
/// Used to resynchronise after an oversized log line. Works through
/// `fill_buf`/`consume` rather than `read_until` so the skipped bytes are
/// never buffered — otherwise recovering from an unbounded line would itself
/// need unbounded memory.
///
/// The byte budget matters as much as the buffering: a server emitting a line
/// that never ends has no newline to find, and an unbudgeted skip would spin
/// in this loop for as long as it kept talking. Giving up instead lets the
/// next [`read_log_line`] surface the following chunk as its own truncated
/// line, so the reader always makes forward progress.
fn skip_to_newline(reader: &mut impl BufRead) {
    let mut skipped: u64 = 0;
    while skipped < MAX_LOG_SKIP_BYTES {
        let Ok(available) = reader.fill_buf() else { return };
        if available.is_empty() {
            return;
        }
        if let Some(index) = available.iter().position(|byte| *byte == b'\n') {
            reader.consume(index + 1);
            return;
        }
        let len = available.len();
        reader.consume(len);
        skipped = skipped.saturating_add(len as u64);
    }
}

/// Reads the body announced by a header block's `Content-Length`.
///
/// A missing `Content-Length` is treated as a zero-length body; see
/// [`read_message`].
fn read_body(
    reader: &mut impl BufRead,
    content_length: Option<usize>,
) -> Option<Vec<u8>> {
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
/// ```text
/// let framed = frame_message(&serde_json::json!({"jsonrpc": "2.0"})).unwrap();
/// assert!(framed.starts_with(b"Content-Length: "));
/// ```
pub(super) fn frame_message(value: &serde_json::Value) -> Option<Vec<u8>> {
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
pub(super) fn handle_server_request(
    id: u64,
    method: &str,
    tx: &mpsc::Sender<Vec<u8>>,
) {
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
/// [`LspEvent::Hover`], [`LspEvent::Completion`], [`LspEvent::Definition`] or
/// [`LspEvent::Formatting`].
///
/// `documents` is the client's mirror of every open document, needed to turn
/// the UTF-16 columns of a formatting reply back into the character offsets
/// the editor addresses text by.
pub(super) fn handle_client_response(
    id: u64,
    value: &serde_json::Value,
    pending: &Arc<Mutex<HashMap<u64, PendingRequest>>>,
    events: &mpsc::SyncSender<LspEvent>,
    documents: &Arc<Mutex<HashMap<String, DocumentState>>>,
) {
    let request = {
        let mut map = pending.lock().unwrap_or_else(|e| e.into_inner());
        map.remove(&id)
    };

    let Some(request) = request else { return };
    let result = value.get("result").unwrap_or(&serde_json::Value::Null);

    match request.kind {
        LspRequestKind::Hover => {
            let text = parse_hover_text(result).unwrap_or_default();
            super::emit(events, LspEvent::Hover { text });
        }
        LspRequestKind::Completion => {
            let items = parse_completion_items(result);
            if !items.is_empty() {
                super::emit(events, LspEvent::Completion { items });
            }
        }
        LspRequestKind::Definition => {
            if let Some((uri, range)) = parse_definition_location(result) {
                super::emit(events, LspEvent::Definition { uri, range });
            }
        }
        LspRequestKind::Formatting => {
            let edits = parse_text_edits(result);
            if edits.is_empty() {
                return;
            }
            let docs = documents.lock().unwrap_or_else(|e| e.into_inner());
            // Without the mirror there is no way to place the edits, and
            // applying UTF-16 columns as character offsets would corrupt any
            // line holding a non-ASCII character. Drop the reply instead.
            let Some(state) = docs.get(&request.uri) else { return };
            let edits = edits
                .into_iter()
                .map(|edit| LspTextChange {
                    range: LspRange {
                        start: state.text.to_char_position(edit.range.start),
                        end: state.text.to_char_position(edit.range.end),
                    },
                    text: edit.text,
                })
                .collect();
            drop(docs);
            super::emit(
                events,
                LspEvent::Formatting { uri: request.uri, edits },
            );
        }
    }
}

/// Handles a server-initiated notification (e.g. `$/progress`).
///
/// Parses the progress payload and emits a [`LspEvent::Progress`].
/// Notifications for unknown methods are silently ignored.
pub(super) fn handle_server_notification(
    method: &str,
    params: &serde_json::Value,
    events: &mpsc::SyncSender<LspEvent>,
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

    super::emit(
        events,
        LspEvent::Progress {
            token,
            server_key: server_key.to_string(),
            title,
            message,
            percentage,
            done,
        },
    );
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

/// Parses a `TextEdit[]` reply, as returned by `textDocument/formatting`.
///
/// Positions stay in the server's UTF-16 coordinates: converting them needs
/// the document mirror, which the caller holds. A `null` result — what a
/// server sends for a document it has nothing to change — yields no edits.
fn parse_text_edits(result: &serde_json::Value) -> Vec<LspTextChange> {
    let Some(array) = result.as_array() else {
        return Vec::new();
    };

    array
        .iter()
        .filter_map(|edit| {
            Some(LspTextChange {
                range: extract_range(edit.get("range")?)?,
                text: edit.get("newText")?.as_str()?.to_string(),
            })
        })
        .collect()
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

#[cfg(test)]
mod tests {
    // Several helpers and tests below carry an `#[allow]` for `expect_used`,
    // `panic` or `unwrap_used`. In test code a panic *is* the failure report:
    // `expect("expected a Hover event")` names the broken expectation far more
    // precisely than an `assert!(false)` workaround would. The workspace denies
    // these lints to protect production code, not tests — this mirrors the
    // existing per-test allows in `update.rs` and `selection.rs`.
    use super::*;
    use crate::canvas_editor::lsp::LspPosition;
    use std::io::BufReader;
    use std::time::Instant;

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
        PendingRequest {
            kind,
            uri: "file:///test.rs".to_string(),
            requested_at: Instant::now(),
        }
    }

    /// An empty document map, for the responses that never consult the mirror.
    fn no_documents() -> Arc<Mutex<HashMap<String, DocumentState>>> {
        Arc::new(Mutex::new(HashMap::new()))
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

    /// A reader that serves an endless run of `a` with no newline ever,
    /// standing in for a server that opens a header line and never closes it.
    ///
    /// It counts what it served, which is what makes the bound observable: a
    /// finite `Cursor` would end the line at EOF and so pass even with no cap
    /// at all.
    struct EndlessHeaderReader {
        served: std::rc::Rc<std::cell::Cell<usize>>,
    }

    impl Read for EndlessHeaderReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            buf.fill(b'a');
            self.served.set(self.served.get() + buf.len());
            Ok(buf.len())
        }
    }

    #[test]
    fn test_read_message_stops_reading_an_unterminated_header_line() {
        let served = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut stream = BufReader::new(EndlessHeaderReader {
            served: std::rc::Rc::clone(&served),
        });

        assert!(read_message(&mut stream).is_none());

        // The real property: the reader gave up near the cap instead of
        // growing its buffer for as long as the server kept talking. Without
        // the cap this test would not fail, it would never return.
        let cap = MAX_HEADER_LINE_BYTES as usize;
        assert!(
            served.get() <= cap * 4,
            "read {} bytes for one header line, cap is {cap}",
            served.get()
        );
    }

    #[test]
    fn test_read_message_rejects_a_header_block_that_never_ends() {
        // Well-formed headers, but no blank separator line ever arrives. Each
        // line is individually bounded, so this is a livelock rather than a
        // memory hazard — the line cap alone would not stop it.
        let flood = "X-Pad: 1\r\n".repeat(MAX_HEADER_LINES + 10);
        let mut stream = std::io::Cursor::new(flood.into_bytes());

        assert!(read_message(&mut stream).is_none());
    }

    #[test]
    fn test_read_log_line_reads_lines_in_order() {
        let mut reader =
            BufReader::new(std::io::Cursor::new(b"first\nsecond\n".to_vec()));

        assert_eq!(read_log_line(&mut reader).as_deref(), Some("first\n"));
        assert_eq!(read_log_line(&mut reader).as_deref(), Some("second\n"));
        assert_eq!(read_log_line(&mut reader), None, "stream is exhausted");
    }

    #[test]
    fn test_read_log_line_truncates_an_oversized_line_and_resyncs() {
        let cap = MAX_LOG_LINE_BYTES as usize;
        let flood = "a".repeat(cap * 3);
        let stream = format!("{flood}\nnext\n");
        let mut reader =
            BufReader::new(std::io::Cursor::new(stream.into_bytes()));

        let line = read_log_line(&mut reader);
        assert!(
            line.as_ref().is_some_and(|line| line.len() <= cap + 8),
            "the kept prefix must be bounded by the cap"
        );
        assert!(
            line.as_ref().is_some_and(|line| line.ends_with('…')),
            "a truncated line must say so"
        );

        // The remainder was skipped rather than buffered, so the next line
        // still starts on a real boundary instead of mid-flood.
        assert_eq!(read_log_line(&mut reader).as_deref(), Some("next\n"));
    }

    #[test]
    fn test_read_log_line_replaces_invalid_utf8_instead_of_ending_the_stream() {
        // Server diagnostics are not worth tearing the log stream down over:
        // a bad byte must not cost us every later line.
        let mut reader = BufReader::new(std::io::Cursor::new(
            b"bad \xff byte\nnext\n".to_vec(),
        ));

        let line = read_log_line(&mut reader);
        assert!(
            line.as_ref().is_some_and(|line| line.contains('\u{fffd}')),
            "the invalid byte should become a replacement character, got {line:?}"
        );
        assert_eq!(read_log_line(&mut reader).as_deref(), Some("next\n"));
    }

    #[test]
    fn test_read_log_line_stops_reading_an_unterminated_line() {
        let served = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut reader = BufReader::new(EndlessHeaderReader {
            served: std::rc::Rc::clone(&served),
        });

        // The line never ends, so the skip-ahead never finds a newline either;
        // both must stay bounded rather than buffering the whole flood.
        let line = read_log_line(&mut reader);
        let cap = MAX_LOG_LINE_BYTES as usize;
        assert!(line.is_some_and(|line| line.len() <= cap + 8));
    }

    #[test]
    fn test_read_message_accepts_headers_just_inside_the_caps() {
        // The caps must not reject legitimate traffic: a long-but-bounded
        // header line, and more headers than a real server sends but fewer
        // than the limit, still produce the framed body.
        let padding = "x".repeat(MAX_HEADER_LINE_BYTES as usize / 2);
        let mut frame = format!("X-Pad: {padding}\r\n");
        for _ in 0..MAX_HEADER_LINES - 3 {
            frame.push_str("X-Small: 1\r\n");
        }
        frame.push_str("Content-Length: 2\r\n\r\n{}");
        let mut stream = std::io::Cursor::new(frame.into_bytes());

        assert_eq!(read_message(&mut stream).as_deref(), Some(&b"{}"[..]));
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
        let (events_tx, events_rx) = mpsc::sync_channel::<LspEvent>(16);
        let pending = Arc::new(Mutex::new(HashMap::new()));
        pending
            .lock()
            .unwrap()
            .insert(1u64, pending_request(LspRequestKind::Hover));

        let value = serde_json::json!({
            "id": 1,
            "result": { "contents": { "value": "hover info" } }
        });
        handle_client_response(
            1,
            &value,
            &pending,
            &events_tx,
            &no_documents(),
        );

        match events_rx.try_recv().expect("expected a Hover event") {
            LspEvent::Hover { text } => assert_eq!(text, "hover info"),
            _ => panic!("expected LspEvent::Hover"),
        }
        assert!(pending.lock().unwrap().is_empty());
    }

    #[test]
    #[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    fn test_handle_client_response_completion() {
        let (events_tx, events_rx) = mpsc::sync_channel::<LspEvent>(16);
        let pending = Arc::new(Mutex::new(HashMap::new()));
        pending
            .lock()
            .unwrap()
            .insert(2u64, pending_request(LspRequestKind::Completion));

        let value = serde_json::json!({
            "id": 2,
            "result": { "items": [{ "label": "foo" }, { "label": "bar" }] }
        });
        handle_client_response(
            2,
            &value,
            &pending,
            &events_tx,
            &no_documents(),
        );

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
        let (events_tx, events_rx) = mpsc::sync_channel::<LspEvent>(16);
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
        handle_client_response(
            3,
            &value,
            &pending,
            &events_tx,
            &no_documents(),
        );

        match events_rx.try_recv().expect("expected a Definition event") {
            LspEvent::Definition { uri, .. } => {
                assert_eq!(uri, "file:///foo/bar.rs");
            }
            _ => panic!("expected LspEvent::Definition"),
        }
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_a_dropped_response_still_clears_its_pending_request() {
        // The claim that makes dropping a full queue safe (see `super::emit`):
        // the pending entry goes before the event is emitted, so an event the
        // queue refuses cannot leave a request in flight forever. A rendezvous
        // channel with no receiver waiting is permanently full, which is the
        // cheapest way to stand in for a saturated queue.
        let (events_tx, _events_rx) = mpsc::sync_channel::<LspEvent>(0);
        let pending = Arc::new(Mutex::new(HashMap::new()));
        pending
            .lock()
            .unwrap()
            .insert(7u64, pending_request(LspRequestKind::Hover));

        let value = serde_json::json!({
            "id": 7,
            "result": { "contents": "fn main()" }
        });
        handle_client_response(
            7,
            &value,
            &pending,
            &events_tx,
            &no_documents(),
        );

        assert!(
            pending.lock().unwrap().is_empty(),
            "a response the queue dropped must still retire its request"
        );
    }

    #[test]
    fn test_handle_client_response_unknown_id_ignored() {
        let (events_tx, events_rx) = mpsc::sync_channel::<LspEvent>(16);
        let pending = Arc::new(Mutex::new(HashMap::new()));

        let value = serde_json::json!({ "id": 99, "result": null });
        handle_client_response(
            99,
            &value,
            &pending,
            &events_tx,
            &no_documents(),
        );
        assert!(
            events_rx.try_recv().is_err(),
            "unknown IDs must not emit events"
        );
    }

    // -------------------------------------------------------------------------
    // formatting responses
    // -------------------------------------------------------------------------

    /// A document map holding `text` under `file:///test.rs`.
    fn documents_with(
        text: &str,
    ) -> Arc<Mutex<HashMap<String, DocumentState>>> {
        let mut map = HashMap::new();
        map.insert(
            "file:///test.rs".to_string(),
            DocumentState {
                text: super::super::text_model::TextModel::from_text(text),
            },
        );
        Arc::new(Mutex::new(map))
    }

    #[test]
    fn test_parse_text_edits_reads_every_edit() {
        let result = serde_json::json!([
            {
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 1, "character": 0 }
                },
                "newText": "fn main() {}\n"
            },
            {
                "range": {
                    "start": { "line": 4, "character": 2 },
                    "end": { "line": 4, "character": 6 }
                },
                "newText": ""
            }
        ]);

        let edits = parse_text_edits(&result);
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].text, "fn main() {}\n");
        assert_eq!(edits[0].range.end, LspPosition { line: 1, character: 0 });
        assert_eq!(edits[1].text, "");
    }

    #[test]
    fn test_parse_text_edits_of_a_null_result_is_empty() {
        // What a server sends when it has nothing to reformat.
        assert!(parse_text_edits(&serde_json::Value::Null).is_empty());
    }

    #[test]
    fn test_parse_text_edits_skips_malformed_entries() {
        let result = serde_json::json!([
            { "newText": "x" },
            {
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 1 }
                }
            }
        ]);
        assert!(parse_text_edits(&result).is_empty());
    }

    #[test]
    #[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    fn test_handle_client_response_formatting_converts_utf16_columns() {
        let (events_tx, events_rx) = mpsc::sync_channel::<LspEvent>(16);
        let pending = Arc::new(Mutex::new(HashMap::new()));
        pending
            .lock()
            .unwrap()
            .insert(4u64, pending_request(LspRequestKind::Formatting));

        // The rocket is two UTF-16 units but one character, so the server's
        // column 3 is the editor's column 2.
        let value = serde_json::json!({
            "id": 4,
            "result": [{
                "range": {
                    "start": { "line": 0, "character": 3 },
                    "end": { "line": 0, "character": 4 }
                },
                "newText": "Z"
            }]
        });
        handle_client_response(
            4,
            &value,
            &pending,
            &events_tx,
            &documents_with("🚀ab"),
        );

        match events_rx.try_recv().expect("expected a Formatting event") {
            LspEvent::Formatting { uri, edits } => {
                assert_eq!(uri, "file:///test.rs");
                assert_eq!(edits.len(), 1);
                assert_eq!(
                    edits[0].range.start,
                    LspPosition { line: 0, character: 2 }
                );
                assert_eq!(
                    edits[0].range.end,
                    LspPosition { line: 0, character: 3 }
                );
            }
            _ => panic!("expected LspEvent::Formatting"),
        }
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_handle_client_response_formatting_without_a_mirror_emits_nothing() {
        // Placing the edits needs the mirror; guessing would corrupt the text.
        let (events_tx, events_rx) = mpsc::sync_channel::<LspEvent>(16);
        let pending = Arc::new(Mutex::new(HashMap::new()));
        pending
            .lock()
            .unwrap()
            .insert(5u64, pending_request(LspRequestKind::Formatting));

        let value = serde_json::json!({
            "id": 5,
            "result": [{
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 1 }
                },
                "newText": "x"
            }]
        });
        handle_client_response(
            5,
            &value,
            &pending,
            &events_tx,
            &no_documents(),
        );

        assert!(events_rx.try_recv().is_err());
        assert!(pending.lock().unwrap().is_empty());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_handle_client_response_formatting_of_an_empty_reply_emits_nothing()
    {
        let (events_tx, events_rx) = mpsc::sync_channel::<LspEvent>(16);
        let pending = Arc::new(Mutex::new(HashMap::new()));
        pending
            .lock()
            .unwrap()
            .insert(6u64, pending_request(LspRequestKind::Formatting));

        let value = serde_json::json!({ "id": 6, "result": null });
        handle_client_response(
            6,
            &value,
            &pending,
            &events_tx,
            &documents_with("fn main() {}"),
        );

        assert!(events_rx.try_recv().is_err());
    }

    // -------------------------------------------------------------------------
    // handle_server_notification
    // -------------------------------------------------------------------------

    #[test]
    #[allow(clippy::expect_used, clippy::panic)]
    fn test_handle_server_notification_progress_done() {
        let (events_tx, events_rx) = mpsc::sync_channel::<LspEvent>(16);
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
        let (events_tx, events_rx) = mpsc::sync_channel::<LspEvent>(16);
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
        let (events_tx, events_rx) = mpsc::sync_channel::<LspEvent>(16);
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
