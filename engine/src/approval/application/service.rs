//! ApprovalService — the application service for the full approval lifecycle:
//! capture, verify, consume, report.
//!
//! @canonical .pi/architecture/modules/approval.md#approvalservice
//! Implements: Contract Freeze — ApprovalService trait + IntentVerification
//! Issue: #786 (approval epic — contract freeze); behavior in
//!   ISSUE-APPROVALSERVICE (#792)
//!
//! The single dispatch choke point (`run_dispatch_loop`, shared by
//! `execute_graph` and `resume_execution`) calls `verify_intent` before every
//! dispatch of an approved node:
//!
//! ```text
//! IntentVerification::Matched   → dispatch the tool; on terminal outcome, consume(node_id)
//! IntentVerification::Mismatched → HALT — node stays/becomes IntentMismatch, re-approval required
//! IntentVerification::Invalid    → HALT — expired or already consumed
//! ```
//!
//! # Contract (Frozen)
//! - `approve` (R1+R3): captures intent + identity at approval time, persists a
//!   single-use record, emits `ApprovalRecorded` into the audit stream
//! - `verify_intent` (R2): re-derives the current intent from the sealed graph
//!   and compares to the recorded hash — runs once per dispatch attempt
//!   (legitimate retries re-verify the same intent; replays against a mutated
//!   intent fail)
//! - `consume` (R3): single-use — happens once on terminal outcome; failed
//!   attempts do NOT consume; non-terminal interruptions keep `Pending` for
//!   cross-process resume
//! - `record_scope_violation` (R5): post-execution effect-scope evidence
//!   (non-blocking)
//! - `get_approval`: query the durable record (TUI, audit, debugging)
//! - Degradation is **fail-closed**: any approval-service error halts the node

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::approval::domain::{
    ApprovalError, ApprovalRecord, ApprovalStatus, ExecutionIntent, IntentHash, ScopeViolation,
};

use super::dto::{ApproveInput, ApproveOutput};

/// R2 verdict returned by `verify_intent` at the dispatch choke point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentVerification {
    /// Hash matches — safe to dispatch.
    Matched,
    /// Hash differs — HALT; re-approval required. Carries both digests for
    /// the audit trail / TUI.
    Mismatched {
        /// The digest recorded at approval time.
        expected: IntentHash,
        /// The digest re-derived from the current graph at dispatch time.
        actual: IntentHash,
    },
    /// Approval expired or already consumed — never dispatches.
    Invalid(ApprovalStatus),
}

/// Approval lifecycle service — capture, verify, consume, report.
#[async_trait]
pub trait ApprovalService: Send + Sync {
    /// R1+R3 — capture intent + identity at approval time and persist a
    /// single-use `ApprovalRecord`; emits `ApprovalRecorded`.
    ///
    /// # Errors
    /// - `ApprovalError::NotFound` — a requested step/node has no sealed node
    /// - `ApprovalError::InvalidState` — a step is not awaiting approval
    /// - `ApprovalError::Internal` — intent capture or persistence failed
    async fn approve(&self, input: ApproveInput) -> Result<ApproveOutput, ApprovalError>;

    /// R2 — re-derive the current intent from the sealed graph and compare it
    /// to the recorded hash. Runs once per dispatch attempt.
    ///
    /// # Contract
    /// - Matching intent → `Matched`
    /// - Diverged intent → `Mismatched` (HALT, re-approval required)
    /// - Expired/consumed/superseded → `Invalid`
    async fn verify_intent(&self, node_id: Uuid) -> Result<IntentVerification, ApprovalError>;

    /// R3 — single-use: consume the approval on terminal outcome (success,
    /// skipped, or exhausted failure after ≥1 dispatch).
    ///
    /// Failed attempts stay `Pending` so legitimate retries re-verify; a
    /// non-terminal interruption keeps `Pending` for cross-process resume.
    async fn consume(&self, node_id: Uuid) -> Result<(), ApprovalError>;

    /// R5 — record a post-execution scope violation into the envelope evidence
    /// (non-blocking, first-class).
    async fn record_scope_violation(&self, violation: ScopeViolation) -> Result<(), ApprovalError>;

    /// Query the durable record (for TUI, audit, debugging).
    async fn get_approval(&self, node_id: Uuid) -> Result<Option<ApprovalRecord>, ApprovalError>;
}

/// A node resolved to the canonical intent that will dispatch.
///
/// Implementation-level support type (NOT part of the frozen contract): the
/// `execution_engine` / `orchestrator` wires a `NodeIntentResolver` so the
/// service can hash exactly what the dispatch choke point will execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedNode {
    /// Node id (dispatch choke point key).
    pub node_id: Uuid,
    /// Human step name (approval surface key).
    pub step_name: String,
    /// The canonical execution intent that will dispatch.
    pub intent: ExecutionIntent,
}

/// Intent source for the service implementation.
///
/// Implementation-level support trait (NOT part of the frozen contract).
#[async_trait]
pub trait NodeIntentResolver: Send + Sync {
    /// Resolve the current intent for a step name (approval time, R1).
    async fn resolve_by_step_name(&self, step_name: &str) -> Option<ResolvedNode>;

    /// Re-derive the current intent for a node id (dispatch time, R2).
    async fn resolve_by_node_id(&self, node_id: Uuid) -> Option<ResolvedNode>;
}

/// Receiver for post-execution scope violations (R5 envelope evidence).
///
/// Implementation-level support trait (NOT part of the frozen contract): the
/// audit wiring implements it to flag `scope_violation` into the envelope.
#[async_trait]
pub trait ScopeViolationSink: Send + Sync {
    /// Record one non-blocking scope-violation evidence item.
    async fn record(&self, violation: &ScopeViolation);
}

/// No-op scope-violation sink (records nothing).
#[async_trait]
impl ScopeViolationSink for () {
    async fn record(&self, _violation: &ScopeViolation) {}
}
