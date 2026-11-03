//! `RefusalDto` — the failure shape, on the wire.
//!
//! @canonical .pi/architecture/decisions/ADR-004-api-design.md
//! Implements: Contract Freeze — RefusalDto
//! Issue: #1294 (contract freeze)

use serde::{Deserialize, Serialize};

/// A refusal or a request rejection, as the host sees it.
///
/// # Contract (Frozen)
/// - `kind` is a stable machine label; `message` is business wording
/// - No backtrace, no source path, no key material, no parameter value (NFR-003)
/// - `port` is present only for a port failure; `evidence_ref` only when one exists
/// - The words `unauthenticated`, `not_verified` and `unverifiable` do not occur in
///   `message` (criterion 5, 12)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefusalDto {
    /// Machine label (`session_unparseable`, `port_unavailable`, `content_rejected`, …).
    pub kind: String,
    /// Business-worded message.
    pub message: String,
    /// The capability that failed, when the kind is a port failure.
    pub port: Option<String>,
    /// Evidence for the refusal, when the engine produced one.
    pub evidence_ref: Option<String>,
}
