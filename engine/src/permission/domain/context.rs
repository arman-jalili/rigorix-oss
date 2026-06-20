//! PermissionContext — override context for permission decisions.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md#context
//! Implements: Contract Freeze — PermissionContext struct
//! Issue: issue-contract-freeze
//!
//! Provides temporal or conditional override context for permission
//! decisions. Hooks or user interactions can set override context
//! that modifies how the permission policy evaluates requests.
//!
//! # Contract (Frozen)
//! - Overrides have clear precedence rules
//! - Context is ephemeral (not persisted)
//! - `duration_secs` = 0 means single-use (expires after one check)

use serde::{Deserialize, Serialize};

use super::PermissionMode;

/// Override context that modifies permission policy decisions.
///
/// Context can be set by hooks (e.g., "user approved elevation via prompt")
/// or by the user explicitly (e.g., `--permission-mode workspace-write`).
///
/// # Precedence
/// 1. If `elevated_mode` is `Some`, it overrides the active mode for
///    the specified duration or single check.
/// 2. If `temporary_bypass` is `true`, all permission checks are bypassed
///    for the specified duration or single check.
/// 3. Otherwise, normal policy evaluation applies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionContext {
    /// Temporarily elevate to this permission mode.
    /// If `Some`, overrides the active mode for the duration.
    pub elevated_mode: Option<PermissionMode>,

    /// Duration in seconds for which the override is active.
    /// 0 means single-use (expires after one permission check).
    pub duration_secs: u64,

    /// If true, all permission checks are bypassed temporarily.
    pub temporary_bypass: bool,

    /// Human-readable reason for the override (for audit).
    pub reason: Option<String>,

    /// Source of the override (e.g., "hook:bash_permission_prompt", "cli:--permission-mode").
    pub source: Option<String>,
}

impl PermissionContext {
    /// Create a new `PermissionContext` with no overrides.
    pub fn none() -> Self {
        Self {
            elevated_mode: None,
            duration_secs: 0,
            temporary_bypass: false,
            reason: None,
            source: None,
        }
    }

    /// Create a `PermissionContext` that elevates to a specific mode.
    pub fn elevate(mode: PermissionMode, reason: &str) -> Self {
        Self {
            elevated_mode: Some(mode),
            duration_secs: 0, // single-use by default
            temporary_bypass: false,
            reason: Some(reason.to_string()),
            source: None,
        }
    }

    /// Create a `PermissionContext` that bypasses all checks.
    pub fn bypass(reason: &str) -> Self {
        Self {
            elevated_mode: None,
            duration_secs: 0,
            temporary_bypass: true,
            reason: Some(reason.to_string()),
            source: None,
        }
    }

    /// Returns `true` if this context has any active override.
    pub fn has_override(&self) -> bool {
        self.elevated_mode.is_some() || self.temporary_bypass
    }
}

impl Default for PermissionContext {
    fn default() -> Self {
        Self::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_none_context() {
        let ctx = PermissionContext::none();
        assert!(!ctx.has_override());
        assert!(ctx.elevated_mode.is_none());
        assert!(!ctx.temporary_bypass);
    }

    #[test]
    fn test_elevate_context() {
        let ctx =
            PermissionContext::elevate(PermissionMode::DangerousFullAccess, "Need to install deps");
        assert!(ctx.has_override());
        assert_eq!(ctx.elevated_mode, Some(PermissionMode::DangerousFullAccess));
        assert_eq!(ctx.reason.as_deref(), Some("Need to install deps"));
    }

    #[test]
    fn test_bypass_context() {
        let ctx = PermissionContext::bypass("User approved full access");
        assert!(ctx.has_override());
        assert!(ctx.temporary_bypass);
        assert_eq!(ctx.reason.as_deref(), Some("User approved full access"));
    }

    #[test]
    fn test_default_is_none() {
        let ctx = PermissionContext::default();
        assert!(!ctx.has_override());
    }

    #[test]
    fn test_serde_roundtrip() {
        let ctx = PermissionContext::elevate(PermissionMode::DangerousFullAccess, "test");
        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: PermissionContext = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ctx);
    }
}
