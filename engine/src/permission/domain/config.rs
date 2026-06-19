//! PermissionConfig — configuration for the permission enforcer.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md#config
//! Implements: Contract Freeze — PermissionConfig struct
//! Issue: issue-contract-freeze
//!
//! Defines the configuration for the permission enforcer, including
//! the default mode, allow/deny/ask rule lists, and per-tool permission
//! mappings. Config is loaded from `.rigorix/permissions.toml` or
//! equivalent configuration source.
//!
//! # Contract (Frozen)
//! - All configuration is optional — defaults are safe
//! - Allow/deny/ask rules are simple string patterns (currently exact match)
//! - Per-tool permission overrides map tool names to required modes
//! - Serialized as TOML for the config file

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::PermissionMode;

/// Configuration for the permission enforcer.
///
/// Loaded from the `.rigorix/permissions.toml` file (or equivalent).
/// All fields have safe defaults so the enforcer works without
/// explicit configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionConfig {
    /// Default permission mode applied when no mode is explicitly set.
    #[serde(default)]
    pub default_mode: PermissionMode,

    /// Tool names that are always allowed (overrides mode check).
    #[serde(default)]
    pub allow: Vec<String>,

    /// Tool names that are always denied (overrides allow rules).
    #[serde(default)]
    pub deny: Vec<String>,

    /// Tool names that require user confirmation via prompter.
    #[serde(default)]
    pub ask: Vec<String>,

    /// Per-tool permission mode overrides.
    /// Maps tool name to the minimum mode required.
    #[serde(default)]
    pub tool_permissions: HashMap<String, PermissionMode>,
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self {
            default_mode: PermissionMode::WorkspaceWrite,
            allow: vec![
                "read_file".to_string(),
                "grep_search".to_string(),
                "glob".to_string(),
                "lsp_query".to_string(),
            ],
            deny: vec![],
            ask: vec![
                "git_commit".to_string(),
                "git_push".to_string(),
                "bash".to_string(),
            ],
            tool_permissions: HashMap::from([
                ("read_file".to_string(), PermissionMode::ReadOnly),
                ("grep_search".to_string(), PermissionMode::ReadOnly),
                ("glob".to_string(), PermissionMode::ReadOnly),
                ("lsp_query".to_string(), PermissionMode::ReadOnly),
                ("write_file".to_string(), PermissionMode::WorkspaceWrite),
                ("edit_file".to_string(), PermissionMode::WorkspaceWrite),
                ("create_file".to_string(), PermissionMode::WorkspaceWrite),
                ("delete_file".to_string(), PermissionMode::DangerousFullAccess),
                ("bash".to_string(), PermissionMode::WorkspaceWrite),
                ("git_commit".to_string(), PermissionMode::WorkspaceWrite),
                ("git_push".to_string(), PermissionMode::DangerousFullAccess),
                ("run_command".to_string(), PermissionMode::WorkspaceWrite),
            ]),
        }
    }
}

impl PermissionConfig {
    /// Create a `PermissionConfig` that allows everything (for testing).
    pub fn permissive() -> Self {
        Self {
            default_mode: PermissionMode::DangerousFullAccess,
            allow: vec!["*".to_string()], // wildcard: allow all
            deny: vec![],
            ask: vec![],
            tool_permissions: HashMap::new(),
        }
    }

    /// Create a `PermissionConfig` that allows only read operations.
    pub fn read_only() -> Self {
        Self {
            default_mode: PermissionMode::ReadOnly,
            allow: vec![
                "read_file".to_string(),
                "grep_search".to_string(),
                "glob".to_string(),
                "lsp_query".to_string(),
            ],
            deny: vec![
                "write_file".to_string(),
                "edit_file".to_string(),
                "create_file".to_string(),
                "delete_file".to_string(),
                "bash".to_string(),
                "git_commit".to_string(),
                "git_push".to_string(),
                "run_command".to_string(),
            ],
            ask: vec![],
            tool_permissions: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PermissionConfig::default();
        assert_eq!(config.default_mode, PermissionMode::WorkspaceWrite);
        assert!(!config.allow.is_empty());
        assert!(config.deny.is_empty());
        assert!(config.tool_permissions.contains_key("read_file"));
        assert_eq!(
            config.tool_permissions.get("read_file"),
            Some(&PermissionMode::ReadOnly)
        );
    }

    #[test]
    fn test_permissive_config() {
        let config = PermissionConfig::permissive();
        assert_eq!(config.default_mode, PermissionMode::DangerousFullAccess);
        assert!(config.allow.contains(&"*".to_string()));
    }

    #[test]
    fn test_read_only_config() {
        let config = PermissionConfig::read_only();
        assert_eq!(config.default_mode, PermissionMode::ReadOnly);
        assert!(config.deny.contains(&"bash".to_string()));
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = PermissionConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: PermissionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, config);
    }

    #[test]
    fn test_config_with_custom_tool_permissions() {
        let config = PermissionConfig {
            default_mode: PermissionMode::ReadOnly,
            allow: vec![],
            deny: vec!["curl".to_string()],
            ask: vec!["bash".to_string()],
            tool_permissions: HashMap::from([
                ("custom_tool".to_string(), PermissionMode::DangerousFullAccess),
            ]),
        };
        assert_eq!(
            config.tool_permissions.get("custom_tool"),
            Some(&PermissionMode::DangerousFullAccess)
        );
    }
}
