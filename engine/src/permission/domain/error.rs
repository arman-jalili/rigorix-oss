//! PermissionError — typed error enum for the permission enforcer.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md#error
//! Implements: Contract Freeze — PermissionError enum
//! Issue: issue-contract-freeze
//!
//! All permission-related errors use `thiserror` derive macros.
//! Errors carry structured context for error reporting and LLM feedback.
//!
//! # Contract (Frozen)
//! - `PermissionError` is the single error type for this module
//! - Each variant carries structured context
//! - Implements `std::error::Error` for library compatibility
//! - All errors are non-retriable by default

use thiserror::Error;

/// Errors that can occur during permission enforcement.
#[derive(Debug, Error)]
pub enum PermissionError {
    /// A tool call was denied by the permission enforcer.
    #[error("Permission denied: {reason}")]
    PermissionDenied {
        /// The name of the tool that was denied.
        tool: String,
        /// Human-readable reason for the denial.
        reason: String,
        /// The active permission mode.
        active_mode: String,
        /// The required permission mode.
        required_mode: String,
    },

    /// The permission configuration is invalid.
    #[error("Invalid permission configuration: {detail}")]
    InvalidConfiguration {
        /// Details about the configuration error.
        detail: String,
    },

    /// The requested policy was not found.
    #[error("Policy not found for tool: {tool}")]
    PolicyNotFound {
        /// The name of the tool with no policy.
        tool: String,
    },

    /// The permission enforcer is in an invalid state.
    #[error("Invalid enforcer state: {detail}")]
    InvalidState {
        /// Details about the state error.
        detail: String,
    },

    /// The workspace root is not configured.
    #[error("Workspace root not configured")]
    WorkspaceRootNotConfigured,
}

impl PermissionError {
    /// Returns `true` if the error is retriable.
    /// Permission errors are generally not retriable.
    pub fn is_retriable(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_denied_error() {
        let err = PermissionError::PermissionDenied {
            tool: "bash".to_string(),
            reason: "bash requires workspace_write mode".to_string(),
            active_mode: "read_only".to_string(),
            required_mode: "workspace_write".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Permission denied"));
        assert!(msg.contains("bash"));
        assert!(msg.contains("workspace_write"));
        assert!(!err.is_retriable());
    }

    #[test]
    fn test_invalid_configuration_error() {
        let err = PermissionError::InvalidConfiguration {
            detail: "Unknown permission mode 'super_admin'".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Invalid permission configuration"));
        assert!(msg.contains("super_admin"));
    }

    #[test]
    fn test_policy_not_found_error() {
        let err = PermissionError::PolicyNotFound {
            tool: "unknown_tool".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Policy not found"));
        assert!(msg.contains("unknown_tool"));
    }

    #[test]
    fn test_invalid_state_error() {
        let err = PermissionError::InvalidState {
            detail: "Lock poisoned".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Invalid enforcer state"));
        assert!(msg.contains("Lock poisoned"));
    }

    #[test]
    fn test_workspace_root_not_configured() {
        let err = PermissionError::WorkspaceRootNotConfigured;
        assert_eq!(err.to_string(), "Workspace root not configured");
    }

    #[test]
    fn test_all_errors_non_retriable() {
        assert!(!PermissionError::PermissionDenied {
            tool: "x".to_string(),
            reason: "test".to_string(),
            active_mode: "ro".to_string(),
            required_mode: "ww".to_string(),
        }
        .is_retriable());

        assert!(!PermissionError::InvalidConfiguration {
            detail: "test".to_string()
        }
        .is_retriable());

        assert!(!PermissionError::PolicyNotFound {
            tool: "x".to_string()
        }
        .is_retriable());

        assert!(!PermissionError::InvalidState {
            detail: "test".to_string()
        }
        .is_retriable());

        assert!(!PermissionError::WorkspaceRootNotConfigured.is_retriable());
    }
}
