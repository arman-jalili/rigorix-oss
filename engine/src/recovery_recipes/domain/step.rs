//! RecoveryStep — individual executable recovery actions.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md#recoverystep
//! Implements: Contract Freeze — RecoveryStep enum
//! Issue: #438 (recovery-recipes epic)
//!
//! # Contract (Frozen)
//! - Each variant is a single atomic recovery action
//! - Parameterized variants (RetryConnection, RestartService, EscalateToHuman)
//!   carry typed fields for configuration
//! - Implements `Clone`, `Debug`, `PartialEq`, `Eq` for testability
//! - Serialization support for eventing and API responses
//! - Display implementation for logging

use std::fmt;

use serde::{Deserialize, Serialize};

/// Individual executable recovery actions.
///
/// Each variant represents a single atomic recovery action that can be
/// composed into a `RecoveryRecipe`. Steps are executed sequentially
/// until one succeeds or all are exhausted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryStep {
    /// Run a clean build (e.g., cargo clean && cargo build).
    CleanBuild,

    /// Retry the failed operation with expanded context (more file reads,
    /// broader search scope, increased context window).
    ExpandContext,

    /// Retry a tool connection with a configurable timeout in milliseconds.
    RetryConnection {
        /// Timeout in milliseconds for the retry attempt.
        #[serde(rename = "timeout_ms")]
        timeout_ms: u64,
    },

    /// Restart an external service or daemon by name.
    RestartService {
        /// Name of the service to restart.
        name: String,
    },

    /// Rebase the current branch onto main.
    RebaseBranch,

    /// Auto-accept a trust prompt for known repositories/tools.
    AcceptTrust,

    /// Restart the worker or executor process.
    RestartWorker,

    /// Escalate to a human operator with a reason.
    EscalateToHuman {
        /// Reason for escalation, as a human-readable string.
        reason: String,
    },
}

impl RecoveryStep {
    /// Returns the canonical snake_case name of this step.
    pub fn as_str(&self) -> &'static str {
        match self {
            RecoveryStep::CleanBuild => "clean_build",
            RecoveryStep::ExpandContext => "expand_context",
            RecoveryStep::RetryConnection { .. } => "retry_connection",
            RecoveryStep::RestartService { .. } => "restart_service",
            RecoveryStep::RebaseBranch => "rebase_branch",
            RecoveryStep::AcceptTrust => "accept_trust",
            RecoveryStep::RestartWorker => "restart_worker",
            RecoveryStep::EscalateToHuman { .. } => "escalate_to_human",
        }
    }

    /// Returns a human-readable description of this step.
    pub fn description(&self) -> &'static str {
        match self {
            RecoveryStep::CleanBuild => "Run a clean build from scratch",
            RecoveryStep::ExpandContext => "Retry with expanded context",
            RecoveryStep::RetryConnection { .. } => "Retry the connection with a timeout",
            RecoveryStep::RestartService { .. } => "Restart an external service",
            RecoveryStep::RebaseBranch => "Rebase the branch onto main",
            RecoveryStep::AcceptTrust => "Auto-accept trust prompt",
            RecoveryStep::RestartWorker => "Restart the worker process",
            RecoveryStep::EscalateToHuman { .. } => "Escalate to a human operator",
        }
    }

    /// Returns `true` if this step is considered safe to auto-execute
    /// (no destructive side effects beyond the intended recovery).
    pub fn is_safe(&self) -> bool {
        match self {
            RecoveryStep::CleanBuild
            | RecoveryStep::ExpandContext
            | RecoveryStep::RetryConnection { .. }
            | RecoveryStep::RebaseBranch
            | RecoveryStep::AcceptTrust
            | RecoveryStep::RestartWorker => true,
            // RestartService and EscalateToHuman require human judgment
            RecoveryStep::RestartService { .. } | RecoveryStep::EscalateToHuman { .. } => false,
        }
    }
}

impl fmt::Display for RecoveryStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecoveryStep::CleanBuild => write!(f, "CleanBuild"),
            RecoveryStep::ExpandContext => write!(f, "ExpandContext"),
            RecoveryStep::RetryConnection { timeout_ms } => {
                write!(f, "RetryConnection({}ms)", timeout_ms)
            }
            RecoveryStep::RestartService { name } => write!(f, "RestartService({})", name),
            RecoveryStep::RebaseBranch => write!(f, "RebaseBranch"),
            RecoveryStep::AcceptTrust => write!(f, "AcceptTrust"),
            RecoveryStep::RestartWorker => write!(f, "RestartWorker"),
            RecoveryStep::EscalateToHuman { reason } => write!(f, "EscalateToHuman({})", reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_str() {
        assert_eq!(RecoveryStep::CleanBuild.as_str(), "clean_build");
        assert_eq!(RecoveryStep::ExpandContext.as_str(), "expand_context");
        assert_eq!(
            RecoveryStep::RetryConnection { timeout_ms: 30000 }.as_str(),
            "retry_connection"
        );
        assert_eq!(
            RecoveryStep::RestartService {
                name: "lsp".to_string()
            }
            .as_str(),
            "restart_service"
        );
        assert_eq!(RecoveryStep::RebaseBranch.as_str(), "rebase_branch");
        assert_eq!(RecoveryStep::AcceptTrust.as_str(), "accept_trust");
        assert_eq!(RecoveryStep::RestartWorker.as_str(), "restart_worker");
        assert_eq!(
            RecoveryStep::EscalateToHuman {
                reason: "test".to_string()
            }
            .as_str(),
            "escalate_to_human"
        );
    }

    #[test]
    fn test_display_clean_build() {
        assert_eq!(RecoveryStep::CleanBuild.to_string(), "CleanBuild");
    }

    #[test]
    fn test_display_retry_connection() {
        let step = RecoveryStep::RetryConnection { timeout_ms: 30000 };
        assert_eq!(step.to_string(), "RetryConnection(30000ms)");
    }

    #[test]
    fn test_display_restart_service() {
        let step = RecoveryStep::RestartService {
            name: "lsp".to_string(),
        };
        assert_eq!(step.to_string(), "RestartService(lsp)");
    }

    #[test]
    fn test_display_escalate() {
        let step = RecoveryStep::EscalateToHuman {
            reason: "max attempts".to_string(),
        };
        assert_eq!(step.to_string(), "EscalateToHuman(max attempts)");
    }

    #[test]
    fn test_is_safe_clean_build() {
        assert!(RecoveryStep::CleanBuild.is_safe());
    }

    #[test]
    fn test_is_safe_restart_service() {
        assert!(!RecoveryStep::RestartService {
            name: "test".to_string()
        }
        .is_safe());
    }

    #[test]
    fn test_is_safe_escalate() {
        assert!(!RecoveryStep::EscalateToHuman {
            reason: "test".to_string()
        }
        .is_safe());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let steps = vec![
            RecoveryStep::CleanBuild,
            RecoveryStep::ExpandContext,
            RecoveryStep::RetryConnection { timeout_ms: 30000 },
            RecoveryStep::RestartService {
                name: "lsp".to_string(),
            },
            RecoveryStep::RebaseBranch,
            RecoveryStep::AcceptTrust,
            RecoveryStep::RestartWorker,
            RecoveryStep::EscalateToHuman {
                reason: "test".to_string(),
            },
        ];

        for step in steps {
            let json = serde_json::to_string(&step).unwrap();
            let deserialized: RecoveryStep = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{:?}", step), format!("{:?}", deserialized));
        }
    }
}
