//! Hook Protocol — JSON contract between the engine and hook scripts.
//!
//! @canonical .pi/architecture/modules/hooks.md#hook-protocol
//! Implements: Contract Freeze — Hook stdin/stdout JSON protocol
//! Issue: #410
//!
//! Defines the wire format for communication between the Rigorix engine
//! and hook scripts. Hook scripts receive a JSON payload on stdin and
//! must return a structured JSON decision on stdout.
//!
//! # Protocol
//!
//! ```text
//! Engine → Hook (stdin):  HookStdinPayload (JSON)
//! Hook → Engine (stdout): HookStdoutResponse (JSON)
//! ```
//!
//! # Contract (Frozen)
//! - All payloads are serializable as JSON
//! - Stdin payload includes event type, tool identity, and context
//! - Stdout response includes decision, optional modifications, feedback
//! - Non-zero exit with invalid JSON is treated as `HookDecision::Deny`

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::event::HookEvent;

// ---------------------------------------------------------------------------
// Stdin Payload (Engine → Hook)
// ---------------------------------------------------------------------------

/// JSON payload sent to a hook script on stdin.
///
/// Provides the hook with full context about the tool invocation so it
/// can make informed decisions.
///
/// # Example (PreToolUse)
///
/// ```json
/// {
///     "event": "pre_tool_use",
///     "tool_name": "run_command",
///     "tool_input": {"params": {"command": "cargo build --release"}},
///     "session_id": "exec_abc123",
///     "workspace_root": "/home/user/project",
///     "environment_vars": {"RIGORIX_ENV": "production"}
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookStdinPayload {
    /// The lifecycle event this hook is responding to.
    pub event: HookEvent,

    /// Name of the tool being invoked (e.g., "run_command", "write_file").
    pub tool_name: String,

    /// The original or current tool input parameters as a JSON value.
    /// This is the full ToolInput struct (params map + execution_id).
    pub tool_input: serde_json::Value,

    /// Globally unique session/execution identifier for correlation.
    pub session_id: String,

    /// Absolute path to the workspace root directory.
    pub workspace_root: String,

    /// Key-value pairs of environment variables relevant to hook execution.
    /// Includes `RIGORIX_TOOL_NAME`, `RIGORIX_EVENT`, `RIGORIX_SESSION_ID`,
    /// `RIGORIX_WORKSPACE`, and any user-defined variables.
    #[serde(default)]
    pub environment_vars: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Stdout Response (Hook → Engine)
// ---------------------------------------------------------------------------

/// JSON response that a hook script must write to stdout.
///
/// The engine reads this response and applies the decision. If the hook
/// exits with a non-zero status code and invalid JSON, it is treated as
/// `HookDecision::Deny` (fail-safe).
///
/// # Example
///
/// ```json
/// {
///     "decision": "allow",
///     "reason": "Command is known and safe",
///     "permission_override": null,
///     "updated_input": null,
///     "messages": ["Running in CI environment - sandbox enabled"]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookStdoutResponse {
    /// The hook's decision.
    pub decision: HookDecision,

    /// Human-readable explanation for the decision.
    /// Fed back to the LLM when the tool is blocked.
    #[serde(default)]
    pub reason: Option<String>,

    /// If `decision` is `AllowWithOverride`, the new risk level to apply.
    /// Ignored for other decisions.
    #[serde(default)]
    pub permission_override: Option<HookPermissionOverride>,

    /// If `decision` is `Modify`, the updated tool input to use.
    /// This replaces the original tool input for this execution.
    #[serde(default)]
    pub updated_input: Option<serde_json::Value>,

    /// Feedback messages to merge into the tool result.
    /// These are surfaced to the LLM for awareness.
    #[serde(default)]
    pub messages: Vec<String>,
}

/// Decision returned by a hook script.
///
/// Determines how the engine proceeds with the tool invocation.
///
/// | Decision | Effect |
/// |----------|--------|
/// | `Allow` | Tool proceeds normally |
/// | `Deny` | Tool blocked, reason fed to LLM |
/// | `AllowWithOverride` | Tool proceeds with permission override |
/// | `Modify` | Tool proceeds with updated input |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookDecision {
    /// Tool proceeds normally without any changes.
    Allow,

    /// Tool is blocked; the reason is fed back to the LLM.
    Deny,

    /// Tool proceeds with a permission override (risk level change).
    AllowWithOverride,

    /// Tool proceeds with modified input replacing the original.
    Modify,
}

