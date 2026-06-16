//! Retry logic domain entities: RetryPolicy, RetryStrategy, BackoffStrategy,
//! RetryDecision, FailureContext.
//!
//! @canonical .pi/architecture/modules/execution-engine.md#retry
//! Implements: Contract Freeze — RetryPolicy, RetryStrategy, BackoffStrategy,
//! RetryDecision, FailureContext
//! Issue: issue-contract-freeze
//!
//! Defines the retry-related domain types for handling node execution failures.
//! The retry system supports multiple strategies, configurable backoff, and
//! integration with failure classification.
//!
//! # Contract (Frozen)
//! - RetryPolicy defines how a node should be retried (max attempts, strategy)
//! - BackoffStrategy defines the delay computation between retries
//! - RetryDecision contains the outcome of retry evaluation
//! - FailureContext carries the failure details for retry decision-making

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// RetryPolicy — Per-Session or Per-Node Retry Configuration
// ---------------------------------------------------------------------------

/// Configuration for retry behavior during node execution.
///
/// A RetryPolicy can be set at the session level (applied to all nodes)
/// or overridden per-node. It defines the number of retry attempts,
/// which strategies to use, and how backoff is computed.
///
/// # Contract (Frozen)
/// - `max_attempts`: Maximum total execution attempts (1 = no retry)
/// - `retry_strategies`: Ordered list of strategies to apply on each retry
/// - `backoff_strategy`: How to compute delay between retries
/// - `retryable_failures`: Which failure types trigger a retry
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum total execution attempts (1 = original + 0 retries).
    /// Default: 4 (original + 3 retries).
    pub max_attempts: u8,

    /// Ordered list of retry strategies to apply.
    /// Each retry attempt uses the next strategy in the list.
    /// If the list is exhausted, the last strategy is reused.
    pub retry_strategies: Vec<RetryStrategy>,

    /// How to compute the delay between retry attempts.
    pub backoff_strategy: BackoffStrategy,

    /// Which failure types should trigger a retry.
    /// If empty, all failures are retriable (subject to max_attempts).
    pub retryable_failures: Vec<String>,

    /// Whether to execute a fallback node when max_attempts is exhausted.
    pub enable_fallback: bool,

    /// Whether to skip the node entirely (mark as Skipped) instead of
    /// failing when max_attempts is exhausted and no fallback is configured.
    pub skip_on_exhaustion: bool,

    /// Optional description of what conditions should cause this node
    /// to be skipped immediately without any retry.
    pub skip_conditions: Option<Vec<String>>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 4,
            retry_strategies: vec![
                RetryStrategy::SameOperation,
                RetryStrategy::SameOperation,
                RetryStrategy::ExpandContext,
            ],
            backoff_strategy: BackoffStrategy::default(),
            retryable_failures: Vec::new(), // All failures retriable by default
            enable_fallback: true,
            skip_on_exhaustion: false,
            skip_conditions: None,
        }
    }
}

impl RetryPolicy {
    /// Determine the retry strategy for a given attempt number (0-indexed).
    ///
    /// Returns the strategy at index `attempt` in the strategies list,
    /// or the last strategy if `attempt` exceeds the list length.
    pub fn strategy_for_attempt(&self, attempt: u8) -> RetryStrategy {
        let idx = attempt as usize;
        if idx < self.retry_strategies.len() {
            self.retry_strategies[idx]
        } else {
            *self
                .retry_strategies
                .last()
                .unwrap_or(&RetryStrategy::SameOperation)
        }
    }

    /// Returns true if the given failure type is retriable.
    ///
    /// If `retryable_failures` is empty, all failures are retriable.
    pub fn is_failure_retriable(&self, failure_type: &str) -> bool {
        if self.retryable_failures.is_empty() {
            true
        } else {
            self.retryable_failures.iter().any(|f| f == failure_type)
        }
    }

    /// Returns true if the node should be checked for skip conditions.
    pub fn has_skip_conditions(&self) -> bool {
        self.skip_conditions
            .as_ref()
            .is_some_and(|c| !c.is_empty())
    }
}

// ---------------------------------------------------------------------------
// RetryStrategy — Strategy for Retrying a Failed Node
// ---------------------------------------------------------------------------

