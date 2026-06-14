//! Data Transfer Objects for the Failure Classification module.
//!
//! @canonical .pi/architecture/modules/failure-classification.md
//! Implements: Contract Freeze — DTO schemas for failure classification
//! Issue: #33
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types (no framework-specific annotations)

use serde::{Deserialize, Serialize};

use crate::failure_classification::domain::{FailureType, RetryStrategy};

// ---------------------------------------------------------------------------
// Classify Failure DTOs
// ---------------------------------------------------------------------------

/// Input for classifying an error message into a `FailureType`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyFailureInput {
    /// The error message string to classify.
    /// Must be non-empty. Truncated to 4096 characters internally if longer.
    pub error_message: String,

    /// Optional context about the operation that failed.
    /// E.g., "executing test", "building project", "LSP analysis".
    /// Used by implementations to improve classification accuracy.
    pub operation_context: Option<String>,

    /// Optional source identifier for where the error originated.
    /// E.g., "git_diff", "shell_command", "lsp_server".
    pub source: Option<String>,
}

/// Output from classifying an error message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassifyFailureOutput {
    /// The classified failure type.
    pub failure_type: FailureType,

    /// The recommended retry strategy for this failure type.
    pub recommended_strategy: RetryStrategy,

    /// Whether the failure is retryable (convenience field).
    pub is_retryable: bool,

    /// Confidence score of the classification (0.0–1.0).
    /// May be `None` if the implementation does not compute confidence.
    pub confidence: Option<f64>,

    /// Human-readable explanation of why this classification was chosen.
    pub explanation: Option<String>,
}

// ---------------------------------------------------------------------------
// Get Retry Strategy DTOs
// ---------------------------------------------------------------------------

/// Input for getting the recommended `RetryStrategy` for a `FailureType`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRetryStrategyInput {
    /// The failure type to get a strategy for.
    pub failure_type: FailureType,

    /// Optional override — if set, bypasses the default mapping.
    pub override_strategy: Option<RetryStrategy>,
}

/// Output from getting a retry strategy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetRetryStrategyOutput {
    /// The selected retry strategy.
    pub strategy: RetryStrategy,

    /// Whether this strategy came from the default mapping or an override.
    pub source: StrategySource,

    /// Human-readable description of the strategy.
    pub description: String,
}

/// Source of a retry strategy selection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StrategySource {
    /// The strategy came from the default FailureType → RetryStrategy mapping.
    DefaultMapping,
    /// The strategy was explicitly overridden by the caller.
    Override,
    /// The strategy was configured via a policy or rule.
    PolicyOverride,
}

// ---------------------------------------------------------------------------
// Retry Eligibility DTOs
// ---------------------------------------------------------------------------

/// Input for checking retry eligibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRetryEligibilityInput {
    /// The failure type to check.
    pub failure_type: FailureType,

    /// Optional current retry count (for retry limit checks).
    pub current_retry_count: Option<u32>,

    /// Optional max retry limit (defaults to 3 if not set).
    pub max_retries: Option<u32>,
}

/// Output from checking retry eligibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckRetryEligibilityOutput {
    /// Whether the failure is eligible for retry.
    pub eligible: bool,
    /// Why the failure is or isn't eligible.
    pub reason: String,
    /// Remaining retry attempts, if applicable.
    pub remaining_attempts: Option<u32>,
}

// ---------------------------------------------------------------------------
// Validate Classification Config DTOs
// ---------------------------------------------------------------------------

/// Input for validating the classification configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateConfigInput {
    /// Custom pattern-to-FailureType mappings to validate.
    /// If empty, validates the built-in default mappings only.
    pub custom_patterns: Option<std::collections::HashMap<String, FailureType>>,

    /// Custom FailureType-to-RetryStrategy mappings to validate.
    pub custom_strategy_mappings: Option<std::collections::HashMap<FailureType, RetryStrategy>>,
}

/// Output from validating classification configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidateConfigOutput {
    /// Whether the configuration is valid.
    pub valid: bool,
    /// List of validation errors (empty if valid).
    pub errors: Vec<ValidationError>,
    /// List of warnings (non-blocking issues).
    pub warnings: Vec<String>,
}

/// A single validation error with structured context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationError {
    /// The field or configuration key that failed validation.
    pub field: String,
    /// Human-readable error message.
    pub message: String,
    /// The invalid value, if representable.
    pub value: Option<String>,
}
