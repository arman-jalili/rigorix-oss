//! Event payload schemas for the Plan Validation bounded context.
//!
//! @canonical .pi/architecture/modules/plan-validation.md#event
//! Implements: Contract Freeze — ValidationEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted throughout the validation loop lifecycle —
//! iteration started, iteration failed, validated, budget exhausted.
//! Consumers (orchestrator, audit, TUI) subscribe to these event
//! types via the EventBus.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `execution_id` correlates to the originating execution
//! - Events align with validation lifecycle: start → iteration → outcome

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Events emitted by the Plan Validation module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationEvent {
    /// The validation loop was initiated for a user intent.
    ValidationStarted {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The original user intent text.
        intent: String,
        /// Maximum number of iterations configured.
        max_iterations: u32,
        /// Required quality level for success.
        required_quality: String,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// A new iteration of the validation loop has started.
    IterationStarted {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The iteration number (1-indexed).
        iteration: u32,
        /// Whether this is a retry (iteration > 1).
        is_retry: bool,
        /// Whether the context has been augmented with failure feedback.
        context_augmented: bool,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// An iteration failed — failures were detected.
    IterationFailed {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The iteration number (1-indexed).
        iteration: u32,
        /// Number of failures detected in this iteration.
        failure_count: u32,
        /// LLM tokens consumed in this iteration.
        llm_tokens_used: u64,
        /// Duration of this iteration in milliseconds.
        duration_ms: u64,
        /// Summary of the failures for logging.
        failure_summary: Vec<String>,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// The validation loop succeeded — template passed all quality gates.
    ValidationSucceeded {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The template ID that was validated.
        template_id: String,
        /// Number of iterations required.
        iterations_used: u32,
        /// Total LLM tokens consumed across all iterations.
        cumulative_tokens: u64,
        /// Total duration in milliseconds.
        total_duration_ms: u64,
        /// The number of fixes applied across iterations.
        fixes_applied: u32,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// All retry attempts exhausted without passing validation.
    ValidationFailed {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// Number of iterations attempted.
        iterations_attempted: u32,
        /// Total LLM tokens consumed.
        cumulative_tokens: u64,
        /// Total duration in milliseconds.
        total_duration_ms: u64,
        /// Total number of failures across all iterations.
        total_failures: u32,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// The validation loop was aborted due to budget exhaustion.
    BudgetExhausted {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The cumulative token limit that was exceeded.
        max_cumulative_tokens: u64,
        /// Tokens actually consumed.
        cumulative_tokens: u64,
        /// Number of iterations completed before exhaustion.
        iterations_completed: u32,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// The validation loop was cancelled by the user or orchestrator.
    ValidationCancelled {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// Reason for cancellation.
        reason: String,
        /// Number of iterations completed before cancellation.
        iterations_completed: u32,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },
}
