//! Event payload schemas for the Failure Classification bounded context.
//!
//! @canonical .pi/architecture/modules/failure-classification.md
//! Implements: Contract Freeze — FailureClassificationEvent payload schemas
//! Issue: #33
//!
//! These events are emitted on the `EventBus` whenever a failure is classified,
//! a retry strategy is selected, or a classification error occurs. Consumers
//! (audit, console printer, TUI) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `sequence` is populated by EventBus at emission time

use serde::{Deserialize, Serialize};

use crate::failure_classification::domain::{FailureType, RetryStrategy};

/// Events emitted by the Failure Classification module.
///
/// Wrapped in `ExecutionEvent::FailureClassification(...)` at the orchestration layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailureClassificationEvent {
    /// A failure was successfully classified into a `FailureType`.
    FailureClassified {
        /// The original error message that was classified.
        original_error: String,
        /// The classified failure type.
        failure_type: FailureType,
        /// The recommended retry strategy.
        retry_strategy: RetryStrategy,
        /// Whether the failure is retryable.
        is_retryable: bool,
        /// Confidence score of the classification (0.0–1.0).
        /// Implementations may not populate this initially.
        confidence: Option<f64>,
    },

    /// Classification failed — no matching FailureType found.
    ClassificationFailed {
        /// The error message that could not be classified.
        original_error: String,
        /// Why classification failed.
        reason: String,
        /// Whether this was treated as NonRetryable (safety fallback).
        fell_back_to_non_retryable: bool,
    },

    /// A retry strategy was explicitly selected (e.g., by policy override).
    StrategySelected {
        /// The failure type the strategy was selected for.
        failure_type: FailureType,
        /// The selected retry strategy.
        strategy: RetryStrategy,
        /// Why this strategy was selected (e.g., "default mapping", "policy override").
        reason: String,
    },

    /// A custom classification pattern was registered (if using pattern repository).
    PatternRegistered {
        /// The pattern string that was registered.
        pattern: String,
        /// The failure type the pattern maps to.
        target_type: FailureType,
    },
}
