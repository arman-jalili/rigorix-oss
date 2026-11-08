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

/// Why a host's permission-mode string could not be turned into a mode.
///
/// #1487: the hosts used to absorb an unrecognized value into the *permissive*
/// default — `_ => PermissionMode::WorkspaceWrite` — so a typo, or a value from
/// a newer/older spelling, silently became "everything in the workspace is
/// allowed". A three-valued setting with a two-valued parser is a setting that
/// lies about what it accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionModeParseError {
    /// The value as supplied, for the error message.
    pub value: String,
}

impl std::fmt::Display for PermissionModeParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "permission mode {:?} is not recognized — expected one of {}. An unrecognized \
             value is refused rather than treated as the permissive default (#1487).",
            self.value,
            PermissionMode::ACCEPTED_VALUES,
        )
    }
}

impl std::error::Error for PermissionModeParseError {}

impl PermissionMode {
    /// The values operators may write, for error messages and docs.
    pub const ACCEPTED_VALUES: &'static str = "read_only | workspace_write | danger_full_access";

    /// Parse the value a host supplies (`RIGORIX_PERMISSION_MODE` / `rigorix.toml`).
    ///
    /// Accepts the engine spelling and the `action.yml` spelling
    /// (`dangerous_full_access`). Numeric prefixes like `2` are *not* accepted:
    /// guessing at an operator's intent is how a mode becomes its neighbour.
    pub fn parse_host_value(raw: &str) -> Result<Self, PermissionModeParseError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "read_only" | "readonly" => Ok(PermissionMode::ReadOnly),
            "workspace_write" | "workspacewrite" => Ok(PermissionMode::WorkspaceWrite),
            "danger_full_access" | "dangerous_full_access" | "dangerousfullaccess" => {
                Ok(PermissionMode::DangerousFullAccess)
            }
            other => Err(PermissionModeParseError {
                value: other.to_string(),
            }),
        }
    }

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
