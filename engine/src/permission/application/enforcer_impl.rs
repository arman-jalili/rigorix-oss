//! Concrete implementation of the PermissionEnforcer service.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md#application
//! Implements: ISSUE-PERMISSION-ENFORCER-3 — PermissionEnforcer runtime enforcement
//! Issue: issue-permissionenforcer
//!
//! Provides the concrete `PermissionEnforcerImpl` that delegates to
//! `PermissionPolicy` for authorization, checks workspace boundaries
//! for file writes, and classifies bash commands for mode-aware gating.
//!
//! # Thread Safety
//! - The enforcer wraps a `PermissionPolicy` behind a `RwLock`
//! - All async methods are safe to call from multiple tasks
//! - Mode changes are atomic

use async_trait::async_trait;
use std::path::Path;
use std::sync::RwLock;

use crate::permission::application::enforcer::PermissionEnforcer;
use crate::permission::domain::{
    BashClassifier, PermissionContext, PermissionError, PermissionMode,
    PermissionOutcome, PermissionPolicy,
};

/// Concrete implementation of the `PermissionEnforcer` trait.
///
/// Provides three distinct checks:
/// 1. General tool gating via `PermissionPolicy`
/// 2. Workspace boundary check for file writes
/// 3. Bash command classification via `BashClassifier`
pub struct PermissionEnforcerImpl {
    /// The permission policy used for authorization decisions.
    policy: RwLock<PermissionPolicy>,

    /// The workspace root path used for boundary checks.
    #[allow(dead_code)]
    workspace_root: RwLock<String>,
}

impl PermissionEnforcerImpl {
    /// Create a new `PermissionEnforcerImpl` with the given policy and workspace root.
    pub fn new(policy: PermissionPolicy, workspace_root: &str) -> Self {
        Self {
            policy: RwLock::new(policy),
            workspace_root: RwLock::new(workspace_root.to_string()),
        }
    }

    /// Helper: check if a path is within the workspace boundary.
    ///
    /// Canonicalizes both paths (resolving symlinks, relative paths)
    /// to prevent symlink escape attacks.
    fn is_within_workspace(path: &str, workspace_root: &str) -> bool {
        let canonical_path = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => {
                // If the path doesn't exist yet (write to new file),
                // resolve its parent directory
                let parent = Path::new(path).parent();
                match parent.and_then(|p| std::fs::canonicalize(p).ok()) {
                    Some(par) => par.join(
                        Path::new(path)
                            .file_name()
                            .unwrap_or_default(),
                    ),
                    None => Path::new(path).to_path_buf(),
                }
            }
        };

        let canonical_root = match std::fs::canonicalize(workspace_root) {
            Ok(r) => r,
            Err(_) => Path::new(workspace_root).to_path_buf(),
        };

        canonical_path.starts_with(&canonical_root)
    }
}

#[async_trait]
impl PermissionEnforcer for PermissionEnforcerImpl {
    async fn check(
        &self,
        tool_name: &str,
        input: &str,
        context: Option<&PermissionContext>,
    ) -> PermissionOutcome {
        let policy = self
            .policy
            .read()
            .map_err(|e| {
                PermissionError::InvalidState {
                    detail: format!("Failed to read policy: {}", e),
                }
            });

        let policy = match policy {
            Ok(p) => p,
            Err(_) => {
                return PermissionOutcome::Denied {
                    tool: tool_name.to_string(),
                    active_mode: "unknown".to_string(),
                    required_mode: "unknown".to_string(),
                    reason: "Policy lock poisoned".to_string(),
                };
            }
        };

        policy.authorize(tool_name, input, context, None)
    }

    async fn check_file_write(
        &self,
        path: &str,
        workspace_root: &str,
        context: Option<&PermissionContext>,
    ) -> PermissionOutcome {
        let policy = self
            .policy
            .read()
            .map_err(|e| {
                PermissionError::InvalidState {
                    detail: format!("Failed to read policy: {}", e),
                }
            });

        let policy = match policy {
            Ok(p) => p,
            Err(_) => {
                return PermissionOutcome::Denied {
                    tool: "write_file".to_string(),
                    active_mode: "unknown".to_string(),
                    required_mode: "unknown".to_string(),
                    reason: "Policy lock poisoned".to_string(),
                };
            }
        };

        // Check context override first
        let effective_mode = context
            .and_then(|c| c.elevated_mode)
            .unwrap_or_else(|| policy.active_mode());

        if context.map(|c| c.temporary_bypass).unwrap_or(false) {
            return PermissionOutcome::Allowed;
        }

        match effective_mode {
            PermissionMode::ReadOnly => PermissionOutcome::Denied {
                tool: "write_file".to_string(),
                active_mode: effective_mode.as_str().to_string(),
                required_mode: PermissionMode::WorkspaceWrite.as_str().to_string(),
                reason: "file writes are not allowed in read_only mode".to_string(),
            },
            PermissionMode::WorkspaceWrite => {
                let within_workspace = Self::is_within_workspace(path, workspace_root);
                if within_workspace {
                    PermissionOutcome::Allowed
                } else {
                    PermissionOutcome::Denied {
                        tool: "write_file".to_string(),
                        active_mode: effective_mode.as_str().to_string(),
                        required_mode: PermissionMode::DangerousFullAccess.as_str().to_string(),
                        reason: format!(
                            "path '{}' is outside workspace root '{}'",
                            path, workspace_root
                        ),
                    }
                }
            }
            PermissionMode::DangerousFullAccess => PermissionOutcome::Allowed,
        }
    }

