//! HookRunResult — aggregated result from running all hook commands for an event.
//!
//! @canonical .pi/architecture/modules/hooks.md#hook-result
//! Implements: Contract Freeze — HookRunResult struct
//! Issue: #410
//!
//! Represents the aggregated outcome of executing all registered hook
//! commands for a given lifecycle event. Multiple hooks are executed and
//! their results are merged into a single `HookRunResult`.
//!
//! # Aggregation Rules
//! - First `deny` wins: if any hook denies, the tool is blocked
//! - Messages from all hooks are concatenated
//! - Last `permission_override` wins (if multiple hooks provide one)
//! - Last `updated_input` wins (if multiple hooks modify input)
//! - Cancellation overrides allow
//!
//! # Contract (Frozen)
//! - All fields are public for direct access
//! - Helper methods provide ergonomic access to aggregated state
//! - Serialization support for API responses

use serde::{Deserialize, Serialize};

use super::event::HookEvent;

/// Aggregated result from executing all hook commands for a given event.
///
/// # Example
///
/// ```rust
/// use rigorix_engine::hooks::domain::HookRunResult;
///
/// let result = HookRunResult::blocked(
///     "PreToolUse hook denied execution".to_string(),
///     vec!["Command 'rm -rf /' not allowed".to_string()],
/// );
/// assert!(result.is_denied());
/// assert!(!result.is_allowed());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HookRunResult {
    /// Which lifecycle event this result is for.
    pub event: HookEvent,

    /// True if any hook command returned `Deny`.
    pub denied: bool,

    /// True if any hook command failed (non-zero exit + no valid JSON).
    pub failed: bool,

    /// True if the abort signal was set during execution.
    pub cancelled: bool,

    /// Aggregated messages from all hooks, in execution order.
    #[serde(default)]
    pub messages: Vec<String>,

    /// Permission override from the last hook that provided one.
    #[serde(default)]
    pub permission_override: Option<super::protocol::HookPermissionOverride>,

    /// Reason for the permission override.
    #[serde(default)]
    pub permission_reason: Option<String>,

    /// Updated tool input from the last hook that modified it.
    #[serde(default)]
    pub updated_input: Option<serde_json::Value>,
}

impl HookRunResult {
    /// Create a new aggregated result.
    pub fn new(event: HookEvent) -> Self {
        Self {
            event,
            denied: false,
            failed: false,
            cancelled: false,
            messages: vec![],
            permission_override: None,
            permission_reason: None,
            updated_input: None,
        }
    }

    /// Create a result representing a blocked (denied or failed) outcome.
    pub fn blocked(reason: String, messages: Vec<String>) -> Self {
        Self {
            event: HookEvent::PreToolUse,
            denied: true,
            failed: false,
            cancelled: false,
            messages,
            permission_override: None,
            permission_reason: Some(reason),
            updated_input: None,
        }
    }

    /// Create a result representing a cancelled outcome.
    pub fn cancelled(event: HookEvent, messages: Vec<String>) -> Self {
        Self {
            event,
            denied: false,
            failed: false,
            cancelled: true,
            messages,
            permission_override: None,
            permission_reason: None,
            updated_input: None,
        }
    }

    /// Returns true if the tool execution should be blocked.
    pub fn is_denied(&self) -> bool {
        self.denied
    }

    /// Returns true if any hook execution failed.
    pub fn is_failed(&self) -> bool {
        self.failed
    }

    /// Returns true if the execution was cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// Returns true if the tool can proceed (not denied, failed, or cancelled).
    pub fn is_allowed(&self) -> bool {
        !self.denied && !self.failed && !self.cancelled
    }

    /// Returns the modified input if any hook provided one.
    pub fn modified_input(&self) -> Option<&serde_json::Value> {
        self.updated_input.as_ref()
    }

    /// Returns the permission override if any hook provided one.
    pub fn override_permission(&self) -> Option<super::protocol::HookPermissionOverride> {
        self.permission_override
    }

    /// Returns all feedback messages concatenated.
    pub fn feedback_messages(&self) -> &[String] {
        &self.messages
    }

