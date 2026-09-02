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
