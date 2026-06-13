//! RetryStrategy — defines what action to take when a failure occurs.
//!
//! @canonical .pi/architecture/modules/failure-classification.md#strategies
//! Implements: Contract Freeze — RetryStrategy enum
//! Issue: #33
//!
//! # Contract (Frozen)
//! - Five strategy variants with clear semantics
//! - `PatchWithFeedback` and `ExpandContext` carry context data
//! - Implements `Clone`, `Debug` for testability
//! - Serialization support for eventing and API responses
//! - Display implementation for logging

use std::fmt;

use serde::{Deserialize, Serialize};

/// Defines what action to take when a failure is classified.
///
/// Each `FailureType` maps to a recommended `RetryStrategy`. The strategy
/// is selected by `FailureMappingService` and executed by the DAG executor
/// to recover from the failure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RetryStrategy {
    /// Retry the exact same operation (for transient failures).
    SameOperation,

    /// Re-execute the entire task from scratch (for LSP conflicts).
    ReExecute,

    /// Retry with error feedback (for test/build failures that need replanning).
    PatchWithFeedback {
        /// The error message or compiler output to use as feedback.
        feedback: String,
    },

    /// Execute a fallback task instead (for resource/system failures).
    Fallback,

    /// Expand context window and retry (for context-sensitive failures).
    ExpandContext {
        /// How many context levels to expand (0 = current, 1 = parent, etc.).
        /// Must be in range 0-5. Default is 1.
        level: u8,
    },
}

impl RetryStrategy {
    /// Returns a human-readable description of this strategy.
    ///
    /// Used for logging and event payloads.
    pub fn description(&self) -> &str {
        match self {
            RetryStrategy::SameOperation => "Retry the same operation",
            RetryStrategy::ReExecute => "Re-execute from scratch",
            RetryStrategy::PatchWithFeedback { .. } => "Retry with error feedback",
            RetryStrategy::Fallback => "Execute fallback task",
            RetryStrategy::ExpandContext { .. } => "Expand context and retry",
        }
    }
}

impl fmt::Display for RetryStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RetryStrategy::SameOperation => write!(f, "SameOperation"),
            RetryStrategy::ReExecute => write!(f, "ReExecute"),
            RetryStrategy::PatchWithFeedback { .. } => write!(f, "PatchWithFeedback"),
            RetryStrategy::Fallback => write!(f, "Fallback"),
            RetryStrategy::ExpandContext { level } => write!(f, "ExpandContext({})", level),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_same_operation() {
        assert_eq!(RetryStrategy::SameOperation.to_string(), "SameOperation");
    }

    #[test]
    fn test_display_re_execute() {
        assert_eq!(RetryStrategy::ReExecute.to_string(), "ReExecute");
    }

    #[test]
    fn test_display_patch_with_feedback() {
        let strategy = RetryStrategy::PatchWithFeedback {
            feedback: "test error".to_string(),
        };
        assert_eq!(strategy.to_string(), "PatchWithFeedback");
    }

    #[test]
    fn test_display_fallback() {
        assert_eq!(RetryStrategy::Fallback.to_string(), "Fallback");
    }

    #[test]
    fn test_display_expand_context() {
        let strategy = RetryStrategy::ExpandContext { level: 1 };
        assert_eq!(strategy.to_string(), "ExpandContext(1)");
    }

    #[test]
    fn test_description() {
        assert_eq!(
            RetryStrategy::SameOperation.description(),
            "Retry the same operation"
        );
        assert_eq!(
            RetryStrategy::ReExecute.description(),
            "Re-execute from scratch"
        );
        assert_eq!(
            RetryStrategy::Fallback.description(),
            "Execute fallback task"
        );
    }

    #[test]
    fn test_serialization_roundtrip() {
        let strategies = vec![
            RetryStrategy::SameOperation,
            RetryStrategy::ReExecute,
            RetryStrategy::PatchWithFeedback {
                feedback: "build error".to_string(),
            },
            RetryStrategy::Fallback,
            RetryStrategy::ExpandContext { level: 2 },
        ];

        for strategy in strategies {
            let json = serde_json::to_string(&strategy).unwrap();
            let deserialized: RetryStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{:?}", strategy), format!("{:?}", deserialized));
        }
    }
}
