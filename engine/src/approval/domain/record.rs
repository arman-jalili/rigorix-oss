//! ApprovalRecord — the durable, queryable, meaningful record of a human
//! decision, plus the DecisionContext (R4) and ApprovalStatus that shape it.
//!
//! @canonical .pi/architecture/modules/approval.md#approvalrecord
//! Implements: Contract Freeze — ApprovalRecord, DecisionContext, ApprovalStatus
//! Issue: #786 (approval epic — contract freeze); behavior in
//!   ISSUE-APPROVALRECORD (#789) and ISSUE-DECISIONCONTEXT (#790)
//!
//! # Contract (Frozen)
//! - `intent_hash` must match `canonical_bytes(tool ‖ intent ‖ declared_scope)`
//!   at record construction
//! - `status` transitions: `Pending → Consumed | Expired | Superseded`
//!   (single-use)
//! - `Superseded` triggers: (a) a re-plan replaces the sealed graph for a
//!   paused run (same dag_id, new graph — old approvals no longer authorize);
//!   (b) a newer approval for the same node replaces an older one
//!   (re-approval after `IntentMismatch` or expiry-then-reapproval); (c) the
//!   run is cancelled and re-executed with the same dag_id
//! - `Consumed` transitions on **terminal outcome** (success, skipped, or
//!   exhausted failure after ≥1 dispatch) — failed attempts stay `Pending` so
//!   legitimate retries re-verify; non-terminal interruptions keep it
//!   `Pending` for cross-process resume
//! - `expires_at` is enforced at verification time; expired approvals never
//!   dispatch
//! - `nonce` disambiguates legitimate retries from replays of consumed
//!   approvals (single-use semantics)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::ApprovalError;
use super::hash::IntentHash;

/// Lifecycle of a single-use approval record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    /// Approval granted, not yet consumed/expired/superseded. Survives
    /// non-terminal interruptions (cross-process resume) so the resumed run
    /// can verify and continue.
    Pending,
    /// Approval used — the node reached a terminal outcome after at least one
    /// dispatch. A consumed approval cannot be replayed.
    Consumed,
    /// Approval lapsed — `expires_at` passed before dispatch; never dispatches.
    Expired,
    /// Superseded by a re-plan, a newer approval, or a cancelled-and-reexecuted
    /// run with the same dag_id. Old approvals no longer authorize.
    Superseded,
}

/// What the human was shown at approval time — the rendered step, upstream
/// evidence, and state snapshot (R4, "the recorded why").
///
/// # Contract (Frozen)
/// - `rendered_step` and `summary` are always present
/// - `summary` is always envelope-safe (redacted) — it is what leaves the
///   local store in the signed envelope
/// - `full_payload` is **opt-in** (privacy pattern — follows
///   `planning_prompt`/audit conventions); when absent, consumers see only
///   `rendered_step` + `summary`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContext {
    /// Rendered step (command/args/scope — the canonical render, same source
    /// as the hashed intent).
    pub rendered_step: String,
    /// Upstream evidence (test results, plan excerpt, scoring results).
    pub upstream_evidence: Option<serde_json::Value>,
    /// State snapshot (git commit, branch, node states).
    pub state_snapshot: Option<serde_json::Value>,
    /// Redacted summary — always included in the envelope.
    pub summary: String,
    /// Full payload (opt-in, stored locally, never leaves in full by default).
    pub full_payload: Option<serde_json::Value>,
}

impl DecisionContext {
    /// Recursively redact values under sensitive field names.
    ///
    /// Reuses the observability `span_privacy` classification (`api_key`,
    /// `token`, `secret`, `password`, `authorization`, …) — one redaction
    /// policy across traces and approval summaries. The value under a
    /// sensitive key is replaced with `"<redacted>"` at any nesting depth.
    pub fn redact_value(value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let mut out = serde_json::Map::with_capacity(map.len());
                for (key, val) in map {
                    if crate::observability::span_privacy::is_sensitive_field(key) {
                        out.insert(key.clone(), serde_json::Value::String("<redacted>".into()));
                    } else {
                        out.insert(key.clone(), Self::redact_value(val));
                    }
                }
                serde_json::Value::Object(out)
            }
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(Self::redact_value).collect())
            }
            other => other.clone(),
        }
    }

    /// Build the envelope-safe summary deterministically.
    ///
    /// The summary is `rendered_step` plus the compact, **redacted** evidence
    /// and state snapshot (when present). The opt-in `full_payload` is never
    /// included — what leaves the local store is safe by construction.
    pub fn summarize(&self) -> String {
        let mut parts = Vec::new();
        parts.push(self.rendered_step.clone());
        if let Some(evidence) = &self.upstream_evidence {
            let redacted = Self::redact_value(evidence);
            parts.push(serde_json::to_string(&redacted).unwrap_or_else(|_| "<evidence>".into()));
        }
        if let Some(snapshot) = &self.state_snapshot {
            let redacted = Self::redact_value(snapshot);
            parts.push(serde_json::to_string(&redacted).unwrap_or_else(|_| "<snapshot>".into()));
        }
        parts.join(" | ")
    }
}