/// Permission override that a hook can apply.
///
/// Allows hooks to dynamically elevate or restrict the risk level
/// of a tool invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookPermissionOverride {
    /// Elevate to require user confirmation.
    RequireConfirmation,

    /// Elevate to require dry-run first.
    RequireDryRun,

    /// Restrict to auto-execute (bypass gates).
    BypassGates,
}

// ---------------------------------------------------------------------------
// Helper methods
// ---------------------------------------------------------------------------

impl HookStdoutResponse {
    /// Create an `Allow` response with optional messages.
    pub fn allow(messages: Vec<String>) -> Self {
        Self {
            decision: HookDecision::Allow,
            reason: None,
            permission_override: None,
            updated_input: None,
            messages,
        }
    }

    /// Create a `Deny` response with a reason.
    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            decision: HookDecision::Deny,
            reason: Some(reason.into()),
            permission_override: None,
            updated_input: None,
            messages: vec![],
        }
    }

    /// Create an `AllowWithOverride` response.
    pub fn allow_with_override(
        override_kind: HookPermissionOverride,
        reason: impl Into<String>,
        messages: Vec<String>,
    ) -> Self {
        Self {
            decision: HookDecision::AllowWithOverride,
            reason: Some(reason.into()),
            permission_override: Some(override_kind),
            updated_input: None,
            messages,
        }
    }

    /// Create a `Modify` response with updated input.
    pub fn modify(
        updated_input: serde_json::Value,
        reason: impl Into<String>,
        messages: Vec<String>,
    ) -> Self {
        Self {
            decision: HookDecision::Modify,
            reason: Some(reason.into()),
            permission_override: None,
            updated_input: Some(updated_input),
            messages,
        }
    }

    /// Returns true if the hook allows the tool to proceed.
    pub fn is_allowed(&self) -> bool {
        matches!(
            self.decision,
            HookDecision::Allow | HookDecision::AllowWithOverride | HookDecision::Modify
        )
    }

    /// Returns true if the hook denies the tool execution.
    pub fn is_denied(&self) -> bool {
        self.decision == HookDecision::Deny
    }
}

impl HookStdinPayload {
    /// Create a new stdin payload for hook execution.
    pub fn new(
        event: HookEvent,
        tool_name: impl Into<String>,
        tool_input: serde_json::Value,
        session_id: impl Into<String>,
        workspace_root: impl Into<String>,
    ) -> Self {
        Self {
            event,
            tool_name: tool_name.into(),
            tool_input,
            session_id: session_id.into(),
            workspace_root: workspace_root.into(),
            environment_vars: HashMap::new(),
        }
    }

