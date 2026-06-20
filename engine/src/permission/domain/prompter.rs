//! PermissionPrompter — trait for interactive confirmation.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md#prompter
//! Implements: Contract Freeze — PermissionPrompter trait
//! Issue: issue-contract-freeze
//!
//! Provides an interactive confirmation flow for tools that require
//! human approval before execution. The prompter is used by the
//! PermissionPolicy when a tool matches the "ask" rules.
//!
//! # Contract (Frozen)
//! - Trait is object-safe (no async methods, no generic parameters)
//! - `prompt()` returns `PermissionOutcome` — the prompter makes the final decision
//! - `prompt_many()` allows batch confirmation for multiple tools at once

use serde::{Deserialize, Serialize};

use super::outcome::PermissionOutcome;

/// User response from an interactive permission prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PromptResponse {
    /// The user approved the operation.
    Approved,

    /// The user denied the operation.
    Denied,

    /// The user requested a modification (e.g., different args).
    Modify {
        /// Suggested modification or reason.
        suggestion: String,
    },

    /// The prompt timed out without a response.
    Timeout,
}

/// Trait for interactive permission confirmation.
///
/// Implementations can provide TUI prompts, CLI stdin prompts,
/// or API-based confirmation flows.
pub trait PermissionPrompter: Send {
    /// Prompt the user for approval of a tool invocation.
    ///
    /// Returns `PermissionOutcome::Allowed` if approved,
    /// `PermissionOutcome::Denied` if rejected.
    fn prompt(&mut self, tool_name: &str, tool_input: &str) -> PermissionOutcome;

    /// Prompt the user for multiple tool invocations at once.
    ///
    /// Each entry in the slice is a (tool_name, tool_input) pair.
    /// Returns outcomes in the same order.
    fn prompt_many(&mut self, tools: &[(&str, &str)]) -> Vec<PermissionOutcome>;

    /// Returns `true` if this prompter is capable of interactive prompting.
    /// Returns `false` for headless/non-interactive environments.
    fn is_interactive(&self) -> bool;
}

/// A permission prompter that denies all prompts (safe default for CI/headless).
///
/// Useful as a fallback when no interactive prompter is available.
#[derive(Debug, Clone, Default)]
pub struct DenyAllPrompter;

impl DenyAllPrompter {
    pub fn new() -> Self {
        Self
    }
}

impl PermissionPrompter for DenyAllPrompter {
    fn prompt(&mut self, tool_name: &str, _tool_input: &str) -> PermissionOutcome {
        PermissionOutcome::Denied {
            tool: tool_name.to_string(),
            active_mode: "workspace_write".to_string(),
            required_mode: "workspace_write".to_string(),
            reason: "Interactive confirmation not available in headless mode".to_string(),
        }
    }

    fn prompt_many(&mut self, tools: &[(&str, &str)]) -> Vec<PermissionOutcome> {
        tools
            .iter()
            .map(|(tool_name, _)| PermissionOutcome::Denied {
                tool: tool_name.to_string(),
                active_mode: "workspace_write".to_string(),
                required_mode: "workspace_write".to_string(),
                reason: "Interactive confirmation not available in headless mode".to_string(),
            })
            .collect()
    }

    fn is_interactive(&self) -> bool {
        false
    }
}

/// A permission prompter that approves all prompts (for testing).
#[derive(Debug, Clone, Default)]
pub struct AllowAllPrompter;

impl AllowAllPrompter {
    pub fn new() -> Self {
        Self
    }
}

impl PermissionPrompter for AllowAllPrompter {
    fn prompt(&mut self, _tool_name: &str, _tool_input: &str) -> PermissionOutcome {
        PermissionOutcome::Allowed
    }

    fn prompt_many(&mut self, tools: &[(&str, &str)]) -> Vec<PermissionOutcome> {
        tools.iter().map(|_| PermissionOutcome::Allowed).collect()
    }

    fn is_interactive(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deny_all_prompter() {
        let mut prompter = DenyAllPrompter::new();
        assert!(!prompter.is_interactive());

        let outcome = prompter.prompt("bash", "rm -rf /");
        assert!(outcome.is_denied());
    }

    #[test]
    fn test_allow_all_prompter() {
        let mut prompter = AllowAllPrompter::new();
        assert!(!prompter.is_interactive());

        let outcome = prompter.prompt("bash", "rm -rf /");
        assert!(outcome.is_allowed());
    }

    #[test]
    fn test_prompt_many_deny_all() {
        let mut prompter = DenyAllPrompter::new();
        let tools = [("bash", "ls"), ("write_file", "/tmp/test")];
        let outcomes = prompter.prompt_many(&tools);
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes[0].is_denied());
        assert!(outcomes[1].is_denied());
    }

    #[test]
    fn test_prompt_many_allow_all() {
        let mut prompter = AllowAllPrompter::new();
        let tools = [("bash", "ls"), ("write_file", "/tmp/test")];
        let outcomes = prompter.prompt_many(&tools);
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes[0].is_allowed());
        assert!(outcomes[1].is_allowed());
    }
}
