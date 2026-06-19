//! PolicyEngineError types.
//!
//! @canonical .pi/architecture/modules/policy-engine.md#errors
//! Implements: Contract Freeze — PolicyEngineError enum
//! Issue: issue-contract-freeze
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `PolicyEngineError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;

/// Errors that can occur during policy engine operations.
#[derive(Debug, Error)]
pub enum PolicyEngineError {
    /// A rule was not found by the requested identifier.
    #[error("Policy rule not found: {rule_name}")]
    RuleNotFound {
        /// The name of the rule that was not found.
        rule_name: String,
    },

    /// The policy configuration is invalid.
    #[error("Invalid policy configuration: {detail}")]
    InvalidConfiguration {
        /// Details about the configuration error.
        detail: String,
    },

    /// A condition evaluation failed unexpectedly.
    #[error("Condition evaluation error: {detail}")]
    ConditionEvaluationError {
        /// Details about the evaluation error.
        detail: String,
    },

    /// No matching rules were found for the given context.
    #[error("No matching rules for context (lane: {lane_id})")]
    NoMatchingRule {
        /// The lane ID for which no rule matched.
        lane_id: String,
    },

    /// The policy engine is in an invalid state.
    #[error("Invalid policy engine state: {detail}")]
    InvalidState {
        /// Details about the state error.
        detail: String,
    },

    /// A repository operation failed.
    #[error("Policy repository error: {detail}")]
    RepositoryError {
        /// Details about the repository error.
        detail: String,
    },

    /// Failed to deserialize policy configuration from source.
    #[error("Failed to deserialize policy configuration: {detail}")]
    DeserializationError {
        /// Details about the deserialization error.
        detail: String,
    },
}

impl PolicyEngineError {
    /// Whether this error can be retried.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            PolicyEngineError::RepositoryError { .. }
                | PolicyEngineError::ConditionEvaluationError { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_not_found_error() {
        let err = PolicyEngineError::RuleNotFound {
            rule_name: "test-rule".to_string(),
        };
        assert!(!err.is_retriable());
        assert_eq!(
            err.to_string(),
            "Policy rule not found: test-rule"
        );
    }

    #[test]
    fn test_repository_error_is_retriable() {
        let err = PolicyEngineError::RepositoryError {
            detail: "connection failed".to_string(),
        };
        assert!(err.is_retriable());
    }

    #[test]
    fn test_invalid_config_not_retriable() {
        let err = PolicyEngineError::InvalidConfiguration {
            detail: "missing rules".to_string(),
        };
        assert!(!err.is_retriable());
    }
}
