//! HookEvent domain enum — identifies which lifecycle point a hook runs at.
//!
//! @canonical .pi/architecture/modules/hooks.md#hook-event
//! Implements: Contract Freeze — HookEvent enum
//! Issue: #410
//!
//! Defines the three lifecycle interception points around tool execution:
//! - PreToolUse: runs before tool execution, can modify input, block, or override permissions
//! - PostToolUse: runs after successful tool execution, can append feedback
//! - PostToolUseFailure: runs after tool execution failure, can trigger recovery
//!
//! # Contract (Frozen)
//! - Exactly 3 variants, no more, no less
//! - Serialized as snake_case via serde
//! - Copy semantics for easy passing
//! - No implementation logic — pure identification

use serde::{Deserialize, Serialize};

/// Identifies which lifecycle point a hook runs at.
///
/// # Variants
///
/// | Variant | Timing | Purpose |
/// |---------|--------|---------|
/// | `PreToolUse` | Before tool execution | Modify input, block, or override permissions |
/// | `PostToolUse` | After successful execution | Append feedback, enrichment |
/// | `PostToolUseFailure` | After failed execution | Trigger recovery, diagnostics |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    /// Runs before tool execution.
    ///
    /// Can modify input, block execution entirely, override permission levels,
    /// or provide feedback messages to the LLM.
    #[default]
    PreToolUse,

    /// Runs after successful tool execution.
    ///
    /// Can append feedback messages to the tool output, enrich audit context,
    /// or trigger post-flight scripts (e.g., `cargo test` after `write_file`).
    PostToolUse,

    /// Runs after tool execution failure.
    ///
    /// Can trigger recovery scripts, enrich error context with diagnostics,
    /// or notify external monitoring systems.
    PostToolUseFailure,
}

impl HookEvent {
    /// Returns the canonical snake_case name of this event variant.
    pub fn as_str(&self) -> &'static str {
        match self {
            HookEvent::PreToolUse => "pre_tool_use",
            HookEvent::PostToolUse => "post_tool_use",
            HookEvent::PostToolUseFailure => "post_tool_use_failure",
        }
    }

    /// Returns true if this event runs before tool execution.
    pub fn is_pre_tool_use(&self) -> bool {
        matches!(self, HookEvent::PreToolUse)
    }

    /// Returns true if this event runs after successful tool execution.
    pub fn is_post_tool_use(&self) -> bool {
        matches!(self, HookEvent::PostToolUse)
    }

    /// Returns true if this event runs after failed tool execution.
    pub fn is_post_tool_use_failure(&self) -> bool {
        matches!(self, HookEvent::PostToolUseFailure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_str() {
        assert_eq!(HookEvent::PreToolUse.as_str(), "pre_tool_use");
        assert_eq!(HookEvent::PostToolUse.as_str(), "post_tool_use");
        assert_eq!(
            HookEvent::PostToolUseFailure.as_str(),
            "post_tool_use_failure"
        );
    }

    #[test]
    fn test_is_methods() {
        assert!(HookEvent::PreToolUse.is_pre_tool_use());
        assert!(!HookEvent::PreToolUse.is_post_tool_use());
        assert!(!HookEvent::PreToolUse.is_post_tool_use_failure());

        assert!(!HookEvent::PostToolUse.is_pre_tool_use());
        assert!(HookEvent::PostToolUse.is_post_tool_use());
        assert!(!HookEvent::PostToolUse.is_post_tool_use_failure());

        assert!(!HookEvent::PostToolUseFailure.is_pre_tool_use());
        assert!(!HookEvent::PostToolUseFailure.is_post_tool_use());
        assert!(HookEvent::PostToolUseFailure.is_post_tool_use_failure());
    }

    #[test]
    fn test_copy_semantics() {
        let event = HookEvent::PreToolUse;
        let copied = event; // Copy, not move
        assert_eq!(event, copied);
    }

    #[test]
    fn test_serde_roundtrip() {
        for event in &[
            HookEvent::PreToolUse,
            HookEvent::PostToolUse,
            HookEvent::PostToolUseFailure,
        ] {
            let json = serde_json::to_string(event).unwrap();
            let deserialized: HookEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(*event, deserialized);
        }
    }

    #[test]
    fn test_default_is_pre_tool_use() {
        assert_eq!(HookEvent::default(), HookEvent::PreToolUse);
    }

    #[test]
    fn test_serde_snake_case() {
        assert_eq!(
            serde_json::to_string(&HookEvent::PreToolUse).unwrap(),
            "\"pre_tool_use\""
        );
        assert_eq!(
            serde_json::to_string(&HookEvent::PostToolUse).unwrap(),
            "\"post_tool_use\""
        );
        assert_eq!(
            serde_json::to_string(&HookEvent::PostToolUseFailure).unwrap(),
            "\"post_tool_use_failure\""
        );
    }

    #[test]
    fn test_serde_deserialize_snake_case() {
        assert_eq!(
            serde_json::from_str::<HookEvent>("\"pre_tool_use\"").unwrap(),
            HookEvent::PreToolUse
        );
        assert_eq!(
            serde_json::from_str::<HookEvent>("\"post_tool_use\"").unwrap(),
            HookEvent::PostToolUse
        );
        assert_eq!(
            serde_json::from_str::<HookEvent>("\"post_tool_use_failure\"").unwrap(),
            HookEvent::PostToolUseFailure
        );
    }

    #[test]
    fn test_equality() {
        assert_eq!(HookEvent::PreToolUse, HookEvent::PreToolUse);
        assert_ne!(HookEvent::PreToolUse, HookEvent::PostToolUse);
        assert_ne!(HookEvent::PreToolUse, HookEvent::PostToolUseFailure);
    }
}
