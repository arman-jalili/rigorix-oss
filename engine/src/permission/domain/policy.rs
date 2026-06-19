//! PermissionPolicy — authorization logic based on active mode and rules.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md#policy
//! Implements: Contract Freeze — PermissionPolicy authorization logic
//! Issue: issue-contract-freeze
//!
//! Defines the permission policy that evaluates tool authorization
//! requests against the active mode, allow/deny/ask rules, and
//! per-tool permission requirements.
//!
//! # Contract (Frozen)
//! - Authorization follows a strict 6-step pipeline (deny rules → allow → mode → ask → default)
//! - Allow rules override mode restrictions (but not deny rules)
//! - Deny rules are authoritative and checked first
//! - Ask rules trigger interactive confirmation via `PermissionPrompter`
//! - Unknown tools default to `WorkspaceWrite` mode (safe default)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::context::PermissionContext;
use super::mode::PermissionMode;
use super::outcome::PermissionOutcome;
use super::prompter::PermissionPrompter;

/// Authorization policy that controls tool access based on permission mode.
///
/// The policy evaluates tool invocation requests through a structured
/// pipeline:
///
/// 1. **Deny rules** — if the tool matches an explicit deny rule, deny immediately
/// 2. **Allow rules** — if the tool matches an explicit allow rule, allow immediately
/// 3. **Mode check** — check if the active mode is sufficient for the tool's requirement
/// 4. **Context check** — check for temporary overrides (elevation, bypass)
/// 5. **Ask rules** — if the tool matches an ask rule, prompt for confirmation
/// 6. **Default** — allow (mode check already passed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionPolicy {
    /// The active permission mode.
    active_mode: PermissionMode,

    /// Per-tool permission mode requirements.
    /// Maps tool name to the minimum permission mode required.
    tool_permissions: HashMap<String, PermissionMode>,

    /// Tool names that are always allowed (override mode check).
    allow_rules: Vec<String>,

    /// Tool names that are always denied (checked first, authoritative).
    deny_rules: Vec<String>,

    /// Tool names that require user confirmation.
    ask_rules: Vec<String>,

    /// The default permission mode for tools without an explicit mapping.
    default_required_mode: PermissionMode,
}

impl PermissionPolicy {
    /// Create a new `PermissionPolicy`.
    pub fn new(
        active_mode: PermissionMode,
        tool_permissions: HashMap<String, PermissionMode>,
        allow_rules: Vec<String>,
        deny_rules: Vec<String>,
        ask_rules: Vec<String>,
    ) -> Self {
        Self {
            active_mode,
            tool_permissions,
            allow_rules,
            deny_rules,
            ask_rules,
            default_required_mode: PermissionMode::WorkspaceWrite,
        }
    }

