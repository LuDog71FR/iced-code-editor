//! Tracking for LSP requests awaiting a response.
//!
//! Requests are fire-and-forget on the wire, so the client remembers what it
//! asked for in order to route the eventual reply. A server that never answers
//! must not be able to grow that map without bound, which is what
//! [`evict_expired_requests`] is for.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Enumeration of LSP request types that we track for response handling.
pub(super) enum LspRequestKind {
    /// Hover request — shows type information and documentation
    Hover,
    /// Completion request — provides auto-complete suggestions
    Completion,
    /// Definition request — go to definition
    Definition,
    /// Formatting request — reformats the whole document
    Formatting,
}

/// A request awaiting a server response, tracked with the time it was sent.
pub(super) struct PendingRequest {
    /// Which kind of request this is, used to route the eventual response.
    pub(super) kind: LspRequestKind,
    /// URI of the document the request was made against.
    ///
    /// A JSON-RPC response carries only the request id, so the document has
    /// to be remembered here: replies whose payload is in document
    /// coordinates (formatting edits) can only be translated back to
    /// character offsets against the right document mirror.
    pub(super) uri: String,
    /// When the request was sent, used by [`evict_expired_requests`] to
    /// drop it if the server never responds.
    pub(super) requested_at: Instant,
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
pub(super) fn evict_expired_requests(
    pending: &mut HashMap<u64, PendingRequest>,
) {
    let now = Instant::now();
    pending.retain(|_, entry| {
        now.saturating_duration_since(entry.requested_at)
            < PENDING_REQUEST_TIMEOUT
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a [`PendingRequest`] of `kind`, sent "now" for test purposes.
    fn pending_request(kind: LspRequestKind) -> PendingRequest {
        PendingRequest {
            kind,
            uri: "file:///test.rs".to_string(),
            requested_at: Instant::now(),
        }
    }

    #[test]
    fn test_evict_expired_requests_drops_only_stale_entries() {
        let mut pending = HashMap::new();
        pending.insert(
            1u64,
            PendingRequest {
                kind: LspRequestKind::Hover,
                uri: "file:///test.rs".to_string(),
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
}