/// The durable record of a human decision, bound to an execution intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRecord {
    /// Name of the approved step.
    pub step_name: String,
    /// Node the approval authorizes (dispatch choke point key).
    pub node_id: Uuid,
    /// Digest binding the record to the exact dispatch payload.
    pub intent_hash: IntentHash,
    /// Canonical intent payload, as shown to the human at approval time.
    pub intent_payload: serde_json::Value,
    /// Identity subject of the approver (see identity module).
    pub approver_id: String,
    /// Role / policy id — a captured fact, not a judgment.
    pub authority: Option<String>,
    /// When the human approved.
    pub decided_at: DateTime<Utc>,
    /// TTL — the approval lapses and never dispatches after this instant.
    pub expires_at: DateTime<Utc>,
    /// Retry-vs-replay disambiguation (single-use semantics).
    pub nonce: Uuid,
    /// IdP token/claims used at approval time (credential substitution check).
    pub token_claims_ref: Option<String>,
    /// `Pending → Consumed | Expired | Superseded`.
    pub status: ApprovalStatus,
    /// What the human was shown (R4).
    pub decision_context: DecisionContext,
}

impl ApprovalRecord {
    /// Whether the approval is still pending (may be verified and dispatched).
    pub fn is_pending(&self) -> bool {
        matches!(self.status, ApprovalStatus::Pending)
    }

    /// Whether the approval has lapsed at (or before) the given instant.
    ///
    /// TTL is enforced at verification time — an expired approval never
    /// dispatches. Non-expiring decisions are expressed with a far-future
    /// `expires_at`; the comparison is inclusive (`now >= expires_at`).
    pub fn is_expired_at(&self, at: DateTime<Utc>) -> bool {
        at >= self.expires_at
    }

    /// Enforce the TTL at verification time.
    ///
    /// When `now` is at/past `expires_at` the record is transitioned to
    /// `Expired` and `ApprovalError::Expired` is returned — the caller must
    /// not dispatch. No-op for records that already left `Pending`.
    pub fn enforce_ttl(&mut self, now: DateTime<Utc>) -> Result<(), ApprovalError> {
        if !matches!(self.status, ApprovalStatus::Pending) {
            return Ok(());
        }
        if now >= self.expires_at {
            self.status = ApprovalStatus::Expired;
            return Err(ApprovalError::Expired(self.node_id));
        }
        Ok(())
    }

    /// R3 single-use — consume the approval on **terminal outcome** (success,
    /// skipped, or exhausted failure after ≥1 dispatch).
    ///
    /// Failed attempts do NOT consume — a legitimate retry re-verifies the
    /// same intent while the record stays `Pending`. A non-terminal
    /// interruption keeps `Pending` so a resumed run can verify and continue.
    ///
    /// # Errors
    /// - `ApprovalError::Expired` — the approval lapsed before dispatch
    /// - `ApprovalError::AlreadyConsumed` — replay of a consumed approval
    /// - `ApprovalError::InvalidState` — record was superseded
    pub fn consume(&mut self) -> Result<(), ApprovalError> {
        match self.status {
            ApprovalStatus::Pending => {
                self.status = ApprovalStatus::Consumed;
                Ok(())
            }
            ApprovalStatus::Consumed => Err(ApprovalError::AlreadyConsumed(self.node_id)),
            ApprovalStatus::Expired => Err(ApprovalError::Expired(self.node_id)),
            ApprovalStatus::Superseded => Err(ApprovalError::InvalidState(
                "cannot consume a superseded approval".into(),
            )),
        }
    }

    /// Transition a pending approval to `Expired` (e.g. TTL sweep).
    pub fn expire(&mut self) -> Result<(), ApprovalError> {
        match self.status {
            ApprovalStatus::Pending => {
                self.status = ApprovalStatus::Expired;
                Ok(())
            }
            ApprovalStatus::Consumed => Err(ApprovalError::AlreadyConsumed(self.node_id)),
            ApprovalStatus::Expired => Err(ApprovalError::InvalidState(
                "approval already expired".into(),
            )),
            ApprovalStatus::Superseded => Err(ApprovalError::InvalidState(
                "cannot expire a superseded approval".into(),
            )),
        }
    }

    /// Invalidate a pending approval because it no longer authorizes:
    /// (a) a re-plan replaced the sealed graph for a paused run, (b) a newer
    /// approval for the same node replaced this one, or (c) the run was
    /// cancelled and re-executed with the same dag_id.
    pub fn supersede(&mut self) -> Result<(), ApprovalError> {
        match self.status {
            ApprovalStatus::Pending => {
                self.status = ApprovalStatus::Superseded;
                Ok(())
            }
            ApprovalStatus::Consumed => Err(ApprovalError::AlreadyConsumed(self.node_id)),
            ApprovalStatus::Expired => Err(ApprovalError::Expired(self.node_id)),
            ApprovalStatus::Superseded => Err(ApprovalError::InvalidState(
                "approval already superseded".into(),
            )),
        }
    }
}
