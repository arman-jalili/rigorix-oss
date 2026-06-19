//! PermissionEnforcer — application service trait for permission gating.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md#enforcer
//! Implements: Contract Freeze — PermissionEnforcer trait
//! Issue: issue-contract-freeze
//!
//! Defines the application service interface for the permission enforcer.
//! The PermissionEnforcer sits between the execution engine and tool
//! execution, gating every tool call based on the active permission mode,
//! workspace boundaries, and bash command classification.
//!
//! # Contract (Frozen)
//! - All methods return `PermissionOutcome` for structured feedback
//! - Workspace checks require an explicit `workspace_root` parameter
//! - Bash classification is built into the `check_bash` method
//! - Context overrides modify evaluation behavior

use async_trait::async_trait;

use crate::permission::domain::{PermissionContext, PermissionError, PermissionMode, PermissionOutcome};

/// Central permission enforcement service.
///
/// The PermissionEnforcer provides three distinct checks:
///
/// 1. **General tool gating** — checks if the tool is allowed by policy
///    based on the active permission mode and configured rules.
///
/// 2. **File write boundary check** — for write_file/edit_file tools,
///    verifies the target path is within the workspace root.
///
/// 3. **Bash command classification** — for bash tools, classifies the
///    command intent and gates accordingly.
///
/// # Integration
///
/// The enforcer is called by the execution engine (or orchestrator)
/// before every tool invocation. It returns `PermissionOutcome::Allowed`
/// or `PermissionOutcome::Denied` with structured reasoning that can be
/// fed back to the LLM.
#[async_trait]
pub trait PermissionEnforcer: Send + Sync {
    /// General tool permission check.
    ///
    /// Evaluates the tool call against the active permission policy.
    /// Returns `Allowed` if the tool is permitted, `Denied` with a
    /// structured reason otherwise.
    async fn check(
        &self,
        tool_name: &str,
        input: &str,
        context: Option<&PermissionContext>,
    ) -> PermissionOutcome;

    /// Workspace boundary check for file write operations.
    ///
    /// Verifies that the target path is within the workspace root.
    /// In ReadOnly mode, all writes are denied.
    /// In WorkspaceWrite mode, writes outside the workspace are denied.
    /// In DangerousFullAccess mode, all writes are allowed.
    async fn check_file_write(
        &self,
        path: &str,
        workspace_root: &str,
        context: Option<&PermissionContext>,
    ) -> PermissionOutcome;

    /// Bash command classification and permission check.
    ///
    /// Classifies the bash command using `BashClassifier` and gates
    /// based on the active mode. In ReadOnly mode, only read-only
    /// commands are allowed.
    async fn check_bash(
        &self,
        command: &str,
        context: Option<&PermissionContext>,
    ) -> PermissionOutcome;

    /// Get the current active permission mode.
    fn active_mode(&self) -> PermissionMode;

    /// Set the active permission mode.
    fn set_active_mode(&self, mode: PermissionMode);

    /// Reload the permission policy configuration.
    async fn reload_policy(&self) -> Result<(), PermissionError>;
}
