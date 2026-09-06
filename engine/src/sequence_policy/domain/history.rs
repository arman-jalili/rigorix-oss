//! HistoryAction — one executed action recovered from the signed audit trail.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#r7
//! Issue: #871 (R7 cross-run conflicting-action rules)
//!
//! R7 rules consult actions that COMPLETED in PRIOR runs (the cross-run case:
//! "remove X" in run 1, minutes later "add Jeff" in run 2 — each within-run
//! gate passes on its own). The durable source is the signed envelope store
//! (`.rigorix/audit`, LocalAuditEnvelopeRepository) — policy reads signed
//! evidence, and tampering with the trail to evade a rule breaks the HMAC.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One executed action in a prior, completed run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryAction {
    /// The executed node's name (stable template identifier). Parameter
    /// values are never recovered — the trail is pre-redacted (SpanPrivacy).
    pub node: String,
    /// Principal that triggered the prior run (envelope `author`, falling
    /// back to the attested `identity.subject` when present). `None` when the
    /// prior run recorded neither.
    pub principal: Option<String>,
    /// When the action completed (envelope / event timestamp).
    pub at: DateTime<Utc>,
}
