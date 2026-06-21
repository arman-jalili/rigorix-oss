//! Factory interfaces for constructing Action Entrypoint domain objects.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md
//! Implements: Contract Freeze — ActionRouterFactory, ContextFactory, OutputFactory, ModeFactory traits
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of complex domain objects,
//! allowing implementations to inject dependencies and apply defaults
//! without exposing construction logic to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured domain object
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::action_entrypoint::domain::{
    ActionContext, ActionError, ActionMode, ActionOutput, GitHubEvent,
};

/// Factory for constructing `ActionRouter` implementations.
///
/// Encapsulates the wiring of engine dependencies (orchestrator service,
/// validation loop service) into a configured router.
#[async_trait]
pub trait ActionRouterFactory: Send + Sync {
    /// Create a new `ActionRouter` with the given engine dependencies.
    ///
    /// The factory resolves engine service references and wires them
    /// into the router. Returns an error if required dependencies
    /// are unavailable.
    async fn create_router(
        &self,
    ) -> Result<Box<dyn crate::action_entrypoint::application::ActionRouter>, ActionError>;

    /// Create a router with validation loop support.
    ///
    /// Like `create_router()`, but also wires the validation loop
    /// service for `ActionMode::Validate` dispatch.
    async fn create_router_with_validation(
        &self,
    ) -> Result<Box<dyn crate::action_entrypoint::application::ActionRouter>, ActionError>;
}

/// Factory for constructing `ActionContext` instances.
///
/// Handles environment variable reading, event payload parsing, and
/// input resolution to build a fully configured `ActionContext`.
#[async_trait]
pub trait ContextFactory: Send + Sync {
    /// Build an `ActionContext` from environment data.
    ///
    /// Reads workspace root, event info, and token from the environment.
    async fn build_from_env(&self) -> Result<ActionContext, ActionError>;

    /// Build an `ActionContext` from explicit values (for testing).
    async fn build_from_values(
        &self,
        workspace_root: String,
        event: GitHubEvent,
        mode: ActionMode,
        github_token: Option<String>,
    ) -> Result<ActionContext, ActionError>;

    /// Apply mode to an existing context (returns new context).
    async fn apply_mode(&self, ctx: &ActionContext, mode: ActionMode) -> ActionContext;

    /// Apply configuration parameters to a context.
    ///
    /// Sets max_iterations, max_llm_calls, max_llm_tokens, etc.
    /// from the provided map of string key-value pairs.
    async fn apply_config(
        &self,
        ctx: ActionContext,
        config: std::collections::HashMap<String, String>,
    ) -> Result<ActionContext, ActionError>;
}

/// Factory for constructing `ActionOutput` instances.
///
/// Provides convenience methods for creating common output shapes
/// (success, failure, skipped, error annotations).
#[async_trait]
pub trait OutputFactory: Send + Sync {
    /// Create a success output.
    async fn success(
        &self,
        summary: &str,
        execution_id: Option<String>,
    ) -> Result<ActionOutput, ActionError>;

    /// Create a failure output from an error.
    async fn failure(
        &self,
        error: &ActionError,
        summary: &str,
    ) -> Result<ActionOutput, ActionError>;

    /// Create a skipped output.
    async fn skipped(&self, reason: &str) -> Result<ActionOutput, ActionError>;

    /// Create an output with annotations.
    async fn with_annotations(
        &self,
        base: ActionOutput,
        annotations: Vec<crate::action_entrypoint::domain::WorkflowAnnotation>,
    ) -> Result<ActionOutput, ActionError>;

    /// Create an output from a mode mismatch (unsupported mode).
    async fn unsupported_mode(&self, mode: &ActionMode) -> Result<ActionOutput, ActionError>;
}

/// Factory for constructing `ActionMode` values.
///
/// Handles parsing mode strings and event context to produce
/// resolved `ActionMode` values with proper intent extraction.
#[async_trait]
pub trait ModeFactory: Send + Sync {
    /// Create a Run mode with the given intent.
    async fn run(intent: String) -> ActionMode;

    /// Create a Plan mode with the given intent.
    async fn plan(intent: String) -> ActionMode;

    /// Create a Validate mode with the given intent.
    async fn validate(intent: String) -> ActionMode;

    /// Create a Status mode.
    async fn status() -> ActionMode;

    /// Parse a mode from a string.
    ///
    /// Valid values: "run", "plan", "validate", "status".
    /// Returns `None` for unknown values.
    async fn from_string(mode_str: &str, intent: Option<String>) -> Option<ActionMode>;
}
