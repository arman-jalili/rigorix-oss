//! ScoredEvaluationError — typed error enum for all scored evaluation failures.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md#scored-evaluation-error
//! Implements: Contract Freeze — ScoredEvaluationError enum
//! Issue: #673 (scored-evaluation epic)
//!
//! # Contract (Frozen)
//! - Uses `thiserror::Error` derive macro
//! - Each variant carries structured context for error reporting
//! - Implements `is_retriable()` for execution policy integration
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;

/// Errors that can occur during scored evaluation.
#[derive(Debug, Clone, Error)]
pub enum ScoredEvaluationError {
    /// The requested backend was not found in the registry.
    #[error("Backend not found: {0}")]
    BackendNotFound(String),

    /// The backend returned an error during evaluation.
    #[error("Backend error: {0}")]
    BackendError(String),

    /// The rubric is invalid or malformed.
    #[error("Invalid rubric: {0}")]
    InvalidRubric(String),

    /// The artifact is invalid or malformed.
    #[error("Invalid artifact: {0}")]
    InvalidArtifact(String),

    /// The backend is unavailable (health check failed).
    #[error("Backend health check failed: {0}")]
    BackendUnavailable(String),

    /// The backend did not respond within the configured timeout.
    #[error("Timeout: backend did not respond within {0}ms")]
    Timeout(u64),

    /// An internal error occurred (should not happen in normal operation).
    #[error("Internal error: {0}")]
    Internal(String),
}

impl ScoredEvaluationError {
    /// Returns `true` if this error represents a transient condition
    /// that might succeed on retry.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            ScoredEvaluationError::BackendError(_)
                | ScoredEvaluationError::BackendUnavailable(_)
                | ScoredEvaluationError::Timeout(_)
        )
    }

    /// Returns a human-readable error category for this error.
    pub fn category(&self) -> &'static str {
        match self {
            ScoredEvaluationError::BackendNotFound(_) => "misconfiguration",
            ScoredEvaluationError::BackendError(_) => "backend",
            ScoredEvaluationError::InvalidRubric(_) => "validation",
            ScoredEvaluationError::InvalidArtifact(_) => "validation",
            ScoredEvaluationError::BackendUnavailable(_) => "infrastructure",
            ScoredEvaluationError::Timeout(_) => "timeout",
            ScoredEvaluationError::Internal(_) => "internal",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_not_found() {
        let err = ScoredEvaluationError::BackendNotFound("runtimeai".to_string());
        assert_eq!(err.to_string(), "Backend not found: runtimeai");
        assert!(!err.is_retriable());
        assert_eq!(err.category(), "misconfiguration");
    }

    #[test]
    fn test_backend_error() {
        let err = ScoredEvaluationError::BackendError("connection refused".to_string());
        assert!(err.to_string().contains("connection refused"));
        assert!(err.is_retriable());
        assert_eq!(err.category(), "backend");
    }

    #[test]
    fn test_invalid_rubric() {
        let err = ScoredEvaluationError::InvalidRubric("missing dimensions".to_string());
        assert_eq!(err.to_string(), "Invalid rubric: missing dimensions");
        assert!(!err.is_retriable());
        assert_eq!(err.category(), "validation");
    }

    #[test]
    fn test_invalid_artifact() {
        let err = ScoredEvaluationError::InvalidArtifact("empty content".to_string());
        assert_eq!(err.to_string(), "Invalid artifact: empty content");
        assert!(!err.is_retriable());
    }

    #[test]
    fn test_backend_unavailable() {
        let err = ScoredEvaluationError::BackendUnavailable("not reachable".to_string());
        assert!(err.is_retriable());
        assert_eq!(err.category(), "infrastructure");
    }

    #[test]
    fn test_timeout() {
        let err = ScoredEvaluationError::Timeout(30_000);
        assert_eq!(
            err.to_string(),
            "Timeout: backend did not respond within 30000ms"
        );
        assert!(err.is_retriable());
        assert_eq!(err.category(), "timeout");
    }

    #[test]
    fn test_internal() {
        let err = ScoredEvaluationError::Internal("unexpected state".to_string());
        assert!(!err.is_retriable());
        assert_eq!(err.category(), "internal");
    }

    #[test]
    fn test_error_trait_impl() {
        let err = ScoredEvaluationError::BackendError("test".to_string());
        let std_err: &dyn std::error::Error = &err;
        assert_eq!(std_err.to_string(), "Backend error: test");
    }

    #[test]
    fn test_all_variants_coverable() {
        // Ensure all variants can be constructed (compile check)
        let _ = ScoredEvaluationError::BackendNotFound("x".into());
        let _ = ScoredEvaluationError::BackendError("x".into());
        let _ = ScoredEvaluationError::InvalidRubric("x".into());
        let _ = ScoredEvaluationError::InvalidArtifact("x".into());
        let _ = ScoredEvaluationError::BackendUnavailable("x".into());
        let _ = ScoredEvaluationError::Timeout(0);
        let _ = ScoredEvaluationError::Internal("x".into());
    }
}
