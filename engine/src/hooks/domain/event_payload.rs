//! Hook lifecycle event payloads for observability.
//!
//! @canonical .pi/architecture/modules/hooks.md#hook-events
//! Implements: Contract Freeze — HookEventPayload type
//! Issue: #410
//!
//! Defines event payloads that the hook system emits through the event bus
//! for observability, monitoring, and debugging. These are distinct from
//! `HookEvent` (which identifies lifecycle points) — these are the actual
//! data structures published when hook lifecycle events occur.
//!
//! # Contract (Frozen)
//! - Each hook lifecycle event has a dedicated payload struct
//! - All payloads are serializable as JSON for event bus integration
//! - Payloads carry context for debugging and audit trails

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::event::HookEvent;
use super::protocol::HookDecision;

/// Payload emitted when a hook command execution begins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookExecutionStartedPayload {
    /// Unique identifier for this hook execution.
    pub hook_execution_id: uuid::Uuid,

    /// The session/execution ID this hook is part of.
    pub session_id: String,

    /// The lifecycle event this hook is responding to.
    pub event: HookEvent,

    /// The hook command being executed.
    pub command: String,

    /// Name of the tool being intercepted.
    pub tool_name: String,

    /// ISO 8601 timestamp of when execution started.
    pub timestamp: DateTime<Utc>,
}

/// Payload emitted when a hook command execution completes successfully.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookExecutionCompletedPayload {
    /// Unique identifier for this hook execution.
    pub hook_execution_id: uuid::Uuid,

    /// The lifecycle event this hook responded to.
    pub event: HookEvent,

    /// The hook command that completed.
    pub command: String,

    /// The decision returned by the hook.
    pub decision: HookDecision,

    /// Duration of hook execution in milliseconds.
    pub duration_ms: u64,

    /// Number of feedback messages produced.
    pub message_count: usize,

    /// ISO 8601 timestamp of completion.
    pub timestamp: DateTime<Utc>,
}

/// Payload emitted when a hook command execution fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookExecutionFailedPayload {
    /// Unique identifier for this hook execution.
    pub hook_execution_id: uuid::Uuid,

    /// The lifecycle event this hook was responding to.
    pub event: HookEvent,

    /// The hook command that failed.
    pub command: String,

    /// Error message describing the failure.
    pub error: String,

    /// Duration of hook execution before failure in milliseconds.
    pub duration_ms: u64,

    /// ISO 8601 timestamp of the failure.
    pub timestamp: DateTime<Utc>,
}

/// Payload emitted when hook execution is aborted via `HookAbortSignal`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookExecutionAbortedPayload {
    /// Unique identifier for this hook execution.
    pub hook_execution_id: uuid::Uuid,

    /// The lifecycle event this hook was responding to.
    pub event: HookEvent,

    /// The hook command that was aborted.
    pub command: String,

    /// ISO 8601 timestamp of the abort.
    pub timestamp: DateTime<Utc>,
}

/// A unified hook lifecycle event payload for event bus emission.
///
/// Wraps all hook execution lifecycle payloads into a single enum
/// for convenient event bus integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HookEventPayload {
    /// A hook command execution has started.
    ExecutionStarted(HookExecutionStartedPayload),

    /// A hook command execution completed successfully.
    ExecutionCompleted(HookExecutionCompletedPayload),

    /// A hook command execution failed.
    ExecutionFailed(HookExecutionFailedPayload),

    /// A hook command execution was aborted.
    ExecutionAborted(HookExecutionAbortedPayload),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_started() -> HookExecutionStartedPayload {
        HookExecutionStartedPayload {
            hook_execution_id: uuid::Uuid::new_v4(),
            session_id: "session-1".into(),
            event: HookEvent::PreToolUse,
            command: "validate-path".into(),
            tool_name: "run_command".into(),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_execution_started_payload() {
        let p = sample_started();
        assert_eq!(p.event, HookEvent::PreToolUse);
        assert_eq!(p.command, "validate-path");
        assert_eq!(p.tool_name, "run_command");
    }

    #[test]
    fn test_execution_completed_payload() {
        let p = HookExecutionCompletedPayload {
            hook_execution_id: uuid::Uuid::new_v4(),
            event: HookEvent::PostToolUse,
            command: "fmt-check".into(),
            decision: HookDecision::Allow,
            duration_ms: 150,
            message_count: 2,
            timestamp: Utc::now(),
        };
        assert_eq!(p.decision, HookDecision::Allow);
        assert_eq!(p.duration_ms, 150);
    }

    #[test]
    fn test_execution_failed_payload() {
        let p = HookExecutionFailedPayload {
            hook_execution_id: uuid::Uuid::new_v4(),
            event: HookEvent::PostToolUseFailure,
            command: "notify".into(),
            error: "exit code 1".into(),
            duration_ms: 500,
            timestamp: Utc::now(),
        };
        assert_eq!(p.error, "exit code 1");
    }

    #[test]
    fn test_execution_aborted_payload() {
        let p = HookExecutionAbortedPayload {
            hook_execution_id: uuid::Uuid::new_v4(),
            event: HookEvent::PreToolUse,
            command: "slow-hook".into(),
            timestamp: Utc::now(),
        };
        assert_eq!(p.command, "slow-hook");
    }

    #[test]
    fn test_hook_event_payload_enum() {
        let payload = HookEventPayload::ExecutionStarted(sample_started());
        match &payload {
            HookEventPayload::ExecutionStarted(p) => {
                assert_eq!(p.command, "validate-path");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_serde_roundtrip_started() {
        let p = sample_started();
        let json = serde_json::to_string(&p).unwrap();
        let deserialized: HookExecutionStartedPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.command, p.command);
        assert_eq!(deserialized.hook_execution_id, p.hook_execution_id);
    }

    #[test]
    fn test_serde_roundtrip_event_payload() {
        let payload =
            HookEventPayload::ExecutionAborted(HookExecutionAbortedPayload {
                hook_execution_id: uuid::Uuid::new_v4(),
                event: HookEvent::PreToolUse,
                command: "timed-out".into(),
                timestamp: Utc::now(),
            });
        let json = serde_json::to_string(&payload).unwrap();
        let deserialized: HookEventPayload = serde_json::from_str(&json).unwrap();
        match deserialized {
            HookEventPayload::ExecutionAborted(p) => {
                assert_eq!(p.command, "timed-out");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_serde_tagged_union() {
        let payload =
            HookEventPayload::ExecutionCompleted(HookExecutionCompletedPayload {
                hook_execution_id: uuid::Uuid::new_v4(),
                event: HookEvent::PostToolUse,
                command: "hook-a".into(),
                decision: HookDecision::AllowWithOverride,
                duration_ms: 200,
                message_count: 1,
                timestamp: Utc::now(),
            });
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json.get("type").and_then(|v| v.as_str()), Some("execution_completed"));
    }
}
