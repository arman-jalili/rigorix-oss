//! Repository interfaces for the Action Input bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-input.md
//! Implements: Contract Freeze — InputRepository, ConfigRepository, EventRepository traits
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

use crate::action_input::domain::ActionInputError;

/// Repository for reading environment variables.
///
/// Abstracts `std::env::var` / `std::env::vars` behind a trait for
/// testability. Implementations can use real environment variables
/// or an in-memory map.
///
/// # Security
/// - Implementations MUST NOT log environment variable values
/// - Variable names (keys) are safe for logging
#[async_trait]
pub trait InputRepository: Send + Sync {
    /// Read a single environment variable by name.
    ///
    /// Returns `Ok(None)` if the variable is not set.
    async fn read_env_var(&self, name: &str) -> Result<Option<String>, ActionInputError>;

    /// Read all environment variables matching a prefix.
    ///
    /// Returns a map of variable names (without prefix) to values.
    /// E.g., prefix `INPUT_` with `INPUT_MODE=run` returns `{"MODE": "run"}`.
    async fn read_env_vars(
        &self,
        prefix: &str,
    ) -> Result<std::collections::HashMap<String, String>, ActionInputError>;

    /// Check if an environment variable is set.
    async fn has_env_var(&self, name: &str) -> Result<bool, ActionInputError>;

    /// Get the workspace root path.
    ///
    /// Reads `GITHUB_WORKSPACE` in CI, falls back to current dir.
    async fn workspace_root(&self) -> Result<String, ActionInputError>;

    /// Get all CI-related environment variables.
    ///
    /// Returns variables like `GITHUB_ACTIONS`, `GITHUB_EVENT_NAME`,
    /// `GITHUB_EVENT_PATH`, `GITHUB_ACTOR`, etc.
    async fn read_ci_env_vars(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, ActionInputError>;
}

/// Repository for reading action configuration sources.
///
/// Abstracts filesystem access to `action.yml` and CLI arguments
/// behind a trait for testability.
///
/// # Security
/// - File paths must be validated against directory traversal
/// - YAML parsing must not execute arbitrary code
#[async_trait]
pub trait ConfigRepository: Send + Sync {
    /// Read the `action.yml` file content.
    ///
    /// Searches CWD for `action.yml` by default.
    /// Returns `Ok(None)` if the file doesn't exist (non-fatal).
    async fn read_action_yml(
        &self,
        path_override: Option<&str>,
    ) -> Result<Option<String>, ActionInputError>;

    /// Parse default input values from `action.yml` content.
    ///
    /// Extracts the `inputs` section from the YAML and returns
    /// a map of input name to default value.
    async fn parse_yml_defaults(
        &self,
        yaml_content: &str,
    ) -> Result<std::collections::HashMap<String, serde_yaml::Value>, ActionInputError>;

    /// Read CLI argument overrides.
    ///
    /// Returns parsed CLI arguments as a flat map of name → value.
    /// Returns empty map if no CLI args are available.
    async fn read_cli_args(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, ActionInputError>;

    /// Resolve the full path to `action.yml`.
    ///
    /// Returns the absolute path if the file exists.
    async fn resolve_action_yml_path(&self) -> Result<Option<String>, ActionInputError>;
}

/// Repository for reading GitHub event payloads.
///
/// Abstracts filesystem access to the `GITHUB_EVENT_PATH` JSON file
/// behind a trait for testability.
#[async_trait]
pub trait EventRepository: Send + Sync {
    /// Read the GitHub event payload file content.
    ///
    /// Reads the file at the path specified by `GITHUB_EVENT_PATH`.
    async fn read_event_payload(&self, path: &str) -> Result<String, ActionInputError>;

    /// Get the `GITHUB_EVENT_PATH` from the environment.
    async fn get_event_path(&self) -> Result<Option<String>, ActionInputError>;

    /// Get the `GITHUB_EVENT_NAME` from the environment.
    async fn get_event_name(&self) -> Result<Option<String>, ActionInputError>;

    /// Get the `GITHUB_SHA` from the environment.
    async fn get_head_sha(&self) -> Result<Option<String>, ActionInputError>;

    /// Get the `GITHUB_REF` from the environment.
    async fn get_ref(&self) -> Result<Option<String>, ActionInputError>;
}
