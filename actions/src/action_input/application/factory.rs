//! Factory interfaces for constructing Action Input domain objects.
//!
//! @canonical actions/.pi/architecture/modules/action-input.md
//! Implements: Contract Freeze — InputFactory, ConfigFactory, EventFactory traits
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

use crate::action_input::domain::{ActionConfig, ActionInputError, ActionInputs, GitHubEvent};

/// Factory for constructing `ActionInputs` from raw environment data.
///
/// Implementations handle parsing string values from environment variables
/// into typed `ActionInputs` fields, applying default values and validation.
#[async_trait]
pub trait InputFactory: Send + Sync {
    /// Build an `ActionInputs` from a map of raw string key-value pairs.
    ///
    /// Keys should be the env var name without prefix (e.g., `INTENT`, `MODE`).
    /// Values are raw strings from environment variables.
    async fn build_from_env_map(
        &self,
        env_map: std::collections::HashMap<String, String>,
    ) -> Result<ActionInputs, ActionInputError>;

    /// Build an `ActionInputs` from parsed CLI arguments.
    ///
    /// Accepts a flat map of argument names to values.
    async fn build_from_cli_args(
        &self,
        args: std::collections::HashMap<String, String>,
    ) -> Result<ActionInputs, ActionInputError>;

    /// Create an `ActionInputs` with all default values.
    fn defaults(&self) -> ActionInputs;

    /// Parse a single value into the correct type for the given field.
    ///
    /// Returns the parsed value as a string representation or an error.
    async fn parse_field(
        &self,
        field: &str,
        raw_value: &str,
    ) -> Result<Option<String>, ActionInputError>;
}

/// Factory for constructing `ActionConfig` from merged sources.
///
/// Handles the final resolution of `ActionInputs` (with `Option` fields)
/// into `ActionConfig` (all fields resolved to concrete values).
#[async_trait]
pub trait ConfigFactory: Send + Sync {
    /// Resolve an `ActionInputs` into a concrete `ActionConfig`.
    ///
    /// All `Option` fields in `ActionInputs` are resolved:
    /// - If `Some(value)`, use the value
    /// - If `None`, use the default from `ActionConfig::default()`
    async fn resolve_config(&self, inputs: ActionInputs) -> Result<ActionConfig, ActionInputError>;

    /// Apply environment-specific overrides to a config.
    ///
    /// For CI environments: sets permission_mode to `workspace_write`
    /// if not explicitly set.
    async fn apply_environment_overrides(
        &self,
        config: ActionConfig,
        is_ci: bool,
    ) -> Result<ActionConfig, ActionInputError>;

    /// Merge two `ActionInputs` with the second taking precedence.
    async fn merge(
        &self,
        base: ActionInputs,
        overrides: ActionInputs,
    ) -> Result<ActionInputs, ActionInputError>;
}

/// Factory for constructing `GitHubEvent` from raw JSON event payloads.
///
/// Handles deserialization of the event payload file with event-type-aware
/// parsing. Different event types have different JSON structures.
#[async_trait]
pub trait EventFactory: Send + Sync {
    /// Build a `GitHubEvent` from raw JSON content and event name.
    async fn build_from_json(
        &self,
        event_name: &str,
        json_content: &str,
    ) -> Result<GitHubEvent, ActionInputError>;

    /// Determine the event type from the event name string.
    fn classify_event_type(&self, event_name: &str) -> &'static str;

    /// Create an unknown event with the given name.
    fn unknown_event(&self, event_name: &str) -> GitHubEvent;
}