/// Strategy to apply when retrying a failed node.
///
/// Determines how the execution engine should approach a retry:
/// - `SameOperation`: Retry the exact same operation with the same parameters
/// - `ExpandContext`: Retry with expanded context (more files, dependencies)
/// - `SimplifyOperation`: Retry with a simpler/smaller scope
/// - `AlternateApproach`: Retry using a different approach entirely
/// - `SkipAndContinue`: Skip the node and continue execution
///
/// # Contract (Frozen)
/// - Strategies can be ordered per RetryPolicy (escalating approach)
/// - SkipAndContinue is terminal (node is marked Skipped, not Failed)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RetryStrategy {
    /// Retry the exact same operation with the same parameters.
    SameOperation,
    /// Retry with expanded context (more files, dependencies, information).
    ExpandContext,
    /// Retry with a simpler or smaller scope.
    SimplifyOperation,
    /// Retry using a different approach entirely.
    AlternateApproach,
    /// Skip the node and continue execution (marks node as Skipped).
    SkipAndContinue,
}

impl RetryStrategy {
    /// Returns the canonical snake_case name of this strategy.
    pub fn as_str(&self) -> &'static str {
        match self {
            RetryStrategy::SameOperation => "same_operation",
            RetryStrategy::ExpandContext => "expand_context",
            RetryStrategy::SimplifyOperation => "simplify_operation",
            RetryStrategy::AlternateApproach => "alternate_approach",
            RetryStrategy::SkipAndContinue => "skip_and_continue",
        }
    }

    /// Returns true if this strategy results in the node being marked
    /// as Skipped (terminal, not a failure).
    pub fn is_skip(&self) -> bool {
        matches!(self, RetryStrategy::SkipAndContinue)
    }
}

// ---------------------------------------------------------------------------
// BackoffStrategy — Delay Computation Between Retries
// ---------------------------------------------------------------------------

/// Strategy for computing the delay between retry attempts.
///
/// # Contract (Frozen)
/// - `Fixed`: Constant delay between retries (use `base_delay_ms`)
/// - `Exponential`: Exponential backoff: `delay = base * multiplier^attempt`
/// - `Linear`: Linear backoff: `delay = base + (step * attempt)`
/// - `Immediate`: No delay (0ms between retries)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BackoffStrategy {
    /// Constant delay between retries.
    Fixed {
        /// Delay in milliseconds.
        base_delay_ms: u64,
    },
    /// Exponential backoff: delay = min(base * multiplier^attempt, max_delay_ms).
    Exponential {
        /// Base delay in milliseconds.
        base_delay_ms: u64,
        /// Multiplier applied each retry (must be >= 1.0).
        multiplier: f64,
        /// Maximum delay cap in milliseconds.
        max_delay_ms: u64,
    },
    /// Linear backoff: delay = base_delay_ms + (step_ms * attempt).
    Linear {
        /// Base delay in milliseconds.
        base_delay_ms: u64,
        /// Additional milliseconds per retry attempt.
        step_ms: u64,
        /// Maximum delay cap in milliseconds.
        max_delay_ms: u64,
    },
    /// No delay between retries.
    Immediate,
}

impl Default for BackoffStrategy {
    fn default() -> Self {
        BackoffStrategy::Exponential {
            base_delay_ms: 100,
            multiplier: 2.0,
            max_delay_ms: 30_000,
        }
    }
}

impl BackoffStrategy {
    /// Compute the delay for a given retry attempt (0-indexed).
    ///
    /// # Arguments
    /// * `attempt` - The retry attempt number (0 = first retry after initial failure).
    ///
    /// # Returns
    /// The delay in milliseconds before this retry attempt should proceed.
    pub fn compute_delay_ms(&self, attempt: u8) -> u64 {
        match self {
            BackoffStrategy::Fixed { base_delay_ms } => *base_delay_ms,
            BackoffStrategy::Exponential {
                base_delay_ms,
                multiplier,
                max_delay_ms,
            } => {
                let delay = (*base_delay_ms as f64 * multiplier.powi(attempt as i32)) as u64;
                delay.min(*max_delay_ms)
            }
            BackoffStrategy::Linear {
                base_delay_ms,
                step_ms,
                max_delay_ms,
            } => {
                let delay = *base_delay_ms + (*step_ms * attempt as u64);
                delay.min(*max_delay_ms)
            }
            BackoffStrategy::Immediate => 0,
        }
    }

    /// Returns the canonical snake_case name of this strategy.
    pub fn as_str(&self) -> &'static str {
        match self {
            BackoffStrategy::Fixed { .. } => "fixed",
            BackoffStrategy::Exponential { .. } => "exponential",
            BackoffStrategy::Linear { .. } => "linear",
            BackoffStrategy::Immediate => "immediate",
        }
    }
}