    /// Create a `PermissionPolicy` with the default configuration.
    pub fn default_with_mode(active_mode: PermissionMode) -> Self {
        Self::new(
            active_mode,
            HashMap::from([
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
            vec![
                "read_file".to_string(),
                "grep_search".to_string(),
                "glob".to_string(),
                "lsp_query".to_string(),
            ],
            vec![],
            vec![
                "git_commit".to_string(),
                "git_push".to_string(),
                "bash".to_string(),
            ],
        )
    }

    /// Returns the current active permission mode.
    pub fn active_mode(&self) -> PermissionMode {
        self.active_mode
    }

    /// Set the active permission mode.
    pub fn set_active_mode(&mut self, mode: PermissionMode) {
        self.active_mode = mode;
    }

    /// Get the required permission mode for a tool.
    ///
    /// Returns the explicit mapping if one exists, otherwise the default.
    pub fn required_mode_for(&self, tool_name: &str) -> PermissionMode {
        self.tool_permissions
            .get(tool_name)
            .copied()
            .unwrap_or(self.default_required_mode)
    }

    /// Check if a tool name matches any rule in a given list.
    ///
    /// Supports exact match and wildcard (`*`) matching.
    fn matches_any_rule(&self, tool_name: &str, rules: &[String]) -> bool {
        rules.iter().any(|rule| rule == "*" || rule == tool_name)
    }

    /// Authorize a tool invocation.
    ///
    /// Evaluates the request through the policy pipeline:
    ///
    /// 1. Check explicit deny rules (authoritative)
    /// 2. Check explicit allow rules (override mode)
    /// 3. Check context overrides
    /// 4. Check mode requirement
    /// 5. Check ask rules (prompt for confirmation)
    /// 6. Default: allow
    ///
    /// Returns `PermissionOutcome::Allowed` or `PermissionOutcome::Denied`
    /// with structured reasoning.
    pub fn authorize(
        &self,
        tool_name: &str,
        _tool_input: &str,
        context: Option<&PermissionContext>,
        prompter: Option<&mut dyn PermissionPrompter>,
    ) -> PermissionOutcome {
        let effective_mode = context
            .and_then(|c| c.elevated_mode)
            .unwrap_or(self.active_mode);

        let bypass = context
            .map(|c| c.temporary_bypass)
            .unwrap_or(false);

        if bypass {
            return PermissionOutcome::Allowed;
        }

        // 1. Check explicit deny rules (authoritative — checked first)
        if self.matches_any_rule(tool_name, &self.deny_rules) {
            return PermissionOutcome::Denied {
                tool: tool_name.to_string(),
                active_mode: effective_mode.as_str().to_string(),
                required_mode: self.required_mode_for(tool_name).as_str().to_string(),
                reason: format!("'{}' is explicitly denied by policy", tool_name),
            };
        }

        // 2. Check allow rules (override mode restrictions)
        if self.matches_any_rule(tool_name, &self.allow_rules) {
            if self.matches_any_rule(tool_name, &self.ask_rules) {
                // Even allowed tools may require confirmation if in ask list
                return self.prompt_or_deny(tool_name, _tool_input, effective_mode, prompter);
            }
            return PermissionOutcome::Allowed;
        }

        // 3. Get required mode for this tool
        let required = self.required_mode_for(tool_name);

        // 4. Mode check
        if effective_mode < required {
            return PermissionOutcome::Denied {
                tool: tool_name.to_string(),
                active_mode: effective_mode.as_str().to_string(),
                required_mode: required.as_str().to_string(),
                reason: format!(
                    "'{}' requires '{}' mode, but active mode is '{}'",
                    tool_name,
                    required,
                    effective_mode
                ),
            };
        }

        // 5. Ask rules (prompt for confirmation)
        if self.matches_any_rule(tool_name, &self.ask_rules) {
            return self.prompt_or_deny(tool_name, _tool_input, effective_mode, prompter);
        }

        // 6. Default: allow
        PermissionOutcome::Allowed
    }

    /// Helper: prompt for confirmation or deny if no prompter available.
    fn prompt_or_deny(
        &self,
        tool_name: &str,
        tool_input: &str,
        effective_mode: PermissionMode,
        prompter: Option<&mut dyn PermissionPrompter>,
    ) -> PermissionOutcome {
        if let Some(p) = prompter {
            p.prompt(tool_name, tool_input)
        } else {
            PermissionOutcome::Denied {
                tool: tool_name.to_string(),
                active_mode: effective_mode.as_str().to_string(),
                required_mode: self.required_mode_for(tool_name).as_str().to_string(),
                reason: format!(
                    "'{}' requires confirmation but no interactive prompt is available",
                    tool_name
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::domain::prompter::{AllowAllPrompter, DenyAllPrompter};

    fn read_only_policy() -> PermissionPolicy {
        PermissionPolicy::default_with_mode(PermissionMode::ReadOnly)
    }

    fn workspace_policy() -> PermissionPolicy {
        PermissionPolicy::default_with_mode(PermissionMode::WorkspaceWrite)
    }

    fn dangerous_policy() -> PermissionPolicy {
        PermissionPolicy::default_with_mode(PermissionMode::DangerousFullAccess)
    }

    #[test]
    fn test_read_only_denies_write_file() {
        let policy = read_only_policy();
        let outcome = policy.authorize("write_file", "/tmp/test", None, None);
        assert!(outcome.is_denied());
        assert!(outcome.to_string().contains("write_file"));
        assert!(outcome.to_string().contains("read_only"));
    }

    #[test]
    fn test_read_only_allows_read_file() {
        let policy = read_only_policy();
        let outcome = policy.authorize("read_file", "foo.txt", None, None);
        assert!(outcome.is_allowed());
    }

    #[test]
    fn test_workspace_write_allows_write_file() {
        let policy = workspace_policy();
        let outcome = policy.authorize("write_file", "foo.txt", None, None);
        assert!(outcome.is_allowed());
    }

    #[test]
    fn test_workspace_write_denies_delete_file() {
        let policy = workspace_policy();
        let outcome = policy.authorize("delete_file", "/tmp/x", None, None);
        assert!(outcome.is_denied());
    }

    #[test]
    fn test_dangerous_allows_everything() {
        let policy = dangerous_policy();
        assert!(policy.authorize("write_file", "/tmp/x", None, None).is_allowed());
        assert!(policy.authorize("delete_file", "/tmp/x", None, None).is_allowed());
        // git_push is in ask rules so needs a prompter, but DangerousFullAccess
        // passes the mode check so it should be allowed if prompter approves
        let mut prompter = AllowAllPrompter::new();
        assert!(policy.authorize("git_push", "origin main", None, Some(&mut prompter)).is_allowed());
    }

    #[test]
    fn test_deny_rules_authoritative() {
        let mut policy = workspace_policy();
        policy.deny_rules = vec!["write_file".to_string()];
        let outcome = policy.authorize("write_file", "/tmp/x", None, None);
        assert!(outcome.is_denied());
        assert!(outcome.to_string().contains("explicitly denied"));
    }

    #[test]
    fn test_allow_rules_override_mode() {
        let custom = PermissionPolicy::new(
            PermissionMode::ReadOnly,
            HashMap::from([("bash".to_string(), PermissionMode::WorkspaceWrite)]),
            vec!["bash".to_string()], // allow bash explicitly
            vec![],
            vec![],
        );
        let outcome = custom.authorize("bash", "ls", None, None);
        assert!(outcome.is_allowed());
    }

    #[test]
    fn test_ask_rules_prompt_with_allow_all() {
        let mut policy = workspace_policy();
        policy.ask_rules = vec!["bash".to_string()];
        let mut prompter = AllowAllPrompter::new();
        let outcome = policy.authorize("bash", "ls", None, Some(&mut prompter));
        assert!(outcome.is_allowed());
    }

    #[test]
    fn test_ask_rules_deny_without_prompter() {
        let mut policy = workspace_policy();
        policy.ask_rules = vec!["bash".to_string()];
        let outcome = policy.authorize("bash", "ls", None, None);
        assert!(outcome.is_denied());
        assert!(outcome.to_string().contains("no interactive prompt"));
    }

    #[test]
    fn test_ask_rules_deny_with_deny_all() {
        let mut policy = workspace_policy();
        policy.ask_rules = vec!["bash".to_string()];
        let mut prompter = DenyAllPrompter::new();
        let outcome = policy.authorize("bash", "ls", None, Some(&mut prompter));
        assert!(outcome.is_denied());
    }

    #[test]
    fn test_context_elevation() {
        let policy = read_only_policy();
        let context = PermissionContext::elevate(PermissionMode::WorkspaceWrite, "Need to write");
        let outcome = policy.authorize("write_file", "/tmp/test", Some(&context), None);
        assert!(outcome.is_allowed());
    }

    #[test]
    fn test_context_bypass() {
        let policy = read_only_policy();
        let context = PermissionContext::bypass("Emergency bypass");
        let outcome = policy.authorize("delete_file", "/tmp/x", Some(&context), None);
        assert!(outcome.is_allowed());
    }

    #[test]
    fn test_unknown_tool_defaults_to_workspace_write() {
        let policy = read_only_policy();
        let outcome = policy.authorize("unknown_tool", "input", None, None);
        // Unknown tools default to WorkspaceWrite requirement,
        // but active mode is ReadOnly, so it's denied
        assert!(outcome.is_denied());

        let policy = workspace_policy();
        let outcome = policy.authorize("unknown_tool", "input", None, None);
        assert!(outcome.is_allowed());
    }

    #[test]
    fn test_wildcard_allow_rule() {
        let mut policy = read_only_policy();
        policy.allow_rules = vec!["*".to_string()];
        let outcome = policy.authorize("write_file", "/tmp/test", None, None);
        assert!(outcome.is_allowed());
    }

    #[test]
    fn test_wildcard_deny_rule() {
        let mut policy = workspace_policy();
        policy.deny_rules = vec!["*".to_string()];
        let outcome = policy.authorize("read_file", "test.txt", None, None);
        assert!(outcome.is_denied());
    }

    #[test]
    fn test_required_mode_for() {
        let policy = workspace_policy();
        assert_eq!(policy.required_mode_for("read_file"), PermissionMode::ReadOnly);
        assert_eq!(policy.required_mode_for("write_file"), PermissionMode::WorkspaceWrite);
        assert_eq!(policy.required_mode_for("delete_file"), PermissionMode::DangerousFullAccess);
        assert_eq!(policy.required_mode_for("unknown_tool"), PermissionMode::WorkspaceWrite);
    }

    #[test]
    fn test_set_active_mode() {
        let mut policy = read_only_policy();
        assert_eq!(policy.active_mode(), PermissionMode::ReadOnly);
        policy.set_active_mode(PermissionMode::DangerousFullAccess);
        assert_eq!(policy.active_mode(), PermissionMode::DangerousFullAccess);
    }
}
