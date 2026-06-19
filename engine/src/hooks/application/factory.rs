//! Factory interfaces for constructing HookRunner instances.
//!
//! @canonical .pi/architecture/modules/hooks.md
//! Implements: Contract Freeze — HookRunnerFactory trait
//! Issue: #410
//!
//! Factories encapsulate the construction of `HookRunnerService` instances,
//! allowing implementations to inject dependencies (process spawner,
//! configuration loader) and apply default configurations without exposing
//! construction logic to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured service instance
//! - Default configuration is applied when not explicitly provided
//! - No mutable state in factory implementations

use crate::hooks::domain::config::HookConfig;
use crate::hooks::domain::error::HookError;

use super::service::HookRunnerService;

/// Factory for constructing `HookRunnerService` instances.
///
/// Handles creation with default or explicit configuration, including
/// hook command registration and timeout settings.
pub trait HookRunnerFactory: Send + Sync {
    /// Create a `HookRunnerService` with explicit configuration.
    ///
    /// The provided `HookConfig` defines which commands to run for
    /// each lifecycle event, timeout settings, and execution mode.
    fn create(&self, config: HookConfig) -> Result<Box<dyn HookRunnerService>, HookError>;

    /// Create a `HookRunnerService` with default (empty) configuration.
    ///
    /// Uses `HookConfig::default()` (no hooks registered, 30s timeout).
    fn create_default(&self) -> Result<Box<dyn HookRunnerService>, HookError>;

    /// Create a `HookRunnerService` with only PreToolUse hooks.
    ///
    /// Convenience method for the most common use case — pre-flight
    /// validation without post-execution hooks.
    fn create_with_pre_hooks(
        &self,
        pre_tool_use_commands: Vec<String>,
    ) -> Result<Box<dyn HookRunnerService>, HookError>;

    /// Create a `HookRunnerService` with a custom timeout.
    ///
    /// Convenience method for overriding the default 30s timeout.
    fn create_with_timeout(
        &self,
        config: HookConfig,
        timeout_secs: u64,
    ) -> Result<Box<dyn HookRunnerService>, HookError>;
}
