//! Data Transfer Objects for the Permission Enforcer module.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md
//! Implements: Contract Freeze — DTO schemas for permission operations
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs

use serde::{Deserialize, Serialize};

use crate::permission::domain::{PermissionConfig, PermissionMode};

// ---------------------------------------------------------------------------
// Check Permission DTOs
// ---------------------------------------------------------------------------

/// Input for checking tool permission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckPermissionInput {
    /// The name of the tool being invoked.
    pub tool: String,

    /// The raw input/arguments to the tool.
    pub input: String,

    /// Optional execution ID for correlation.
    pub execution_id: Option<String>,

    /// Optional node ID (for DAG execution context).
    pub node_id: Option<String>,
}

/// Output from checking tool permission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckPermissionOutput {
    /// Whether the tool is allowed or denied.
    pub outcome: String, // "allowed" or "denied"

    /// The active permission mode.
    pub active_mode: String,

    /// The required permission mode.
    pub required_mode: String,

    /// Human-readable reason (present if denied).
    pub reason: Option<String>,

    /// Whether this operation requires user confirmation.
    pub requires_confirmation: bool,
}

// ---------------------------------------------------------------------------
// File Write Check DTOs
// ---------------------------------------------------------------------------

/// Input for checking a file write operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckFileWriteInput {
    /// The path being written to.
    pub path: String,

    /// The workspace root directory.
    pub workspace_root: String,

    /// Optional execution ID for correlation.
    pub execution_id: Option<String>,
}

/// Output from checking a file write operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckFileWriteOutput {
    /// Whether the write is allowed.
    pub allowed: bool,

    /// The active permission mode.
    pub active_mode: String,

    /// Whether the path is within the workspace boundary.
    pub within_workspace: bool,

    /// Reason if denied.
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Bash Check DTOs
// ---------------------------------------------------------------------------

/// Input for checking a bash command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckBashInput {
    /// The bash command string to classify and check.
    pub command: String,

    /// Optional execution ID for correlation.
    pub execution_id: Option<String>,
}

/// Output from checking a bash command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckBashOutput {
    /// Whether the command is allowed.
    pub allowed: bool,

    /// The classified command intent.
    pub classification: String,

    /// The active permission mode.
    pub active_mode: String,

    /// The minimum permission mode required for this command intent.
    pub required_mode: String,

    /// Reason if denied.
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Mode Management DTOs
// ---------------------------------------------------------------------------

/// Input for setting the active permission mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetModeInput {
    /// The new permission mode.
    pub mode: PermissionMode,

    /// Optional execution ID for correlation.
    pub execution_id: Option<String>,

    /// Source of the mode change (e.g., "cli", "user", "hook").
    pub source: String,

    /// Optional reason for the mode change.
    pub reason: Option<String>,
}

/// Output from setting the active permission mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetModeOutput {
    /// The previous permission mode.
    pub previous_mode: PermissionMode,

    /// The new permission mode.
    pub current_mode: PermissionMode,

    /// Whether the mode was successfully changed.
    pub success: bool,
}

/// Status response for the current permission state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionStatusOutput {
    /// The current active permission mode.
    pub active_mode: PermissionMode,

    /// The current permission configuration summary.
    pub config_summary: PermissionConfigSummary,
}

/// Summary of the permission configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfigSummary {
    /// Number of allow rules.
    pub allow_count: u32,

    /// Number of deny rules.
    pub deny_count: u32,

    /// Number of ask rules.
    pub ask_count: u32,

    /// Number of per-tool permission overrides.
    pub tool_permission_count: u32,
}

impl From<&PermissionConfig> for PermissionConfigSummary {
    fn from(config: &PermissionConfig) -> Self {
        Self {
            allow_count: config.allow.len() as u32,
            deny_count: config.deny.len() as u32,
            ask_count: config.ask.len() as u32,
            tool_permission_count: config.tool_permissions.len() as u32,
        }
    }
}

// ---------------------------------------------------------------------------
// Reload Policy DTOs
// ---------------------------------------------------------------------------

/// Output from reloading the permission policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadPolicyOutput {
    /// Whether the reload was successful.
    pub success: bool,

    /// Summary of the new configuration.
    pub config_summary: PermissionConfigSummary,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_config_summary_from_config() {
        let config = PermissionConfig::default();
        let summary = PermissionConfigSummary::from(&config);
        assert_eq!(summary.allow_count, config.allow.len() as u32);
        assert_eq!(summary.deny_count, config.deny.len() as u32);
        assert_eq!(summary.ask_count, config.ask.len() as u32);
        assert_eq!(
            summary.tool_permission_count,
            config.tool_permissions.len() as u32
        );
    }

    #[test]
    fn test_dto_serde() {
        let input = CheckPermissionInput {
            tool: "bash".to_string(),
            input: "ls -la".to_string(),
            execution_id: Some("exec-1".to_string()),
            node_id: Some("node-1".to_string()),
        };
        let json = serde_json::to_string(&input).unwrap();
        let deserialized: CheckPermissionInput = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tool, "bash");
        assert_eq!(deserialized.input, "ls -la");
    }
}