    async fn check_bash(
        &self,
        command: &str,
        context: Option<&PermissionContext>,
    ) -> PermissionOutcome {
        let policy = self
            .policy
            .read()
            .map_err(|e| {
                PermissionError::InvalidState {
                    detail: format!("Failed to read policy: {}", e),
                }
            });

        let policy = match policy {
            Ok(p) => p,
            Err(_) => {
                return PermissionOutcome::Denied {
                    tool: "bash".to_string(),
                    active_mode: "unknown".to_string(),
                    required_mode: "unknown".to_string(),
                    reason: "Policy lock poisoned".to_string(),
                };
            }
        };

        // Check context override first
        let effective_mode = context
            .and_then(|c| c.elevated_mode)
            .unwrap_or_else(|| policy.active_mode());

        if context.map(|c| c.temporary_bypass).unwrap_or(false) {
            return PermissionOutcome::Allowed;
        }

        match effective_mode {
            PermissionMode::ReadOnly => {
                let classification = BashClassifier::classify(command);
                if classification.is_read_only() {
                    PermissionOutcome::Allowed
                } else {
                    PermissionOutcome::Denied {
                        tool: "bash".to_string(),
                        active_mode: effective_mode.as_str().to_string(),
                        required_mode: BashClassifier::required_mode_for_intent(classification)
                            .as_str()
                            .to_string(),
                        reason: format!(
                            "'{}' is classified as '{}' — not allowed in read_only mode",
                            command, classification
                        ),
                    }
                }
            }
            PermissionMode::WorkspaceWrite | PermissionMode::DangerousFullAccess => {
                PermissionOutcome::Allowed
            }
        }
    }

    fn active_mode(&self) -> PermissionMode {
        self.policy
            .read()
            .map(|p| p.active_mode())
            .unwrap_or(PermissionMode::ReadOnly)
    }

    fn set_active_mode(&self, mode: PermissionMode) {
        if let Ok(mut policy) = self.policy.write() {
            policy.set_active_mode(mode);
        }
    }

    async fn reload_policy(&self) -> Result<(), PermissionError> {
        // Reload from default config.
        // Preserves the current active mode but resets rules to defaults.
        let config = crate::permission::domain::PermissionConfig::default();
        let current_mode = self.active_mode();
        let new_policy = PermissionPolicy::new(
            current_mode,
            config.tool_permissions,
            config.allow,
            config.deny,
            config.ask,
        );

        let mut policy = self.policy.write().map_err(|e| {
            PermissionError::InvalidState {
                detail: format!("Failed to write policy: {}", e),
            }
        })?;
        *policy = new_policy;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::domain::PermissionPolicy;
    

    /// Helper: create a test enforcer with workspace_write mode.
    fn test_enforcer(workspace_root: &str) -> PermissionEnforcerImpl {
        let policy = PermissionPolicy::default_with_mode(PermissionMode::WorkspaceWrite);
        PermissionEnforcerImpl::new(policy, workspace_root)
    }

    /// Helper: create a read-only enforcer.
    fn read_only_enforcer(workspace_root: &str) -> PermissionEnforcerImpl {
        let policy = PermissionPolicy::default_with_mode(PermissionMode::ReadOnly);
        PermissionEnforcerImpl::new(policy, workspace_root)
    }

    /// Helper: create a dangerous enforcer.
    fn dangerous_enforcer(workspace_root: &str) -> PermissionEnforcerImpl {
        let policy = PermissionPolicy::default_with_mode(PermissionMode::DangerousFullAccess);
        PermissionEnforcerImpl::new(policy, workspace_root)
    }

    // -----------------------------------------------------------------------
    // check() tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_check_allows_read_file_in_workspace_write() {
        let enforcer = test_enforcer("/tmp");
        let outcome = enforcer.check("read_file", "test.txt", None).await;
        assert!(outcome.is_allowed(), "read_file should be allowed in workspace_write");
    }

