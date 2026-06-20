//! Service interfaces (use cases) for the Hook System.
//!
//! @canonical .pi/architecture/modules/hooks.md#hook-service
//! Implements: Contract Freeze — HookRunnerService trait
//! Issue: #410
//!
//! This trait defines the application-level operations for hook execution:
//! running hooks for each lifecycle event, getting runner status, and
//! managing hook configuration at runtime.
//!
//! All methods are synchronous (hooks execute as child processes, not
//! async tasks). The abort signal supports cooperative cancellation.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - Methods accept references to `HookAbortSignal` for cancellation
//! - No implementation — only contract signatures

use crate::hooks::domain::abort::HookAbortSignal;
use crate::hooks::domain::config::HookConfig;
use crate::hooks::domain::error::HookError;
use crate::hooks::domain::result::HookRunResult;

use super::dto::{
    HookRunnerStatus, RunPostToolUseFailureInput, RunPostToolUseFailureOutput, RunPostToolUseInput,
    RunPostToolUseOutput, RunPreToolUseInput, RunPreToolUseOutput,
};

/// Service for executing hook commands across all lifecycle events.
///
/// Orchestrates hook execution: spawns hook processes, reads their
/// JSON responses, aggregates results, and supports cancellation via
/// `HookAbortSignal`. Hooks are executed in registration order.
///
/// # Safety
/// - Hooks run as child processes with the same privileges as the engine
/// - Hook output is validated as JSON before processing
/// - Aborted hooks are killed, not just signalled
pub trait HookRunnerService: Send + Sync {
    /// Run all PreToolUse hooks for a tool invocation.
    ///
    /// Executes every registered PreToolUse command, passing the tool
    /// context via stdin JSON. Results are aggregated: first deny wins,
    /// last permission_override wins, last updated_input wins.
    ///
    /// If `abort_signal` is set before or during execution, remaining
    /// hooks are skipped and the result is marked as cancelled.
    fn run_pre_tool_use(
        &self,
        input: RunPreToolUseInput,
        abort_signal: Option<&HookAbortSignal>,
    ) -> Result<RunPreToolUseOutput, HookError>;

    /// Run all PostToolUse hooks after successful tool execution.
    ///
    /// Executes every registered PostToolUse command with the tool's
    /// output context. Hooks can append feedback messages but cannot
    /// modify input or block execution retroactively.
    fn run_post_tool_use(
        &self,
        input: RunPostToolUseInput,
        abort_signal: Option<&HookAbortSignal>,
    ) -> Result<RunPostToolUseOutput, HookError>;

    /// Run all PostToolUseFailure hooks after failed tool execution.
    ///
    /// Executes every registered PostToolUseFailure command with the
    /// tool's error context. Hooks can append diagnostic messages
    /// and trigger recovery scripts.
    fn run_post_tool_use_failure(
        &self,
        input: RunPostToolUseFailureInput,
        abort_signal: Option<&HookAbortSignal>,
    ) -> Result<RunPostToolUseFailureOutput, HookError>;

    /// Get the current hook runner status.
    fn status(&self) -> HookRunnerStatus;

    /// Update the hook configuration at runtime.
    ///
    /// Replaces the current hook command lists with the provided ones.
    /// This allows dynamic reconfiguration without restarting.
    fn reconfigure(&self, config: HookConfig) -> Result<(), HookError>;

    /// Create a new `HookAbortSignal` tied to this runner's lifecycle.
    ///
    /// When the runner is shut down, all signals created through this
    /// method are triggered automatically.
    fn create_abort_signal(&self) -> HookAbortSignal;
}

/// Convenience trait for running a single hook command.
///
/// Lower-level interface for executing one hook command in isolation.
/// Used internally by `HookRunnerService` implementations.
pub trait HookCommandExecutor: Send + Sync {
    /// Execute a single hook command with the given stdin payload.
    ///
    /// The command is spawned as a child process with the JSON payload
    /// piped to stdin. The stdout is read and parsed as `HookStdoutResponse`.
    ///
    /// Returns the parsed response, or a `HookError` if execution failed.
    fn execute_command(
        &self,
        command: &str,
        stdin_payload: &serde_json::Value,
        abort_signal: Option<&HookAbortSignal>,
    ) -> Result<HookRunResult, HookError>;
}
