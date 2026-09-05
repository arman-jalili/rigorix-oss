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
