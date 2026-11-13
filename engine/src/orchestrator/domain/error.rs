//! Orchestrator error types.
//!
//! @canonical .pi/architecture/modules/orchestrator.md#errors
//! Implements: Contract Freeze — OrchestratorError enum
//! Issue: #338
//!
//! Errors that can occur during the orchestration lifecycle. Each variant
//! maps to a failure in a specific sub-service or phase.
//!
//! Note: `OrchestratorError` converts to `CoreOrchestratorError` via `#[from]`
//! at the crate root, not the other way around — sub-errors carry their own
//! types and are wrapped at the orchestrator boundary.
//!
//! # Contract (Frozen)
//! - `OrchestratorError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the crate root

use thiserror::Error;

/// Errors that can occur during orchestrator operations.
#[derive(Debug, Error)]
pub enum OrchestratorError {
    /// The planning phase failed (PlanningPipeline error).
    #[error("Planning failed: {detail}")]
    PlanningFailed {
        /// Human-readable error description.
        detail: String,
        /// The intent that was being planned.
        intent: String,
    },

    /// DAG execution failed (ParallelExecutionService error).
    #[error("Execution failed: {detail}")]
    ExecutionFailed {
        /// Human-readable error description.
        detail: String,
        /// How many DAG nodes completed before the failure.
        nodes_completed: u32,
        /// How many nodes remain unexecuted.
        nodes_remaining: u32,
    },

    /// State persistence failed (StateManagerService error).
    #[error("State persistence failed: {detail}")]
    StatePersistenceFailed {
        /// Human-readable error description.
        detail: String,
        /// Which execution state was being saved.
        state: String,
    },

    /// The caller's plan does not describe a graph (#1526).
    #[error("Invalid step dependencies ({kind}): {detail}")]
    InvalidStepDependencies {
        /// Which of the four refusals fired — stable, so a caller can branch without parsing.
        kind: &'static str,
        /// The refusal's message, which names the offending step(s).
        detail: String,
    },

    /// Cancellation signal could not be propagated.
    #[error("Cancellation failed: {detail}")]
    CancellationFailed {
        /// Human-readable error description.
        detail: String,
    },

    /// Audit envelope delivery failed (non-fatal).
    #[error("Audit delivery failed: {detail}")]
    AuditFailed {
        /// Human-readable error description.
        detail: String,
        /// Whether the execution record was still returned successfully.
        execution_completed: bool,
    },

    /// A matched sequence-policy `deny` rule refused the runbook before any
    /// step executed (R2 plan-time denial — the forbidden sequence never
    /// executes; the later matched step's tool is never called).
    #[error(
        "Sequence policy denied step '{later_step}' (rule '{rule_id}'): the plan was refused before any step executed"
    )]
    SequencePolicyDenied {
        /// Name of the later matched step that would have been denied.
        later_step: String,
        /// Stable id of the `deny` rule that matched.
        rule_id: String,
    },

    /// Sequence-policy evaluation failed. Fail-closed: the plan is refused
    /// rather than executed ungated (corrupt/over-cap rule config, or an
    /// unexpected evaluation error).
    #[error("Sequence policy evaluation failed (fail closed): {detail}")]
    SequencePolicyEvaluationFailed {
        /// Underlying evaluation error detail.
        detail: String,
    },

    /// ADR-016 (GAP-A-30): a deny-class cross-run rule would fire on
    /// `local_unanchored` history and no recorded `allow_unanchored` opt-in is
    /// present — the plan is refused (fail closed) because the engine cannot
    /// authenticate the evidence. Structured (rule + step) so the native API
    /// surfaces a structured refusal rather than an opaque evaluation failure.
    #[error(
        "Cross-run deny rule '{rule_id}' refused step '{step}': local history is unanchored (fail closed)"
    )]
    HistoryUnanchoredRefused {
        /// Stable id of the deny-class rule that matched.
        rule_id: String,
        /// Name of the later matched (refused) step.
        step: String,
    },

    /// An R9 operator-controlled step requirement (ADR-015) was unmet for a
    /// matched step and the requirement's action is `deny`. Fail-closed: the
    /// plan is refused before any step executes. Parameter VALUES are never
    /// included — only the requirement id, the step, and the unmet pointer
    /// names.
    #[error(
        "Step '{step}' does not meet operator requirement '{requirement_id}' (fail closed): missing {}",
        .unmet.join(", ")
    )]
    RequirementUnmet {
        /// Stable id of the requirement that fired.
        requirement_id: String,
        /// Name of the matched step that failed the requirement.
        step: String,
        /// Unmet obligation names — pointer names and/or "identity".
        unmet: Vec<String>,
    },

    /// A step declares `require_identity = true` but the caller has no
    /// attested identity (no session claim, or source = unverified).
    /// Refused at plan time — the step's tool is never called. L1 identity
    /// gate (F-20260907-05): an unauthenticated caller cannot run
    /// consequential steps; the error names the recovery that can actually
    /// help (#1277).
    #[error(
        "{}",
        identity_required_message(.step, .status, .reason.as_deref())
    )]
    IdentityRequired {
        /// Name of the step that requires an attested identity.
        step: String,
        /// Identity status at refusal time ("unauthenticated" / "unverified").
        status: String,
        /// The verifier's recorded cause, when the calling process had one
        /// (#1285). Quoted instead of naming a stage the engine cannot see.
        reason: Option<String>,
    },

    /// An internal orchestrator error occurred.
    #[error("Internal orchestrator error: {detail}")]
    Internal {
        /// Error detail for diagnostics.
        detail: String,
        /// Source module or service that generated the error.
        source_module: String,
    },
}

impl OrchestratorError {
    /// Returns `true` if the error is retriable at the orchestrator level.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            OrchestratorError::PlanningFailed { .. }
                | OrchestratorError::ExecutionFailed { .. }
                | OrchestratorError::StatePersistenceFailed { .. }
        )
    }
}

/// The refusal's recovery sentence, chosen by the identity status (#1277).
///
/// `unauthenticated` means there is no credential to check — logging in is the
/// fix. `unverified` means the credential is present and this process could
/// not check it; telling the caller to log in points at the one action that
/// cannot help, so the message names the two recoveries that can.
///
/// #1285 — the sentence names no stage. The engine sees an `IdentityRef` whose
/// source is `Unverified`; it cannot tell whether discovery failed or the JWKS
/// fetch did, and saying "discovery" was wrong for the common case (a session
/// whose IdP went away after discovery succeeded). When the calling process
/// recorded a cause it is quoted; verification — not rediscovery — is what is
/// re-attempted, so that is what the recovery sentence promises.
fn identity_required_message(step: &str, status: &str, reason: Option<&str>) -> String {
    if status == "unverified" {
        match reason {
            Some(cause) => format!(
                "Step '{step}' requires an attested identity but the caller is unverified — the \
                 claim is present and the last credential-verification attempt failed: {cause}. \
                 Re-run once the IdP answers (verification is re-attempted on the next call), or \
                 fix RIGORIX_IDP_ISSUER"
            ),
            None => format!(
                "Step '{step}' requires an attested identity but the caller is unverified — the \
                 claim is present and this process could not verify it: re-run once the IdP \
                 answers (verification is re-attempted on the next call), or fix \
                 RIGORIX_IDP_ISSUER; rigorix_auth_status reports the recorded cause"
            ),
        }
    } else {
        format!(
            "Step '{step}' requires an attested identity but the caller is unauthenticated — run \
             rigorix_auth_login (or execute as a local principal) first"
        )
    }
}