// ---------------------------------------------------------------------------
// RetryDecision — Outcome of Retry Evaluation
// ---------------------------------------------------------------------------

/// Decision about whether and how to retry a failed node.
///
/// Produced by the retry evaluation logic and consumed by the parallel
/// executor to determine the next action for a failed node.
///
/// # Contract (Frozen)
/// - `Retry`: The node should be retried with the given strategy and delay
/// - `Fallback`: Retries exhausted; execute the fallback node
/// - `Skip`: Skip the node (mark as Skipped, not Failed)
/// - `Abort`: Abort the entire execution with a permanent failure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RetryDecision {
    /// Retry the node with the specified strategy and backoff delay.
    Retry {
        /// The retry strategy to apply.
        strategy: RetryStrategy,
        /// Next attempt number (1-indexed).
        attempt: u8,
        /// Backoff delay in milliseconds before retrying.
        backoff_ms: u64,
        /// Human-readable reason for the retry decision.
        reason: String,
    },
    /// Execute the fallback node (retries exhausted).
    Fallback {
        /// The UUID of the fallback node to execute.
        fallback_node_id: Uuid,
        /// Human-readable reason for the fallback decision.
        reason: String,
    },
    /// Skip the node (mark as Skipped).
    Skip {
        /// Human-readable reason for skipping.
        reason: String,
    },
    /// Abort the entire execution.
    Abort {
        /// Human-readable reason for aborting.
        reason: String,
    },
}

impl RetryDecision {
    /// Returns true if this decision results in a retry.
    pub fn is_retry(&self) -> bool {
        matches!(self, RetryDecision::Retry { .. })
    }

    /// Returns true if this decision is terminal for the node.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            RetryDecision::Fallback { .. }
                | RetryDecision::Skip { .. }
                | RetryDecision::Abort { .. }
        )
    }
}

// ---------------------------------------------------------------------------
// FailureContext — Failure Details for Retry Decision-Making
// ---------------------------------------------------------------------------

/// Contextual information about a node failure, used by the retry logic
/// to make informed decisions about retry strategy selection.
///
/// # Contract (Frozen)
/// - Carries the failure type, error message, and timing
/// - Includes the execution history (previous attempts) for analysis
/// - Provides the node's intent for strategy selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureContext {
    /// The UUID of the node that failed.
    pub node_id: Uuid,
    /// The node's name for display purposes.
    pub node_name: String,
    /// The node's tool binding that was being executed.
    pub tool: String,
    /// The node's intent description.
    pub intent: String,
    /// The failure type classification.
    pub failure_type: String,
    /// The error message from the failure.
    pub error_message: String,
    /// The retry attempt number when this failure occurred (0 = first attempt).
    pub attempt: u8,
    /// The maximum number of attempts configured.
    pub max_attempts: u8,
    /// Duration of this attempt in milliseconds.
    pub duration_ms: u64,
    /// Total accumulated execution time across all attempts in milliseconds.
    pub total_duration_ms: u64,
    /// ISO 8601 timestamp of the failure.
    pub timestamp: DateTime<Utc>,
    /// List of previous error messages from prior attempts (empty on first failure).
    pub previous_errors: Vec<String>,
}

impl FailureContext {
    /// Create a new FailureContext.
    pub fn new(
        node_id: Uuid,
        node_name: impl Into<String>,
        tool: impl Into<String>,
        intent: impl Into<String>,
        failure_type: impl Into<String>,
        error_message: impl Into<String>,
        attempt: u8,
        max_attempts: u8,
        duration_ms: u64,
        total_duration_ms: u64,
    ) -> Self {
        Self {
            node_id,
            node_name: node_name.into(),
            tool: tool.into(),
            intent: intent.into(),
            failure_type: failure_type.into(),
            error_message: error_message.into(),
            attempt,
            max_attempts,
            duration_ms,
            total_duration_ms,
            timestamp: Utc::now(),
            previous_errors: Vec::new(),
        }
    }

    /// Returns true if this is the first failure (no prior retries).
    pub fn is_first_failure(&self) -> bool {
        self.attempt == 0
    }

    /// Returns true if retries are exhausted.
    pub fn is_exhausted(&self) -> bool {
        self.attempt + 1 >= self.max_attempts
    }

    /// Returns the number of retries remaining (saturating).
    pub fn retries_remaining(&self) -> u8 {
        self.max_attempts.saturating_sub(self.attempt + 1)
    }
}
