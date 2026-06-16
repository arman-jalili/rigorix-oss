//! Event payload schemas for the Orchestrator bounded context.
//!
//! @canonical .pi/architecture/modules/orchestrator.md#events
//! Implements: Contract Freeze — OrchestratorEvent payload schemas
//! Issue: #338
//!
//! These events are emitted on the `EventBus` during the orchestration lifecycle.
//! Consumers (console, TUI, alerting) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `execution_id` correlates to the originating execution

use serde::{Deserialize, Serialize};

/// Events emitted by the Orchestrator module.
///
/// Wrapped in `ExecutionEvent::Orchestrator(...)` at the orchestration layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrchestratorEvent {
    /// A new execution run has started.
    RunStarted {
        /// The execution ID for this run.
        execution_id: uuid::Uuid,
        /// Timestamp when the run started (ISO 8601 / UTC).
        started_at: chrono::DateTime<chrono::Utc>,
        /// The user intent being executed.
        intent: String,
    },

    /// Execution run completed successfully.
    RunCompleted {
        /// The execution ID for this run.
        execution_id: uuid::Uuid,
        /// Duration of the run in milliseconds.
        duration_ms: u64,
        /// Number of DAG nodes executed.
        nodes_executed: u32,
        /// Number of DAG nodes that failed.
        nodes_failed: u32,
    },

    /// Execution run failed.
    RunFailed {
        /// The execution ID for this run.
        execution_id: uuid::Uuid,
        /// Duration in milliseconds before failure.
        duration_ms: u64,
        /// The phase in which the failure occurred.
        failed_phase: FailedPhase,
        /// Error description.
        reason: String,
    },

    /// Execution run was cancelled.
    RunCancelled {
        /// The execution ID for this run.
        execution_id: uuid::Uuid,
        /// Duration in milliseconds before cancellation.
        duration_ms: u64,
        /// Reason for cancellation (if provided).
        reason: Option<String>,
        /// Number of DAG nodes that were cancelled mid-execution.
        nodes_cancelled: u32,
    },

    /// State was persisted at a checkpoint.
    StatePersisted {
        /// The execution ID for this run.
        execution_id: uuid::Uuid,
        /// Current execution state saved.
        state: String,
        /// Duration of the persist operation in milliseconds.
        duration_ms: u64,
    },

    /// Cancellation signal was propagated to sub-services.
    CancellationPropagated {
        /// The execution ID for this run.
        execution_id: uuid::Uuid,
        /// Which services were notified.
        services_notified: Vec<String>,
        /// How many had already completed.
        services_already_completed: u32,
    },

    /// Audit envelope was built (regardless of send success).
    AuditEnvelopeBuilt {
        /// The execution ID for this run.
        execution_id: uuid::Uuid,
        /// Number of events included in the envelope.
        event_count: u32,
        /// Whether the envelope was sent successfully.
        sent_successfully: bool,
    },
}

/// The phase in which a run failure occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailedPhase {
    /// Failure during planning.
    Planning,
    /// Failure during DAG execution.
    Execution,
    /// Failure during state persistence.
    StatePersistence,
    /// Failure during cancellation.
    Cancellation,
    /// Failure during audit.
    Audit,
}
