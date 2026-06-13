//! Service interfaces (use cases) for the Failure Classification bounded context.
//!
//! @canonical .pi/architecture/modules/failure-classification.md
//! Implements: Contract Freeze — FailureClassifierService and FailureMappingService traits
//! Issue: #33
//!
//! These traits define the application-level operations that can be performed
//! for failure classification and retry strategy selection. All methods are
//! async and return domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::failure_classification::domain::{FailureClassificationError, FailureType};

use super::dto::{
    CheckRetryEligibilityInput, CheckRetryEligibilityOutput, ClassifyFailureInput,
    ClassifyFailureOutput, GetRetryStrategyInput, GetRetryStrategyOutput, ValidateConfigInput,
    ValidateConfigOutput,
};

/// Application service for classifying error messages into `FailureType`.
///
/// Implementations perform pattern matching against known error message
/// patterns (built-in and optionally user-registered) to determine the
/// appropriate `FailureType` for a given error message.
///
/// # Classification Rules (Built-in)
///
/// | Pattern | FailureType |
/// |---------|-------------|
/// | "test" + "fail"/"error" | TestFailure |
/// | "build fail"/"compile error" | BuildFailure |
/// | "lsp"/"type error"/"type mismatch" | LspConflict |
/// | "out of memory"/"disk full" | ResourceExhausted |
/// | "signal"/"process crash"/"killed" | SystemError |
/// | "timeout"/"connection"/"network" | Transient |
/// | else | NonRetryable |
#[async_trait]
pub trait FailureClassifierService: Send + Sync {
    /// Classify an error message into a `FailureType`.
    ///
    /// Patterns are matched case-insensitively. Returns the first matching
    /// pattern. If no pattern matches, returns `FailureType::NonRetryable`.
    /// Emits `FailureClassified` event on successful classification.
    async fn classify(
        &self,
        input: ClassifyFailureInput,
    ) -> Result<ClassifyFailureOutput, FailureClassificationError>;

    /// Classify an error message and return only the `FailureType`.
    ///
    /// Convenience method for callers that only need the type, not the
    /// full output DTO. Delegates to `classify()` internally.
    async fn classify_type(
        &self,
        error_message: &str,
    ) -> Result<FailureType, FailureClassificationError>;

    /// Check whether a `FailureType` is eligible for retry.
    ///
    /// Considers both the type's inherent retryability and any
    /// caller-provided retry limits (max retries, current count).
    async fn check_retry_eligibility(
        &self,
        input: CheckRetryEligibilityInput,
    ) -> Result<CheckRetryEligibilityOutput, FailureClassificationError>;
}

/// Application service for mapping `FailureType` to recommended `RetryStrategy`.
///
/// Each `FailureType` has a default recommended `RetryStrategy`. Callers can
/// optionally override the default strategy for specific failure types.
///
/// # Default Mapping
///
/// | FailureType | RetryStrategy | Notes |
/// |-------------|---------------|-------|
/// | Transient | SameOperation | Safe to retry identically |
/// | LspConflict | ReExecute | Re-run from scratch |
/// | ResourceExhausted | Fallback | Use alternative resources |
/// | SystemError | Fallback | Use fallback handler |
/// | TestFailure | PatchWithFeedback | Replan with error output |
/// | BuildFailure | PatchWithFeedback | Replan with compiler output |
/// | NonRetryable | N/A | Fatal — no retry |
#[async_trait]
pub trait FailureMappingService: Send + Sync {
    /// Get the recommended `RetryStrategy` for a `FailureType`.
    ///
    /// Uses the default mapping unless an override is provided in the input.
    /// Emits `StrategySelected` event when a strategy is returned.
    async fn get_strategy(
        &self,
        input: GetRetryStrategyInput,
    ) -> Result<GetRetryStrategyOutput, FailureClassificationError>;

    /// Validate the classification configuration.
    ///
    /// Checks that all FailureType → RetryStrategy mappings are valid,
    /// all custom patterns are non-empty, and expansion levels are within
    /// the valid range (0–5).
    async fn validate_config(
        &self,
        input: ValidateConfigInput,
    ) -> Result<ValidateConfigOutput, FailureClassificationError>;

    /// Register a custom pattern-to-FailureType mapping.
    ///
    /// Custom patterns take precedence over built-in patterns.
    /// Returns the number of patterns registered.
    async fn register_pattern(
        &self,
        pattern: String,
        target: FailureType,
    ) -> Result<u32, FailureClassificationError>;
}
