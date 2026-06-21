//! Repository interfaces for the Action Entrypoint bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md
//! Implements: Contract Freeze — ContextRepository trait
//! Issue: issue-contract-freeze
//!
//! Repositories abstract data access behind interfaces, allowing
//! implementations to use environment variables, filesystem, or mock
//! storage without coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::action_entrypoint::domain::ActionError;

/// Repository for reading GitHub Action execution context.
///
/// Abstracts `std::env::var` / `std::env::vars` and filesystem access
/// to event payload files behind a trait for testability.
///
/// # Security
/// - Implementations MUST NOT log sensitive values (tokens, secrets)
/// - File paths must be validated against directory traversal
#[async_trait]
pub trait ContextRepository: Send + Sync {
    /// Read a single environment variable by name.
    ///
    /// Returns `Ok(None)` if the variable is not set.
    async fn read_env_var(&self, name: &str) -> Result<Option<String>, ActionError>;

    /// Read all environment variables matching a prefix.
    ///
    /// Returns a map of variable names (without prefix) to values.
    /// E.g., prefix `INPUT_` with `INPUT_MODE=run` returns `{"MODE": "run"}`.
    async fn read_env_vars(
        &self,
        prefix: &str,
    ) -> Result<std::collections::HashMap<String, String>, ActionError>;

    /// Check if an environment variable is set.
    async fn has_env_var(&self, name: &str) -> Result<bool, ActionError>;

    /// Get the workspace root path.
    ///
    /// Reads `GITHUB_WORKSPACE` in CI.
    async fn workspace_root(&self) -> Result<String, ActionError>;

    /// Get the GitHub event name.
    ///
    /// Reads `GITHUB_EVENT_NAME`.
    async fn event_name(&self) -> Result<String, ActionError>;

    /// Get the GitHub event payload file path.
    ///
    /// Reads `GITHUB_EVENT_PATH`.
    async fn event_path(&self) -> Result<String, ActionError>;

    /// Read the GitHub event payload file content.
    ///
    /// Reads the file at the path specified by `GITHUB_EVENT_PATH`.
    async fn read_event_payload(&self, path: &str) -> Result<String, ActionError>;

    /// Get the GitHub token.
    ///
    /// Checks `GITHUB_TOKEN` then `INPUT_GITHUB_TOKEN`.
    async fn github_token(&self) -> Result<Option<String>, ActionError>;

    /// Get the GitHub API URL base.
    ///
    /// Reads `GITHUB_API_URL` (default: `https://api.github.com`).
    async fn github_api_url(&self) -> Result<String, ActionError>;

    /// Get all CI-related environment variables.
    ///
    /// Returns variables like `GITHUB_ACTIONS`, `GITHUB_EVENT_NAME`,
    /// `GITHUB_EVENT_PATH`, `GITHUB_ACTOR`, `GITHUB_REPOSITORY`, etc.
    async fn read_ci_env_vars(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, ActionError>;

    /// Resolve the absolute path for a potentially relative path.
    ///
    /// If `path` is relative, resolves it against the workspace root.
    async fn resolve_path(&self, path: &str) -> Result<String, ActionError>;
}
