//! SequencePolicyService — the application service for sequence-policy
//! evaluation: plan-time (R2) and run-time prefix (R3).
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#sequencepolicyservice
//! Implements: Contract Freeze — SequencePolicyService trait
//! Issue: #838 (sequence-policy epic — contract freeze); behavior in
//!   ISSUE-SEQUENCE-POLICY-2 (SequencePolicyService) and
//!   ISSUE-SEQUENCE-POLICY-5 (Matcher)
//!
//! Two evaluation points, same deterministic matching core:
//!
//! ```text
//! R2  evaluate_plan(ordered_steps)      → Vec<SequenceMatch>   (plan-time, pre-side-effect)
//! R3  evaluate_prefix(prefix, next)     → Vec<SequenceMatch>   (dispatch boundary, dynamic plans)
//! ```
//!
//! The consumers branch on `SequenceMatch.action`:
//! - `RuleAction::Promote` → later step is built / routed
//!   `requires_approval = true` (existing approval pause/resume chain)
//! - `RuleAction::Deny` → later step fails before dispatch with a structured
//!   `SequencePolicyDenied` node failure (tool never called)
//!
//! # Contract (Frozen)
//! - All service methods are async and trait-object safe (`Send + Sync`,
//!   via `async-trait`)
//! - **Evaluation is fail-closed**: an evaluation error (`SequencePolicyError`)
//!   halts the run / refuses the plan — never a silent pass-through to
//!   dispatch (mirrors ADR-011 fail-closed verification)
//! - A missing optional rule config is **not** an evaluation error — it yields
//!   no matches (fail-open-absent)
//! - The rule set is read per-run from disk through
//!   `SequencePolicyRepository` (R5: admin-authored, never agent-supplied)
//! - Matching is deterministic over the serialized step data (R4)

use async_trait::async_trait;

use crate::sequence_policy::domain::{SequenceMatch, SequencePolicyError};

use super::dto::{DispatchedStep, PlannedStep};

/// Sequence-policy evaluation service.
#[async_trait]
pub trait SequencePolicyService: Send + Sync {
    /// Evaluate a fully-materialized ordered step list (plan-time, R2).
    ///
    /// Called **before** `build_graph_from_steps` seals the graph. A matched
    /// `promote` rule means the later matched step must be built with
    /// `requires_approval = true`; a matched `deny` rule means it must fail
    /// before dispatch.
    ///
    /// `principal` is the current run's initiating identity (envelope
    /// `author` / attested claim subject). R7 rules with
    /// `history.same_principal = true` match only prior actions by this
    /// principal; `None` never yields a same-principal match (no false
    /// denial).
    ///
    /// # Errors
    /// - `SequencePolicyError::InvalidConfig` / `RuleExceedsCaps` — rule
    ///   config corrupt or over-cap → **fail closed**, plan refused, no steps
    ///   execute
    /// - `SequencePolicyError::Internal` — evaluation (incl. history read)
    ///   failed unexpectedly → also fail closed
    async fn evaluate_plan(
        &self,
        steps: &[PlannedStep],
        principal: Option<&str>,
    ) -> Result<Vec<SequenceMatch>, SequencePolicyError>;

    /// Evaluate a dispatch prefix plus the next node (run-time, R3).
    ///
    /// Called inside `run_dispatch_loop` (the single dispatch choke point,
    /// shared by `execute_graph` and `resume_execution`) when the next step
    /// would complete a forbidden sequence over the completed prefix.
    ///
    /// `principal` follows the same R7 semantics as [`Self::evaluate_plan`];
    /// the executor does not currently track a per-run principal, so
    /// callers pass `None` — same-principal history rules therefore evaluate
    /// at plan time (R2), not mid-dispatch (documented in the module spec).
    ///
    /// # Errors
    /// - Fail-closed: any error halts the run with `SequencePolicyError` —
    ///   the node is not dispatched
    async fn evaluate_prefix(
        &self,
        prefix: &[DispatchedStep],
        next: &PlannedStep,
        principal: Option<&str>,
    ) -> Result<Vec<SequenceMatch>, SequencePolicyError>;
}
