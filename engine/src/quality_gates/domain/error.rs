//! Quality gate error types for the quality-gates bounded context.
//!
//! @canonical .pi/architecture/modules/quality-gates.md
//! Implements: Contract Freeze — QualityGateError enum
//! Issue: #449 (quality-gates epic)
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `QualityGateError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;

/// Errors that can occur during quality gate evaluation and configuration.
#[derive(Debug, Error)]
pub enum QualityGateError {
    /// No quality level could be determined for the given test scope.
    #[error("Could not classify test scope: {reason}")]
    ScopeClassificationFailed {
        /// Human-readable reason for the failure.
        reason: String,
    },

    /// The quality gate configuration is invalid.
    #[error("Invalid quality gate configuration: {detail}")]
    InvalidConfiguration {
        /// Details about why the configuration is invalid.
        detail: String,
    },

    /// A required dependency (event bus, execution engine, etc.) is unavailable.
    #[error("Dependency unavailable: {dependency} — {reason}")]
    DependencyUnavailable {
        /// Name of the unavailable dependency.
        dependency: String,
        /// Details about the failure.
        reason: String,
    },

    /// An invalid quality level value was encountered (e.g., during deserialization).
    #[error("Invalid quality level: {value}")]
    InvalidQualityLevel {
        /// The invalid value that was encountered.
        value: String,
    },

    /// No contract defined for the given task or template.
    #[error("No contract defined for: {target}")]
    MissingContract {
        /// The target (task ID, template name) that has no contract.
        target: String,
    },
}

impl QualityGateError {
    /// Returns `true` if this error represents a transient condition
    /// that might succeed on retry.
    pub fn is_retriable(&self) -> bool {
        matches!(self, QualityGateError::DependencyUnavailable { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_classification_failed() {
        let err = QualityGateError::ScopeClassificationFailed {
            reason: "no test results available".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Could not classify test scope: no test results available"
        );
    }

    #[test]
    fn test_invalid_configuration() {
        let err = QualityGateError::InvalidConfiguration {
            detail: "default_required_level must be set".to_string(),
        };
        assert!(err.to_string().contains("default_required_level"));
    }

    #[test]
    fn test_dependency_unavailable() {
        let err = QualityGateError::DependencyUnavailable {
            dependency: "event_bus".to_string(),
            reason: "not connected".to_string(),
        };
        assert!(err.is_retriable());
        assert!(err.to_string().contains("event_bus"));
    }

    #[test]
    fn test_invalid_quality_level() {
        let err = QualityGateError::InvalidQualityLevel {
            value: "ultra".to_string(),
        };
        assert_eq!(err.to_string(), "Invalid quality level: ultra");
    }

    #[test]
    fn test_missing_contract() {
        let err = QualityGateError::MissingContract {
            target: "task-42".to_string(),
        };
        assert!(!err.is_retriable());
    }

    #[test]
    fn test_error_trait_impl() {
        let err = QualityGateError::ScopeClassificationFailed {
            reason: "test".to_string(),
        };
        let std_err: &dyn std::error::Error = &err;
        assert_eq!(std_err.to_string(), "Could not classify test scope: test");
    }
}