    /// Aggregate another hook's result into this one.
    ///
    /// # Aggregation Rules
    /// - `denied`: OR (first deny wins)
    /// - `failed`: OR (any failure marks as failed)
    /// - `cancelled`: OR (any cancellation marks as cancelled)
    /// - `messages`: appended
    /// - `permission_override`: last wins
    /// - `updated_input`: last wins
    pub fn merge(&mut self, other: &HookRunResult) {
        self.denied = self.denied || other.denied;
        self.failed = self.failed || other.failed;
        self.cancelled = self.cancelled || other.cancelled;
        self.messages.extend_from_slice(&other.messages);
        if other.permission_override.is_some() {
            self.permission_override = other.permission_override;
            self.permission_reason = other.permission_reason.clone();
        }
        if other.updated_input.is_some() {
            self.updated_input = other.updated_input.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::domain::protocol::HookPermissionOverride;

    #[test]
    fn test_new_result_is_allowed() {
        let result = HookRunResult::new(HookEvent::PreToolUse);
        assert!(result.is_allowed());
        assert!(!result.is_denied());
        assert!(!result.is_failed());
        assert!(!result.is_cancelled());
    }

    #[test]
    fn test_blocked_result() {
        let result = HookRunResult::blocked(
            "Policy violation".into(),
            vec!["rm blocked".into()],
        );
        assert!(result.is_denied());
        assert!(!result.is_allowed());
        assert_eq!(result.permission_reason, Some("Policy violation".into()));
    }

    #[test]
    fn test_cancelled_result() {
        let result = HookRunResult::cancelled(HookEvent::PostToolUse, vec!["Aborted".into()]);
        assert!(result.is_cancelled());
        assert!(!result.is_allowed());
        assert_eq!(result.event, HookEvent::PostToolUse);
    }

    #[test]
    fn test_merge_deny_wins() {
        let mut result = HookRunResult::new(HookEvent::PreToolUse);
        let deny = HookRunResult {
            event: HookEvent::PreToolUse,
            denied: true,
            failed: false,
            cancelled: false,
            messages: vec!["Denied!".into()],
            permission_override: None,
            permission_reason: None,
            updated_input: None,
        };
        result.merge(&deny);
        assert!(result.is_denied());
        assert_eq!(result.messages, vec!["Denied!"]);
    }

    #[test]
    fn test_merge_permission_override_last_wins() {
        let mut result = HookRunResult::new(HookEvent::PreToolUse);
        let first = HookRunResult {
            event: HookEvent::PreToolUse,
            denied: false,
            failed: false,
            cancelled: false,
            messages: vec![],
            permission_override: Some(HookPermissionOverride::RequireConfirmation),
            permission_reason: Some("First override".into()),
            updated_input: None,
        };
        let second = HookRunResult {
            event: HookEvent::PreToolUse,
            denied: false,
            failed: false,
            cancelled: false,
            messages: vec![],
            permission_override: Some(HookPermissionOverride::RequireDryRun),
            permission_reason: Some("Second override".into()),
            updated_input: None,
        };
        result.merge(&first);
        result.merge(&second);
        assert_eq!(
            result.permission_override,
            Some(HookPermissionOverride::RequireDryRun)
        );
        assert_eq!(result.permission_reason, Some("Second override".into()));
    }

    #[test]
    fn test_merge_updated_input_last_wins() {
        let mut result = HookRunResult::new(HookEvent::PreToolUse);
        let first = HookRunResult {
            event: HookEvent::PreToolUse,
            denied: false,
            failed: false,
            cancelled: false,
            messages: vec![],
            permission_override: None,
            permission_reason: None,
            updated_input: Some(serde_json::json!({"a": 1})),
        };
        let second = HookRunResult {
            event: HookEvent::PreToolUse,
            denied: false,
            failed: false,
            cancelled: false,
            messages: vec![],
            permission_override: None,
            permission_reason: None,
            updated_input: Some(serde_json::json!({"b": 2})),
        };
        result.merge(&first);
        result.merge(&second);
        assert_eq!(result.updated_input, Some(serde_json::json!({"b": 2})));
    }

    #[test]
    fn test_merge_messages_accumulate() {
        let mut result = HookRunResult::new(HookEvent::PreToolUse);
        result.merge(&HookRunResult {
            event: HookEvent::PreToolUse,
            denied: false,
            failed: false,
            cancelled: false,
            messages: vec!["A".into()],
            permission_override: None,
            permission_reason: None,
            updated_input: None,
        });
        result.merge(&HookRunResult {
            event: HookEvent::PreToolUse,
            denied: false,
            failed: false,
            cancelled: false,
            messages: vec!["B".into()],
            permission_override: None,
            permission_reason: None,
            updated_input: None,
        });
        assert_eq!(result.messages, vec!["A", "B"]);
    }

    #[test]
    fn test_serde_roundtrip() {
        let result = HookRunResult::blocked(
            "Not allowed".into(),
            vec!["Security violation".into()],
        );
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: HookRunResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }

    #[test]
    fn test_accessor_methods() {
        let mut result = HookRunResult::new(HookEvent::PreToolUse);
        assert!(result.modified_input().is_none());
        assert!(result.override_permission().is_none());
        assert!(result.feedback_messages().is_empty());

        result.updated_input = Some(serde_json::json!({"key": "value"}));
        result.permission_override = Some(HookPermissionOverride::BypassGates);
        result.messages.push("Feedback".into());

        assert_eq!(result.modified_input(), Some(&serde_json::json!({"key": "value"})));
        assert_eq!(
            result.override_permission(),
            Some(HookPermissionOverride::BypassGates)
        );
        assert_eq!(result.feedback_messages(), &["Feedback"]);
    }
}
