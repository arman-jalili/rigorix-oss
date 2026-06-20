//! PermissionOutcome — structured result of permission enforcement.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md#outcome
//! Implements: Contract Freeze — PermissionOutcome / EnforcementResult enum
//! Issue: issue-contract-freeze
//!
//! Represents the result of a permission check. A tool invocation is
//! either `Allowed` or `Denied` with a structured reason that includes
//! the tool name, the active mode, the required mode, and a human-readable
//! explanation. This structured feedback allows the LLM to adapt.
//!
//! # Contract (Frozen)
//! - `Allowed` carries no extra data (fast path)
//! - `Denied` carries full context for LLM feedback and audit
//! - Serialized with `outcome` tag for easy deserialization

use serde::{Deserialize, Serialize};
use std::fmt;

/// The result of a permission enforcement check.
///
/// Returned by `PermissionPolicy::authorize()` and all
/// `PermissionEnforcer` check methods.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome")]
pub enum PermissionOutcome {
    /// The operation is permitted.
    Allowed,

    /// The operation is denied with a structured reason.
    Denied {
        /// The name of the tool that was denied.
        tool: String,

        /// The active permission mode at the time of denial.
        active_mode: String,

        /// The permission mode that would have been required.
        required_mode: String,

        /// Human-readable explanation of why the operation was denied.
        reason: String,
    },
}

impl PermissionOutcome {
    /// Returns `true` if the outcome is `Allowed`.
    pub fn is_allowed(&self) -> bool {
        matches!(self, PermissionOutcome::Allowed)
    }

    /// Returns `true` if the outcome is `Denied`.
    pub fn is_denied(&self) -> bool {
        matches!(self, PermissionOutcome::Denied { .. })
    }

    /// Returns the deny reason if denied, or `None` if allowed.
    pub fn deny_reason(&self) -> Option<&str> {
        match self {
            PermissionOutcome::Allowed => None,
            PermissionOutcome::Denied { reason, .. } => Some(reason.as_str()),
        }
    }

    /// Returns the denied tool if denied, or `None`.
    pub fn denied_tool(&self) -> Option<&str> {
        match self {
            PermissionOutcome::Allowed => None,
            PermissionOutcome::Denied { tool, .. } => Some(tool.as_str()),
        }
    }
}

impl fmt::Display for PermissionOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PermissionOutcome::Allowed => write!(f, "Allowed"),
            PermissionOutcome::Denied {
                tool,
                active_mode,
                required_mode,
                reason,
            } => {
                write!(
                    f,
                    "Denied[tool={}, active_mode={}, required_mode={}]: {}",
                    tool, active_mode, required_mode, reason
                )
            }
        }
    }
}

/// Alias for `PermissionOutcome` — used when integrating with
/// the existing enforcement module to emphasize enforcement semantics.
pub type EnforcementResult = PermissionOutcome;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowed_outcome() {
        let outcome = PermissionOutcome::Allowed;
        assert!(outcome.is_allowed());
        assert!(!outcome.is_denied());
        assert!(outcome.deny_reason().is_none());
        assert!(outcome.denied_tool().is_none());
    }

    #[test]
    fn test_denied_outcome() {
        let outcome = PermissionOutcome::Denied {
            tool: "bash".to_string(),
            active_mode: "read_only".to_string(),
            required_mode: "workspace_write".to_string(),
            reason: "bash requires workspace_write mode".to_string(),
        };
        assert!(!outcome.is_allowed());
        assert!(outcome.is_denied());
        assert_eq!(
            outcome.deny_reason(),
            Some("bash requires workspace_write mode")
        );
        assert_eq!(outcome.denied_tool(), Some("bash"));
    }

    #[test]
    fn test_display_allowed() {
        assert_eq!(PermissionOutcome::Allowed.to_string(), "Allowed");
    }

    #[test]
    fn test_display_denied() {
        let outcome = PermissionOutcome::Denied {
            tool: "write_file".to_string(),
            active_mode: "read_only".to_string(),
            required_mode: "workspace_write".to_string(),
            reason: "file writes not allowed".to_string(),
        };
        let display = outcome.to_string();
        assert!(display.contains("Denied"));
        assert!(display.contains("write_file"));
        assert!(display.contains("read_only"));
        assert!(display.contains("workspace_write"));
        assert!(display.contains("file writes not allowed"));
    }

    #[test]
    fn test_serde_roundtrip_allowed() {
        let outcome = PermissionOutcome::Allowed;
        let json = serde_json::to_string(&outcome).unwrap();
        let deserialized: PermissionOutcome = serde_json::from_str(&json).unwrap();
        assert_eq!(outcome, deserialized);
    }

    #[test]
    fn test_serde_roundtrip_denied() {
        let outcome = PermissionOutcome::Denied {
            tool: "rm".to_string(),
            active_mode: "read_only".to_string(),
            required_mode: "danger_full_access".to_string(),
            reason: "rm is destructive".to_string(),
        };
        let json = serde_json::to_string(&outcome).unwrap();
        let deserialized: PermissionOutcome = serde_json::from_str(&json).unwrap();
        assert_eq!(outcome, deserialized);
    }

    #[test]
    fn test_serde_tagged_serialization() {
        let json = serde_json::to_string(&PermissionOutcome::Allowed).unwrap();
        assert!(json.contains(r#""outcome":"Allowed""#) || json.contains("\"Allowed\""));

        let denied = PermissionOutcome::Denied {
            tool: "x".to_string(),
            active_mode: "a".to_string(),
            required_mode: "b".to_string(),
            reason: "test".to_string(),
        };
        let json = serde_json::to_string(&denied).unwrap();
        assert!(json.contains(r#""outcome":"Denied""#));
    }

    #[test]
    fn test_enforcement_result_alias() {
        let allowed: EnforcementResult = PermissionOutcome::Allowed;
        assert!(allowed.is_allowed());

        let denied: EnforcementResult = PermissionOutcome::Denied {
            tool: "test".to_string(),
            active_mode: "ro".to_string(),
            required_mode: "ww".to_string(),
            reason: "test deny".to_string(),
        };
        assert!(denied.is_denied());
    }
}
