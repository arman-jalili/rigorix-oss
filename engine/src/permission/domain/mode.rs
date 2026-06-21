//! PermissionMode — three-tier permission hierarchy.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md#mode
//! Implements: Contract Freeze — PermissionMode enum
//! Issue: issue-contract-freeze
//!
//! Defines the three-tier permission mode that controls which tools
//! can be executed. The active mode caps the maximum risk level a tool
//! can access. Tools requesting a higher `required_mode` than the
//! active mode are denied.
//!
//! # Contract (Frozen)
//! - Ord derives from the discriminant order: ReadOnly < WorkspaceWrite < DangerousFullAccess
//! - Serde serialization uses snake_case
//! - No additional variants without explicit contract change approval

use serde::{Deserialize, Serialize};
use std::fmt;

/// Three-tier permission mode hierarchy.
///
/// Each mode is more permissive than the previous one:
/// - `ReadOnly` — Only read operations (file reads, grep, git log, read-only bash).
/// - `WorkspaceWrite` — Read + write within the workspace boundary.
/// - `DangerousFullAccess` — No restrictions.
///
/// The `Ord` derives from discriminant values: ReadOnly(0) < WorkspaceWrite(1) < DangerousFullAccess(2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum PermissionMode {
    /// Only read operations: file_read, grep, glob, git_log, lsp_query.
    /// Bash: only read-only commands (ls, cat, grep, find, etc.).
    #[serde(rename = "read_only")]
    ReadOnly = 0,

    /// Read + write operations within the workspace boundary.
    /// Bash: all commands allowed (subject to tool policy).
    #[serde(rename = "workspace_write")]
    #[default]
    WorkspaceWrite = 1,

    /// No restrictions — full system access. Use with extreme caution.
    #[serde(rename = "danger_full_access")]
    DangerousFullAccess = 2,
}

impl PermissionMode {
    /// Returns the string representation of this mode.
    pub fn as_str(&self) -> &'static str {
        match self {
            PermissionMode::ReadOnly => "read_only",
            PermissionMode::WorkspaceWrite => "workspace_write",
            PermissionMode::DangerousFullAccess => "danger_full_access",
        }
    }

    /// Returns `true` if this mode allows read operations.
    pub fn can_read(&self) -> bool {
        true // All modes allow reads
    }

    /// Returns `true` if this mode allows write operations within workspace.
    pub fn can_write_within_workspace(&self) -> bool {
        matches!(
            self,
            PermissionMode::WorkspaceWrite | PermissionMode::DangerousFullAccess
        )
    }

    /// Returns `true` if this mode allows write operations outside workspace.
    pub fn can_write_outside_workspace(&self) -> bool {
        matches!(self, PermissionMode::DangerousFullAccess)
    }

    /// Returns `true` if this mode allows destructive operations.
    pub fn can_execute_destructive(&self) -> bool {
        matches!(self, PermissionMode::DangerousFullAccess)
    }

    /// Returns `true` if this mode allows package management commands.
    pub fn can_manage_packages(&self) -> bool {
        matches!(
            self,
            PermissionMode::WorkspaceWrite | PermissionMode::DangerousFullAccess
        )
    }

    /// Returns `true` if this mode allows network operations.
    pub fn can_use_network(&self) -> bool {
        matches!(self, PermissionMode::DangerousFullAccess)
    }

    /// Returns the minimum mode required for the given capability.
    pub fn required_for_capability(capability: &str) -> Self {
        match capability {
            "read" | "grep" | "query" => PermissionMode::ReadOnly,
            "write_within_workspace" | "edit" | "build" | "package_management" | "git_commit" => {
                PermissionMode::WorkspaceWrite
            }
            "write_outside_workspace"
            | "destructive"
            | "network"
            | "process_management"
            | "system_admin"
            | "git_push" => PermissionMode::DangerousFullAccess,
            _ => PermissionMode::WorkspaceWrite, // safe default
        }
    }
}

impl fmt::Display for PermissionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ordering() {
        assert!(PermissionMode::ReadOnly < PermissionMode::WorkspaceWrite);
        assert!(PermissionMode::WorkspaceWrite < PermissionMode::DangerousFullAccess);
        assert!(PermissionMode::ReadOnly < PermissionMode::DangerousFullAccess);
    }

    #[test]
    fn test_capabilities() {
        assert!(PermissionMode::ReadOnly.can_read());
        assert!(!PermissionMode::ReadOnly.can_write_within_workspace());
        assert!(!PermissionMode::ReadOnly.can_write_outside_workspace());
        assert!(!PermissionMode::ReadOnly.can_execute_destructive());

        assert!(PermissionMode::WorkspaceWrite.can_read());
        assert!(PermissionMode::WorkspaceWrite.can_write_within_workspace());
        assert!(!PermissionMode::WorkspaceWrite.can_write_outside_workspace());
        assert!(!PermissionMode::WorkspaceWrite.can_execute_destructive());

        assert!(PermissionMode::DangerousFullAccess.can_read());
        assert!(PermissionMode::DangerousFullAccess.can_write_within_workspace());
        assert!(PermissionMode::DangerousFullAccess.can_write_outside_workspace());
        assert!(PermissionMode::DangerousFullAccess.can_execute_destructive());
    }

    #[test]
    fn test_serde_roundtrip() {
        for mode in &[
            PermissionMode::ReadOnly,
            PermissionMode::WorkspaceWrite,
            PermissionMode::DangerousFullAccess,
        ] {
            let json = serde_json::to_string(mode).unwrap();
            let deserialized: PermissionMode = serde_json::from_str(&json).unwrap();
            assert_eq!(*mode, deserialized);
        }
    }

    #[test]
    fn test_serde_snake_case() {
        assert_eq!(
            serde_json::to_string(&PermissionMode::ReadOnly).unwrap(),
            "\"read_only\""
        );
        assert_eq!(
            serde_json::to_string(&PermissionMode::WorkspaceWrite).unwrap(),
            "\"workspace_write\""
        );
        assert_eq!(
            serde_json::to_string(&PermissionMode::DangerousFullAccess).unwrap(),
            "\"danger_full_access\""
        );
    }

    #[test]
    fn test_as_str() {
        assert_eq!(PermissionMode::ReadOnly.as_str(), "read_only");
        assert_eq!(PermissionMode::WorkspaceWrite.as_str(), "workspace_write");
        assert_eq!(
            PermissionMode::DangerousFullAccess.as_str(),
            "danger_full_access"
        );
    }

    #[test]
    fn test_default_is_workspace_write() {
        assert_eq!(PermissionMode::default(), PermissionMode::WorkspaceWrite);
    }

    #[test]
    fn test_required_for_capability() {
        assert_eq!(
            PermissionMode::required_for_capability("write_within_workspace"),
            PermissionMode::WorkspaceWrite
        );
        assert_eq!(
            PermissionMode::required_for_capability("destructive"),
            PermissionMode::DangerousFullAccess
        );
        assert_eq!(
            PermissionMode::required_for_capability("grep"),
            PermissionMode::ReadOnly
        );
        assert_eq!(
            PermissionMode::required_for_capability("unknown"),
            PermissionMode::WorkspaceWrite
        );
    }
}
