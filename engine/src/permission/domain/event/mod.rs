//! Event payload schemas for the Permission Enforcer.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md
//! Implements: Contract Freeze — PermissionEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted whenever permission enforcement actions
//! are taken — tools evaluated, modes changed, permissions overridden.
//! Consumers (audit, TUI) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries full context needed by consumers
//! - No internal implementation details exposed
//! - All events include an optional `execution_id` for correlation

use serde::{Deserialize, Serialize};

/// Events emitted by the Permission Enforcer module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PermissionEvent {
    /// A tool invocation was evaluated by the permission enforcer.
    ToolEvaluated {
        /// Optional execution ID for correlation.
        execution_id: Option<String>,
        /// The name of the tool being evaluated.
        tool: String,
        /// The active permission mode at evaluation time.
        active_mode: String,
        /// The required permission mode for this tool.
        required_mode: String,
        /// Whether the tool was allowed or denied.
        outcome: String, // "allowed" or "denied"
        /// The reason for the decision (if denied).
        reason: Option<String>,
    },

    /// The active permission mode was changed.
    ModeChanged {
        /// Optional execution ID for correlation.
        execution_id: Option<String>,
        /// The previous permission mode.
        previous_mode: String,
        /// The new permission mode.
        new_mode: String,
        /// Source of the change (e.g., "cli", "hook", "user").
        source: String,
        /// Reason for the change.
        reason: Option<String>,
    },

    /// A permission override was applied.
    OverrideApplied {
        /// Optional execution ID for correlation.
        execution_id: Option<String>,
        /// Type of override (e.g., "elevation", "bypass").
        override_type: String,
        /// Duration of the override in seconds (0 = single-use).
        duration_secs: u64,
        /// Source of the override.
        source: Option<String>,
        /// Reason for the override.
        reason: Option<String>,
    },

    /// A bash command was classified and evaluated.
    BashCommandEvaluated {
        /// Optional execution ID for correlation.
        execution_id: Option<String>,
        /// The bash command string.
        command: String,
        /// The classified intent.
        classification: String,
        /// Whether the command was allowed.
        allowed: bool,
        /// The active mode at evaluation.
        active_mode: String,
        /// The minimum mode required for this command intent.
        required_mode: String,
    },

    /// A file write was checked against workspace boundaries.
    FileWriteChecked {
        /// Optional execution ID for correlation.
        execution_id: Option<String>,
        /// The path that was checked.
        path: String,
        /// The workspace root.
        workspace_root: String,
        /// Whether the path is within the workspace.
        within_workspace: bool,
        /// Whether the write was allowed.
        allowed: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_evaluated_event() {
        let event = PermissionEvent::ToolEvaluated {
            execution_id: Some("exec-1".to_string()),
            tool: "bash".to_string(),
            active_mode: "read_only".to_string(),
            required_mode: "workspace_write".to_string(),
            outcome: "denied".to_string(),
            reason: Some("bash requires workspace_write".to_string()),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: PermissionEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, PermissionEvent::ToolEvaluated { .. }));
    }

    #[test]
    fn test_mode_changed_event() {
        let event = PermissionEvent::ModeChanged {
            execution_id: None,
            previous_mode: "read_only".to_string(),
            new_mode: "workspace_write".to_string(),
            source: "cli".to_string(),
            reason: Some("User requested elevation".to_string()),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: PermissionEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, PermissionEvent::ModeChanged { .. }));
    }

    #[test]
    fn test_serde_roundtrip_all_variants() {
        let variants = vec![
            PermissionEvent::ToolEvaluated {
                execution_id: None,
                tool: "test".to_string(),
                active_mode: "ro".to_string(),
                required_mode: "ww".to_string(),
                outcome: "denied".to_string(),
                reason: None,
            },
            PermissionEvent::ModeChanged {
                execution_id: None,
                previous_mode: "ro".to_string(),
                new_mode: "ww".to_string(),
                source: "cli".to_string(),
                reason: None,
            },
            PermissionEvent::OverrideApplied {
                execution_id: None,
                override_type: "elevation".to_string(),
                duration_secs: 0,
                source: None,
                reason: None,
            },
            PermissionEvent::BashCommandEvaluated {
                execution_id: None,
                command: "ls".to_string(),
                classification: "read_only".to_string(),
                allowed: true,
                active_mode: "read_only".to_string(),
                required_mode: "read_only".to_string(),
            },
            PermissionEvent::FileWriteChecked {
                execution_id: None,
                path: "/tmp/test".to_string(),
                workspace_root: "/workspace".to_string(),
                within_workspace: false,
                allowed: false,
            },
        ];

        for event in variants {
            let json = serde_json::to_string(&event).unwrap();
            let deserialized: PermissionEvent = serde_json::from_str(&json).unwrap();
            assert!(std::mem::discriminant(&event) == std::mem::discriminant(&deserialized));
        }
    }
}
