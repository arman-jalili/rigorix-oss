//! Service interfaces (use cases) for the Action Entrypoint bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md
//! Implements: Contract Freeze — ActionRouter trait, ModeResolver trait
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for event routing,
//! mode resolution, and action dispatch. All methods are async and return
//! domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::action_entrypoint::domain::{ActionError, ActionMode};

use super::dto::{
    BuildContextInput, BuildContextOutput, DispatchInput, DispatchOutput, ResolveModeInput,
    ResolveModeOutput,
};

/// Routes GitHub Action events to engine orchestrator calls.
///
/// The router is stateless — all state lives in the engine. It maps
/// the resolved `ActionMode` and `ActionContext` to the appropriate
/// engine service call, formats the result into `ActionOutput`.
///
/// # Contract (Frozen)
/// - `dispatch()` is the primary entry point
/// - The router never holds state across calls
/// - All engine interaction is through trait methods
/// - Unknown events return `DispatchStatus::Skipped`
#[async_trait]
pub trait ActionRouter: Send + Sync {
    /// Dispatch based on event type and action mode.
    ///
    /// Routes the action context to the appropriate engine call:
    /// - `ActionMode::Run` → orchestrator run
    /// - `ActionMode::Plan` → orchestrator plan_only
    /// - `ActionMode::Validate` → validation loop (with fallback to run)
    /// - `ActionMode::Status` → orchestrator status
    ///
    /// # Returns
    ///
    /// `ActionOutput` with status, summary, and any annotations.
    async fn dispatch(&self, input: DispatchInput) -> Result<DispatchOutput, ActionError>;

    /// Dispatch but with a timeout.
    ///
    /// Same as `dispatch()` but with a configurable timeout.
    /// Returns `ActionError::EngineError` if the timeout is exceeded
    /// (with `is_retriable() = true`).
    async fn dispatch_with_timeout(
        &self,
        input: DispatchInput,
        timeout_secs: u64,
    ) -> Result<DispatchOutput, ActionError>;

    /// Check if this router can handle the given mode.
    ///
    /// Returns `true` for all supported `ActionMode` variants.
    /// Used by the entrypoint to decide whether to dispatch or skip.
    async fn can_handle(&self, mode: &ActionMode) -> bool;

    /// Get the list of supported execution modes.
    ///
    /// Returns all `ActionMode` variants that this router supports.
    async fn supported_modes(&self) -> Vec<ActionMode>;
}

/// Resolves the execution mode from inputs and event context.
///
/// Determines what the action should do (run, plan, validate, status)
/// based on `INPUT_MODE`, the event type, and slash commands in comments.
///
/// # Contract (Frozen)
/// - `resolve()` is the primary entry point
/// - Mode resolution follows a priority order: explicit input > event context > default
/// - Returns `ActionMode::Status` as the fallback
#[async_trait]
pub trait ModeResolver: Send + Sync {
    /// Resolve the execution mode from inputs and event context.
    ///
    /// Resolution priority:
    /// 1. Explicit `INPUT_MODE` from workflow inputs (highest priority)
    /// 2. Event context: issue_comment with /rigorix command
    /// 3. Event context: pull_request → Validate
    /// 4. Event context: workflow_dispatch → Run
    /// 5. Fallback: Status (lowest priority)
    async fn resolve(&self, input: ResolveModeInput) -> Result<ResolveModeOutput, ActionError>;

    /// Resolve mode from a raw mode string (from `INPUT_MODE`).
    ///
    /// Valid values: "run", "plan", "validate", "status", "auto".
    /// Returns `None` for unknown values (caller should fall back to event-based resolution).
    async fn resolve_from_string(&self, mode_str: &str) -> Option<ActionMode>;

    /// Resolve mode from a slash command in a comment.
    ///
    /// Maps `/rigorix run <intent>` → `ActionMode::Run`
    /// Maps `/rigorix validate <intent>` → `ActionMode::Validate`
    /// Maps `/rigorix plan <intent>` → `ActionMode::Plan`
    /// Maps `/rigorix status` → `ActionMode::Status`
    async fn resolve_from_command(
        &self,
        command_type: &str,
        intent: Option<String>,
    ) -> Result<ActionMode, ActionError>;

    /// Resolve mode from event type.
    ///
    /// - `workflow_dispatch` → Run (with intent from input)
    /// - `pull_request` → Validate
    /// - `push` → Status
    /// - `issue_comment` → depends on slash command content
    async fn resolve_from_event(
        &self,
        event_type: &str,
        event_data: &serde_json::Value,
    ) -> Result<ActionMode, ActionError>;
}

/// Builds an `ActionContext` from the environment and inputs.
///
/// Handles parsing environment variables, event payload JSON,
/// and input values into a typed `ActionContext` that can be
/// passed to the router.
///
/// # Contract (Frozen)
/// - `build()` is the primary entry point
/// - All environment reads use the `ContextRepository` trait
/// - Missing optional fields result in `None`, not errors
/// - Missing required fields result in `ActionError::MissingContext`
#[async_trait]
pub trait ContextBuilder: Send + Sync {
    /// Build an `ActionContext` from the environment.
    ///
    /// Reads:
    /// - `GITHUB_WORKSPACE` → workspace_root
    /// - `GITHUB_EVENT_NAME` + `GITHUB_EVENT_PATH` → event
    /// - `INPUT_*` → mode + configuration
    /// - `GITHUB_TOKEN` / `INPUT_GITHUB_TOKEN` → token
    async fn build(&self, input: BuildContextInput) -> Result<BuildContextOutput, ActionError>;

    /// Get the workspace root from the environment.
    ///
    /// Reads `GITHUB_WORKSPACE` and validates it exists.
    async fn get_workspace_root(&self) -> Result<String, ActionError>;

    /// Get the GitHub token from the environment.
    ///
    /// Checks `GITHUB_TOKEN` then `INPUT_GITHUB_TOKEN`.
    /// Returns `Ok(None)` if no token is set (non-fatal).
    async fn get_github_token(&self) -> Result<Option<String>, ActionError>;

    /// Parse the event payload from `GITHUB_EVENT_PATH`.
    ///
    /// Reads the JSON file and deserializes into a `GitHubEvent`.
    async fn parse_event(
        &self,
        event_name: &str,
        event_path: &str,
    ) -> Result<super::dto::ParseEventOutput, ActionError>;
}
