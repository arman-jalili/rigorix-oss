//! Data Transfer Objects for the Approval Binding module.
//!
//! @canonical .pi/architecture/modules/approval.md#approveinput-approveoutput
//! Implements: Contract Freeze — ApproveInput / ApproveOutput DTO schemas
//! Issue: #786 (approval epic — contract freeze)
//!
//! Typed DTOs for the approval boundary — they replace the bare `step_names`
//! surface (`approve_node(step_names)`) with a consequence-bound contract
//! carrying identity, decision context (R4), and token binding (R3).
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API and persistence)
//! - Field names and types are frozen — implementation issues depend on them
//! - `approver_id` is required; `authority`/`token_claims_ref` are optional
//!   captured facts, not judgments
//! - `decision_context` is optional on input (degraded approvals carry
//!   `summary`-only contexts) and the raw approval payload is never part of
//!   the output DTO beyond the persisted `ApprovalRecord`s

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::approval::domain::{ApprovalRecord, DecisionContext};

/// Input for approving a set of steps of a DAG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApproveInput {
    /// The DAG whose nodes are being approved.
    pub dag_id: Uuid,
    /// Step names to approve (resolved against the sealed graph).
    pub step_names: Vec<String>,
    /// Required — human identity subject (see identity module).
    pub approver_id: String,
    /// Role / policy id — a captured fact, not a judgment.
    pub authority: Option<String>,
    /// R4 — what the human was shown at approval time (rendered step,
    /// upstream evidence, state snapshot). Optional on input.
    pub decision_context: Option<DecisionContext>,
    /// IdP token/claims presented at approval (credential-substitution check).
    pub token_claims_ref: Option<String>,
}

/// Output of the approval flow: which steps were approved, which were not
/// found, and which are still pending, plus the durable records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApproveOutput {
    /// The DAG these results refer to.
    pub dag_id: Uuid,
    /// Step names that were approved (bound to an intent hash).
    pub approved: Vec<String>,
    /// Step names that did not resolve to a sealed node.
    pub not_found: Vec<String>,
    /// Steps still awaiting approval (e.g. not yet in `AwaitingApproval`).
    pub still_pending: Vec<String>,
    /// The persisted single-use records (one per approved node).
    pub approval_records: Vec<ApprovalRecord>,
}
