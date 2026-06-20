//! HookConfig — declarative hook command registration per lifecycle event.
//!
//! @canonical .pi/architecture/modules/hooks.md#hook-config
//! Implements: Contract Freeze — HookConfig struct
//! Issue: #410
//!
//! Defines which shell commands or binaries to execute for each hook
//! lifecycle event. Commands are loaded from configuration (e.g.,
//! `.rigorix/hooks.toml`) and registered per event type.
//!
//! # Configuration Example (`.rigorix/hooks.toml`)
//!
//! ```toml
//! [hooks]
//! pre_tool_use = [
//!     "rigorix-hook-validate-path",
//!     "rigorix-hook-ci-guard --env $RIGORIX_ENV",
//! ]
//! post_tool_use = [
//!     "rigorix-hook-fmt-check --path $TOOL_PATH",
//! ]
//! post_tool_use_failure = [
//!     "rigorix-hook-notify --channel alerts",
//! ]
//! ```
//!
//! # Contract (Frozen)
//! - Each event type has a dedicated list of command strings
//! - Commands are executed in the order listed
//! - Empty lists mean no hooks for that event
//! - Commands must be relative to workspace or absolute paths

use serde::{Deserialize, Serialize};

use super::event::HookEvent;

/// Declarative hook command registration per lifecycle event.
///
/// Commands are shell commands or binaries that receive the hook stdin
/// JSON payload and return a structured stdout JSON response.
///
/// # Validation
/// - Commands are not validated at config load time; validation occurs
///   at hook execution time
/// - Unknown or missing commands are skipped with a warning
///
/// # Default
/// - All fields are empty `Vec`s by default (no hooks registered)
/// - Default timeout is 30 seconds
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookConfig {
    /// Commands to run before every tool execution.
    /// These can modify input, block, or override permissions.
    #[serde(default)]
    pub pre_tool_use: Vec<String>,

    /// Commands to run after every successful tool execution.
    /// These can append feedback, enrich audit context, or trigger scripts.
    #[serde(default)]
    pub post_tool_use: Vec<String>,

    /// Commands to run after every failed tool execution.
    /// These can trigger recovery scripts, enrich error context, or notify.
    #[serde(default)]
    pub post_tool_use_failure: Vec<String>,

    /// Timeout in seconds for each hook command execution.
    /// Default: 30 seconds.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    /// Whether to run PreToolUse hooks sequentially (default) or concurrently.
    /// Sequential execution allows each hook to see the modified input from
    /// the previous hook.
    #[serde(default)]
    pub sequential_pre_tool_use: bool,
}

fn default_timeout_secs() -> u64 {
    30
}

impl Default for HookConfig {
    fn default() -> Self {
        Self {
            pre_tool_use: vec![],
            post_tool_use: vec![],
            post_tool_use_failure: vec![],
            timeout_secs: 30,
            sequential_pre_tool_use: false,
        }
    }
}

impl HookConfig {
    /// Returns the list of commands registered for the given event.
    pub fn commands_for(&self, event: HookEvent) -> &[String] {
        match event {
            HookEvent::PreToolUse => &self.pre_tool_use,
            HookEvent::PostToolUse => &self.post_tool_use,
            HookEvent::PostToolUseFailure => &self.post_tool_use_failure,
        }
    }

    /// Returns true if there are commands registered for the given event.
    pub fn has_commands_for(&self, event: HookEvent) -> bool {
        !self.commands_for(event).is_empty()
    }

    /// Returns true if no hooks are registered for any event.
    pub fn is_empty(&self) -> bool {
        self.pre_tool_use.is_empty()
            && self.post_tool_use.is_empty()
            && self.post_tool_use_failure.is_empty()
    }

    /// Returns the total number of registered hook commands across all events.
    pub fn total_command_count(&self) -> usize {
        self.pre_tool_use.len() + self.post_tool_use.len() + self.post_tool_use_failure.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> HookConfig {
        HookConfig {
            pre_tool_use: vec!["validate-path".into(), "ci-guard".into()],
            post_tool_use: vec!["fmt-check".into()],
            post_tool_use_failure: vec!["notify".into()],
            timeout_secs: 30,
            sequential_pre_tool_use: true,
        }
    }

    #[test]
    fn test_commands_for() {
        let config = sample_config();
        assert_eq!(
            config.commands_for(HookEvent::PreToolUse),
            &["validate-path", "ci-guard"]
        );
        assert_eq!(config.commands_for(HookEvent::PostToolUse), &["fmt-check"]);
        assert_eq!(
            config.commands_for(HookEvent::PostToolUseFailure),
            &["notify"]
        );
    }

    #[test]
    fn test_has_commands_for() {
        let config = sample_config();
        assert!(config.has_commands_for(HookEvent::PreToolUse));
        assert!(config.has_commands_for(HookEvent::PostToolUse));
        assert!(config.has_commands_for(HookEvent::PostToolUseFailure));
    }

    #[test]
    fn test_empty_config() {
        let config = HookConfig::default();
        assert!(config.is_empty());
        assert_eq!(config.total_command_count(), 0);
        assert!(!config.has_commands_for(HookEvent::PreToolUse));
        assert!(!config.has_commands_for(HookEvent::PostToolUse));
        assert!(!config.has_commands_for(HookEvent::PostToolUseFailure));
    }

    #[test]
    fn test_total_command_count() {
        let config = sample_config();
        assert_eq!(config.total_command_count(), 4);
    }

    #[test]
    fn test_default_timeout() {
        let config = HookConfig::default();
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = sample_config();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: HookConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_serde_default_timeout() {
        // When timeout_secs is omitted, it should default to 30
        let json = r#"{"pre_tool_use":["hook1"]}"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.pre_tool_use, vec!["hook1"]);
    }

    #[test]
    fn test_commands_for_returns_empty_slice_for_unregistered() {
        let config = HookConfig::default();
        assert!(config.commands_for(HookEvent::PreToolUse).is_empty());
        assert!(config.commands_for(HookEvent::PostToolUse).is_empty());
        assert!(
            config
                .commands_for(HookEvent::PostToolUseFailure)
                .is_empty()
        );
    }

    #[test]
    fn test_clone_and_eq() {
        let config = sample_config();
        let cloned = config.clone();
        assert_eq!(config, cloned);
    }
}
