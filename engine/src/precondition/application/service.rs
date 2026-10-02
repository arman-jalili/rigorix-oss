//! PreconditionService — the application service that matches a step, enforces
//! `require_params` presence, runs the check, and returns a verdict.
//!
//! @canonical .pi/architecture/modules/consequence-gating.md#preconditionservice
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: Contract Freeze — PreconditionService trait
//! Issue: #938 (consequence-gating epic — contract freeze); behavior closed in
//!   ISSUE-CONSEQUENCE-GATING-4 (PreconditionService)
//!
//! Evaluation contract (v1):
//!
//! ```text
//! 1. load the operator config (per dispatch — never cached across T₀→Tₙ)
//! 2. for each precondition whose StepPredicate matches the about-to-dispatch
//!    step, in config order:
//!    a. verify every require_params pointer is present and non-null
//!       (missing → Deny{before the command runs})
//!    b. run the check through PreconditionRunner
//!    c. exit 0 → continue; non-zero → Deny{failed};
//!       timeout/spawn/trust-boundary → Deny{error}
//! 3. no matching refusal → Dispatch
//! ```
//!
//! # Contract (Frozen)
//! - The service is async and trait-object safe (`Send + Sync`, `async-trait`)
//! - **Fail closed**: any config/load/match/run failure refuses the step —
//!   never a silent pass-through to dispatch
//! - A missing config file is **not** an error: it yields `Dispatch`
//!   (fail-open-absent — status quo, no gating)
//! - Non-matching steps are unaffected: no match ⇒ no process spawned
//! - Deterministic: identical step + config + check process ⇒ identical verdict
//!   (no LLM, no retries that change the verdict)
//! - The service never leaks parameter values into its verdict or findings

use async_trait::async_trait;

use crate::precondition::domain::{PreconditionError, PreconditionFinding, PreconditionVerdict};

use super::dto::DispatchStep;

/// Dispatch-time precondition evaluation service.
#[async_trait]
pub trait PreconditionService: Send + Sync {
    /// Evaluate every matching precondition for the step about to dispatch.
    ///
    /// # Returns
    /// - `Ok(PreconditionVerdict::Dispatch)` — no match, or every match passed
    /// - `Ok(PreconditionVerdict::Deny { .. })` — a match refused; the caller
    ///   must not call the tool and must mark the node failed
    ///
    /// # Errors
    /// - `PreconditionError::ConfigInvalid` — malformed / over-cap config
    ///   (fail closed)
    /// - `PreconditionError::NotArmed` — configured-but-unarmed (fail closed)
    /// - `PreconditionError::Match` — predicate evaluation failed (fail closed)
    async fn evaluate(
        &self,
        execution_id: uuid::Uuid,
        step: &DispatchStep,
    ) -> Result<PreconditionVerdict, PreconditionError>;

    /// Whether dispatch-time preconditions are configured at all.
    ///
    /// `false` → the gate is a no-op (no config file / empty set). Used by the
    /// hardening / arming path to refuse a consequential run that expected
    /// gating but has none (ADR-017 §Phase C).
    ///
    /// # Errors
    /// - `PreconditionError::ConfigInvalid` — config present but corrupt
    async fn is_configured(&self) -> Result<bool, PreconditionError>;

    /// Evaluate and return the redacted findings for every matching
    /// precondition (both passed and refused) alongside the verdict.
    ///
    /// Findings carry `precondition_id`, `step`, `outcome`, `exit_code`,
    /// `inputs_hash`, `checked_at`, and a redacted summary — never parameter
    /// values or stdout (SpanPrivacy). The default implementation preserves
    /// verdict-only implementors by returning no findings.
    async fn evaluate_with_findings(
        &self,
        execution_id: uuid::Uuid,
        step: &DispatchStep,
    ) -> Result<(PreconditionVerdict, Vec<PreconditionFinding>), PreconditionError> {
        Ok((self.evaluate(execution_id, step).await?, Vec::new()))
    }
}