    #[tokio::test]
    async fn test_check_denies_write_file_in_read_only() {
        let enforcer = read_only_enforcer("/tmp");
        let outcome = enforcer.check("write_file", "/tmp/test.txt", None).await;
        assert!(outcome.is_denied(), "write_file should be denied in read_only");
    }

    #[tokio::test]
    async fn test_check_allows_with_context_elevation() {
        let enforcer = read_only_enforcer("/tmp");
        let ctx = PermissionContext::elevate(PermissionMode::WorkspaceWrite, "Need to write");
        let outcome = enforcer.check("write_file", "/tmp/test.txt", Some(&ctx)).await;
        assert!(outcome.is_allowed(), "elevated context should allow write_file");
    }

    #[tokio::test]
    async fn test_check_allows_with_context_bypass() {
        let enforcer = read_only_enforcer("/tmp");
        let ctx = PermissionContext::bypass("Emergency");
        let outcome = enforcer.check("write_file", "/tmp/test.txt", Some(&ctx)).await;
        assert!(outcome.is_allowed(), "bypass context should allow any tool");
    }

    #[tokio::test]
    async fn test_check_denies_delete_file_in_workspace_write() {
        let enforcer = test_enforcer("/tmp");
        let outcome = enforcer.check("delete_file", "/tmp/x", None).await;
        assert!(outcome.is_denied(), "delete_file requires dangerous_full_access");
    }

