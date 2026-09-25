//! ADR-016 anchor contract — the **projection** the guard consumes.
//!
//! @canonical .pi/architecture/decisions/ADR-016-audit-integrity-anchor.md
//! Issue: #899 (OSS-AUDIT-C, Phase C)
//!
//! The anchor (enterprise Execution API) signs a narrow, verifiable projection
//! of its ledger — "prior actions in window W for scope S" — with an **Ed25519**
//! key. The host verifies it with the anchor's **public** key and only then
//! hands the actions to the matcher. This module is the frozen wire contract
//! (a struct copy of `rigorix-verifier`'s `HistorySlice`, per the SDK
//! contract-freeze): no network, no policy, pure data.
//!
//! ## Canonical signing form
//! The anchor signs the compact JSON of the artifact with `sig` set to `null`
//! (`serde_json::to_vec`, struct field order). The field order and `serde`
//! attributes here MUST mirror the enterprise signer byte-for-byte, or every
//! signature fails to verify.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One prior executed action in an anchor-signed history slice.
///
/// Mirrors `rigorix-verifier` / enterprise `HistoryActionRef`. Field order and
/// `effect_key`'s `skip_serializing_if` are part of the signed bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryActionRef {
    /// Executed node's stable name.
    pub node: String,
    /// Principal that triggered the prior run, when recorded.
    pub principal: Option<String>,
    /// When the action completed.
    pub at: DateTime<Utc>,
    /// R8 / ADR-014 effect key (one-way hash), when the run was effect-bearing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_key: Option<String>,
}

/// ADR-016 anchor-signed history projection the guard consumes in `anchored`
/// mode. Mirrors the enterprise / verifier wire struct exactly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistorySlice {
    /// Scope the projection covers (must equal the runtime's configured scope).
    pub scope: String,
    /// Lower bound (inclusive) of the requested window.
    pub since: DateTime<Utc>,
    /// Prior actions in the window (newest first, anchor-ordered).
    pub actions: Vec<HistoryActionRef>,
    /// Verified ledger head this projection was read at.
    pub head_hash: String,
    /// Hex-encoded Ed25519 signature over [`Self::signing_bytes`].
    pub sig: Option<String>,
}

impl HistorySlice {
    /// Canonical bytes the anchor signs — compact JSON with `sig` nulled.
    ///
    /// # Errors
    /// Serialization failure (unreachable for this data-only struct).
    pub fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        let mut unsigned = self.clone();
        unsigned.sig = None;
        serde_json::to_vec(&unsigned).map_err(|e| format!("history slice signing bytes: {e}"))
    }
}

/// ADR-016 deployment mode for history evidence.
///
/// - [`AnchorMode::LocalUnanchored`] — no anchor configured (OSS default): the
///   cache is tamper-evident; deny-class cross-run rules refuse unless the
///   operator records `allow_unanchored`.
/// - [`AnchorMode::Anchored`] — an anchor is configured; history reads are
///   anchor-signed and a consequential run fails closed when the anchor is
///   unreachable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorMode {
    /// Local cache only (Phase A behavior).
    LocalUnanchored,
    /// Anchor-authenticated evidence (Phase C).
    Anchored,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn signing_bytes_null_the_signature() {
        let slice = HistorySlice {
            scope: "local".into(),
            since: Utc.timestamp_opt(0, 0).unwrap(),
            actions: vec![HistoryActionRef {
                node: "remove".into(),
                principal: Some("alice".into()),
                at: Utc.timestamp_opt(1, 0).unwrap(),
                effect_key: None,
            }],
            head_hash: "ab".repeat(32),
            sig: Some("deadbeef".into()),
        };
        let unsigned = HistorySlice {
            sig: None,
            ..slice.clone()
        };
        assert_eq!(
            slice.signing_bytes().unwrap(),
            serde_json::to_vec(&unsigned).unwrap()
        );
    }
}
