//! Factory interfaces for constructing Failure Classification domain objects.
//!
//! @canonical .pi/architecture/modules/failure-classification.md
//! Implements: Contract Freeze — StrategyFactory trait
//! Issue: #33
//!
//! Factories encapsulate the construction of complex domain objects,
//! allowing implementations to inject dependencies and apply defaults
//! without exposing construction logic to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured domain object
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::failure_classification::domain::{
    FailureClassificationError, FailureType, RetryStrategy,
};

/// Factory for constructing `RetryStrategy` instances.
///
/// Implementations handle strategy creation with validation
/// (e.g., ensuring `ExpandContext` levels are within range).
#[async_trait]
pub trait StrategyFactory: Send + Sync {
    /// Create a `PatchWithFeedback` strategy with the given feedback.
    ///
    /// Validates that the feedback string is non-empty.
    async fn create_patch_with_feedback(
        &self,
        feedback: &str,
    ) -> Result<RetryStrategy, FailureClassificationError>;

    /// Create an `ExpandContext` strategy with the given level.
    ///
    /// Validates that level is in range 0–5.
    /// Returns `FailureClassificationError::InvalidExpansionLevel` if out of range.
    async fn create_expand_context(
        &self,
        level: u8,
    ) -> Result<RetryStrategy, FailureClassificationError>;

    /// Create a `SameOperation` strategy (no configuration needed).
    fn create_same_operation(&self) -> RetryStrategy;

    /// Create a `ReExecute` strategy (no configuration needed).
    fn create_re_execute(&self) -> RetryStrategy;

    /// Create a `Fallback` strategy (no configuration needed).
    fn create_fallback(&self) -> RetryStrategy;

    /// Build the default mapping of FailureType → RetryStrategy.
    ///
    /// Returns the canonical mapping as defined in the architecture module.
    fn build_default_mapping(&self) -> std::collections::HashMap<FailureType, RetryStrategy>;
}
