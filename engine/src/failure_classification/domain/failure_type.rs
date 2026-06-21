//! FailureType — classification of execution failures into typed categories.
//!
//! @canonical .pi/architecture/modules/failure-classification.md#types
//! Implements: Contract Freeze — FailureType enum
//! Issue: #33
//!
//! # Contract (Frozen)
//! - Seven mutually exclusive failure categories
//! - Each variant has a clear semantic meaning
//! - Implements `Clone`, `Debug`, `PartialEq`, `Eq` for testability
//! - The `is_retryable()` method is the canonical retry eligibility check
//! - Serialization support for eventing and API responses

use serde::{Deserialize, Serialize};

/// Classifies execution failures into typed categories for retry routing.
///
/// Each variant corresponds to a class of failure with a predictable
/// remediation strategy. Used by `FailureClassifierService` to determine
/// retry eligibility and by `FailureMappingService` to select the
/// recommended `RetryStrategy`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FailureType {
    /// Network hiccup, timeout, connection reset — safe to retry with same operation.
    Transient,

    /// Test suite failure — requires replanning with error feedback.
    TestFailure,

    /// Build/compile failure — requires patching with compiler output.
    BuildFailure,

    /// LSP type conflict or type mismatch — use exponential backoff.
    LspConflict,

    /// Out of memory, disk full — try fallback strategy.
    ResourceExhausted,

    /// Process crash, I/O error — use fallback.
    SystemError,

    /// Bad input, auth failure, invalid configuration — no retry possible.
    NonRetryable,

    /// Missing dependency (crate, library, file) — retryable after resolution.
    MissingDependency,

    /// Plan-level conflict detected during execution — requires replanning.
    PlanConflict,

    /// Unclassified failure — subject to global policy.
    Unknown,
}

impl FailureType {
    /// Returns `true` if this failure type is eligible for automatic retry.
    ///
    /// Non-retryable failures (TestFailure, BuildFailure, NonRetryable)
    /// require human intervention or replanning, not automatic retry.
    ///
    /// # Contract
    /// - Transient → `true`
    /// - LspConflict → `true`
    /// - ResourceExhausted → `true`
    /// - SystemError → `true`
    /// - TestFailure → `false` (requires replanning with feedback)
    /// - BuildFailure → `false` (requires patching with compiler output)
    /// - NonRetryable → `false` (fatal, no retry possible)
    ///   Returns the canonical snake_case name of this failure type.
    pub fn as_str(&self) -> &'static str {
        match self {
            FailureType::Transient => "transient",
            FailureType::TestFailure => "test_failure",
            FailureType::BuildFailure => "build_failure",
            FailureType::LspConflict => "lsp_conflict",
            FailureType::ResourceExhausted => "resource_exhausted",
            FailureType::SystemError => "system_error",
            FailureType::NonRetryable => "non_retryable",
            FailureType::MissingDependency => "missing_dependency",
            FailureType::PlanConflict => "plan_conflict",
            FailureType::Unknown => "unknown",
        }
    }

    pub fn is_retryable(&self) -> bool {
        match self {
            FailureType::Transient
            | FailureType::LspConflict
            | FailureType::ResourceExhausted
            | FailureType::SystemError
            | FailureType::MissingDependency => true,
            FailureType::TestFailure
            | FailureType::BuildFailure
            | FailureType::NonRetryable
            | FailureType::PlanConflict
            | FailureType::Unknown => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable_transient() {
        assert!(FailureType::Transient.is_retryable());
    }

    #[test]
    fn test_is_retryable_lsp_conflict() {
        assert!(FailureType::LspConflict.is_retryable());
    }

    #[test]
    fn test_is_retryable_resource_exhausted() {
        assert!(FailureType::ResourceExhausted.is_retryable());
    }

    #[test]
    fn test_is_retryable_system_error() {
        assert!(FailureType::SystemError.is_retryable());
    }

    #[test]
    fn test_is_retryable_test_failure() {
        assert!(!FailureType::TestFailure.is_retryable());
    }

    #[test]
    fn test_is_retryable_build_failure() {
        assert!(!FailureType::BuildFailure.is_retryable());
    }

    #[test]
    fn test_is_retryable_non_retryable() {
        assert!(!FailureType::NonRetryable.is_retryable());
    }

    #[test]
    fn test_serialization_roundtrip() {
        for variant in &[
            FailureType::Transient,
            FailureType::TestFailure,
            FailureType::BuildFailure,
            FailureType::LspConflict,
            FailureType::ResourceExhausted,
            FailureType::SystemError,
            FailureType::NonRetryable,
            FailureType::MissingDependency,
            FailureType::PlanConflict,
            FailureType::Unknown,
        ] {
            let json = serde_json::to_string(variant).unwrap();
            let deserialized: FailureType = serde_json::from_str(&json).unwrap();
            assert_eq!(*variant, deserialized);
        }
    }
}