    /// Add an environment variable to the payload.
    pub fn with_env_var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment_vars.insert(key.into(), value.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // HookStdoutResponse helper methods
    // -----------------------------------------------------------------------

    #[test]
    fn test_allow_response() {
        let resp = HookStdoutResponse::allow(vec!["All good".into()]);
        assert_eq!(resp.decision, HookDecision::Allow);
        assert!(resp.is_allowed());
        assert!(!resp.is_denied());
        assert_eq!(resp.messages, vec!["All good"]);
    }

    #[test]
    fn test_deny_response() {
        let resp = HookStdoutResponse::deny("Blocked by policy");
        assert_eq!(resp.decision, HookDecision::Deny);
        assert!(!resp.is_allowed());
        assert!(resp.is_denied());
        assert_eq!(resp.reason.unwrap(), "Blocked by policy");
    }

    #[test]
    fn test_allow_with_override_response() {
        let resp = HookStdoutResponse::allow_with_override(
            HookPermissionOverride::RequireConfirmation,
            "Elevated risk",
            vec!["Proceed with caution".into()],
        );
        assert_eq!(resp.decision, HookDecision::AllowWithOverride);
        assert!(resp.is_allowed());
        assert_eq!(
            resp.permission_override,
            Some(HookPermissionOverride::RequireConfirmation)
        );
    }

    #[test]
    fn test_modify_response() {
        let updated = serde_json::json!({"params": {"command": "cargo check"}});
        let resp = HookStdoutResponse::modify(
            updated.clone(),
            "Switched to check mode",
            vec!["Build → check".into()],
        );
        assert_eq!(resp.decision, HookDecision::Modify);
        assert!(resp.is_allowed());
        assert_eq!(resp.updated_input, Some(updated));
    }

    // -----------------------------------------------------------------------
    // HookStdinPayload builder
    // -----------------------------------------------------------------------

    #[test]
    fn test_stdin_payload_construction() {
        let input = serde_json::json!({"params": {"path": "/tmp/test"}});
        let payload = HookStdinPayload::new(
            HookEvent::PreToolUse,
            "read_file",
            input.clone(),
            "session-1",
            "/workspace",
        );
        assert_eq!(payload.event, HookEvent::PreToolUse);
        assert_eq!(payload.tool_name, "read_file");
        assert_eq!(payload.tool_input, input);
        assert_eq!(payload.session_id, "session-1");
        assert_eq!(payload.workspace_root, "/workspace");
        assert!(payload.environment_vars.is_empty());
    }

    #[test]
    fn test_stdin_payload_with_env_vars() {
        let payload = HookStdinPayload::new(
            HookEvent::PostToolUse,
            "write_file",
            serde_json::Value::Null,
            "session-2",
            "/project",
        )
        .with_env_var("RIGORIX_ENV", "production");
        assert_eq!(
            payload.environment_vars.get("RIGORIX_ENV"),
            Some(&"production".to_string())
        );
    }

    // -----------------------------------------------------------------------
    // Serde round-trips
    // -----------------------------------------------------------------------

    #[test]
    fn test_hook_decision_serde_roundtrip() {
        for decision in &[
            HookDecision::Allow,
            HookDecision::Deny,
            HookDecision::AllowWithOverride,
            HookDecision::Modify,
        ] {
            let json = serde_json::to_string(decision).unwrap();
            let deserialized: HookDecision = serde_json::from_str(&json).unwrap();
            assert_eq!(*decision, deserialized);
        }
    }

    #[test]
    fn test_hook_decision_serde_snake_case() {
        assert_eq!(
            serde_json::to_string(&HookDecision::Allow).unwrap(),
            "\"allow\""
        );
        assert_eq!(
            serde_json::to_string(&HookDecision::AllowWithOverride).unwrap(),
            "\"allow_with_override\""
        );
    }

    #[test]
    fn test_hook_stdout_response_serde_roundtrip() {
        let resp = HookStdoutResponse::allow(vec!["OK".into()]);
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: HookStdoutResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.decision, HookDecision::Allow);
    }

    #[test]
    fn test_hook_stdin_payload_serde_roundtrip() {
        let payload = HookStdinPayload::new(
            HookEvent::PreToolUse,
            "run_command",
            serde_json::json!({"params": {"command": "ls"}}),
            "sess-1",
            "/root",
        )
        .with_env_var("PATH", "/usr/bin");
        let json = serde_json::to_string(&payload).unwrap();
        let deserialized: HookStdinPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event, HookEvent::PreToolUse);
        assert_eq!(deserialized.tool_name, "run_command");
        assert_eq!(
            deserialized.environment_vars.get("PATH").unwrap(),
            "/usr/bin"
        );
    }

    // -----------------------------------------------------------------------
    // HookPermissionOverride serde
    // -----------------------------------------------------------------------

    #[test]
    fn test_permission_override_serde() {
        for override_kind in &[
            HookPermissionOverride::RequireConfirmation,
            HookPermissionOverride::RequireDryRun,
            HookPermissionOverride::BypassGates,
        ] {
            let json = serde_json::to_string(override_kind).unwrap();
            let deserialized: HookPermissionOverride = serde_json::from_str(&json).unwrap();
            assert_eq!(*override_kind, deserialized);
        }
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_empty_messages_allow() {
        let resp = HookStdoutResponse::allow(vec![]);
        assert!(resp.messages.is_empty());
        assert!(resp.is_allowed());
    }

    #[test]
    fn test_deny_with_empty_reason() {
        let resp = HookStdoutResponse::deny("");
        assert_eq!(resp.reason.as_ref().unwrap(), "");
        assert!(resp.is_denied());
    }
}