    // -----------------------------------------------------------------------
    // check_file_write() tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_file_write_allowed_in_workspace_write() {
        // Use a temp dir as the workspace root
        let dir = std::env::temp_dir();
        let root = dir.to_str().unwrap_or("/tmp");
        let enforcer = test_enforcer(root);
        let outcome = enforcer.check_file_write(root, root, None).await;
        assert!(outcome.is_allowed(), "write inside workspace should be allowed");
    }

    #[tokio::test]
    async fn test_file_write_denied_in_read_only() {
        let enforcer = read_only_enforcer("/tmp");
        let outcome = enforcer.check_file_write("/tmp/test.txt", "/tmp", None).await;
        assert!(outcome.is_denied(), "file write should be denied in read_only");
    }

    #[tokio::test]
    async fn test_file_write_allowed_in_dangerous() {
        let enforcer = dangerous_enforcer("/tmp");
        let outcome = enforcer.check_file_write("/etc/passwd", "/tmp", None).await;
        assert!(outcome.is_allowed(), "file write should be allowed in dangerous_full_access");
    }

    #[tokio::test]
    async fn test_file_write_denied_outside_workspace() {
        let enforcer = test_enforcer("/tmp");
        // A path that's clearly outside /tmp
        let outcome = enforcer
            .check_file_write("/etc/passwd", "/tmp", None)
            .await;
        assert!(outcome.is_denied(), "write outside workspace should be denied");
        assert!(outcome.to_string().contains("outside workspace"));
    }

    #[tokio::test]
    async fn test_file_write_context_elevation() {
        let enforcer = read_only_enforcer("/tmp");
        let ctx = PermissionContext::elevate(PermissionMode::WorkspaceWrite, "Need to write");
        let outcome = enforcer
            .check_file_write("/tmp/test.txt", "/tmp", Some(&ctx))
            .await;
        assert!(outcome.is_allowed(), "elevated context should allow file write");
    }

    #[tokio::test]
    async fn test_file_write_context_bypass() {
        let enforcer = read_only_enforcer("/tmp");
        let ctx = PermissionContext::bypass("Emergency");
        let outcome = enforcer
            .check_file_write("/etc/passwd", "/tmp", Some(&ctx))
            .await;
        assert!(outcome.is_allowed(), "bypass should allow any file write");
    }

    // -----------------------------------------------------------------------
    // check_bash() tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_bash_read_only_allows_ls() {
        let enforcer = read_only_enforcer("/tmp");
        let outcome = enforcer.check_bash("ls -la /tmp", None).await;
        assert!(outcome.is_allowed(), "ls should be allowed in read_only");
    }

    #[tokio::test]
    async fn test_bash_read_only_denies_rm() {
        let enforcer = read_only_enforcer("/tmp");
        let outcome = enforcer.check_bash("rm -rf /", None).await;
        assert!(outcome.is_denied(), "rm should be denied in read_only");
        assert!(outcome.to_string().contains("destructive"));
    }

    #[tokio::test]
    async fn test_bash_workspace_write_allows_all() {
        let enforcer = test_enforcer("/tmp");
        let outcome = enforcer.check_bash("rm -rf /tmp", None).await;
        assert!(outcome.is_allowed(), "bash should be allowed in workspace_write");
    }

    #[tokio::test]
    async fn test_bash_dangerous_allows_all() {
        let enforcer = dangerous_enforcer("/tmp");
        let outcome = enforcer.check_bash("sudo rm -rf /", None).await;
        assert!(outcome.is_allowed(), "bash should be allowed in dangerous_full_access");
    }

    #[tokio::test]
    async fn test_bash_context_elevation() {
        let enforcer = read_only_enforcer("/tmp");
        let ctx = PermissionContext::elevate(PermissionMode::WorkspaceWrite, "Need bash");
        let outcome = enforcer.check_bash("rm file", Some(&ctx)).await;
        assert!(outcome.is_allowed(), "elevated context should allow bash");
    }

    #[tokio::test]
    async fn test_bash_read_only_allows_grep() {
        let enforcer = read_only_enforcer("/tmp");
        let outcome = enforcer.check_bash("grep pattern file.txt", None).await;
        assert!(outcome.is_allowed(), "grep should be allowed in read_only");
    }

    #[tokio::test]
    async fn test_bash_read_only_denies_cargo_build() {
        let enforcer = read_only_enforcer("/tmp");
        let outcome = enforcer.check_bash("cargo build", None).await;
        assert!(outcome.is_denied(), "cargo build should be denied in read_only");
        assert!(outcome.to_string().contains("package_management"));
    }

    // -----------------------------------------------------------------------
    // Mode management tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_active_mode() {
        let enforcer = test_enforcer("/tmp");
        assert_eq!(enforcer.active_mode(), PermissionMode::WorkspaceWrite);
    }

    #[tokio::test]
    async fn test_set_active_mode() {
        let enforcer = test_enforcer("/tmp");
        enforcer.set_active_mode(PermissionMode::ReadOnly);
        assert_eq!(enforcer.active_mode(), PermissionMode::ReadOnly);
    }

    #[tokio::test]
    async fn test_set_active_mode_affects_checks() {
        let enforcer = test_enforcer("/tmp");
        // Start in workspace_write — bash is allowed
        assert!(enforcer.check_bash("rm file", None).await.is_allowed());

        // Switch to read_only
        enforcer.set_active_mode(PermissionMode::ReadOnly);
        assert!(enforcer.check_bash("rm file", None).await.is_denied());
    }

    // -----------------------------------------------------------------------
    // reload_policy tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_reload_policy() {
        let enforcer = test_enforcer("/tmp");
        let result = enforcer.reload_policy().await;
        assert!(result.is_ok(), "reload should succeed");
    }

    #[tokio::test]
    async fn test_reload_policy_keeps_mode() {
        let enforcer = test_enforcer("/tmp");
        enforcer.set_active_mode(PermissionMode::ReadOnly);
        enforcer.reload_policy().await.unwrap();
        assert_eq!(enforcer.active_mode(), PermissionMode::ReadOnly);
    }

    // -----------------------------------------------------------------------
    // Edge case tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_check_empty_tool_name() {
        let enforcer = test_enforcer("/tmp");
        let outcome = enforcer.check("", "", None).await;
        // Empty tool names should get the default mode requirement (WorkspaceWrite)
        assert!(outcome.is_allowed(), "empty tool should use default mode");
    }

    #[tokio::test]
    async fn test_file_write_root_as_workspace() {
        let enforcer = test_enforcer("/");
        let outcome = enforcer.check_file_write("/tmp/test.txt", "/", None).await;
        assert!(outcome.is_allowed(), "write inside root workspace should be allowed");
    }

    #[tokio::test]
    async fn test_bash_empty_command() {
        let enforcer = read_only_enforcer("/tmp");
        let outcome = enforcer.check_bash("", None).await;
        assert!(outcome.is_denied(), "empty command should be denied in read_only");
    }

    #[tokio::test]
    async fn test_concurrent_checks() {
        let enforcer = std::sync::Arc::new(test_enforcer("/tmp"));
        let mut handles = Vec::new();

        for _ in 0..10 {
            let e = enforcer.clone();
            handles.push(tokio::spawn(async move {
                let r1 = e.check("read_file", "test.txt", None).await;
                let r2 = e.check_bash("ls -la", None).await;
                let r3 = e.check_file_write("/tmp/x", "/tmp", None).await;
                assert!(r1.is_allowed());
                assert!(r2.is_allowed());
                assert!(r3.is_allowed());
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_concurrent_mode_changes() {
        let enforcer = std::sync::Arc::new(test_enforcer("/tmp"));
        let mut handles = Vec::new();

        for i in 0..10 {
            let e = enforcer.clone();
            handles.push(tokio::spawn(async move {
                if i % 2 == 0 {
                    e.set_active_mode(PermissionMode::ReadOnly);
                } else {
                    e.set_active_mode(PermissionMode::WorkspaceWrite);
                }
                let _mode = e.active_mode();
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }
    }
}
