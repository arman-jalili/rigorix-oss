//! Service interfaces (use cases) for the Action Input bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-input.md
//! Implements: Contract Freeze — InputParsingService, EventParsingService,
//! CommentParsingService, CiDetectionService, ConfigLoadingService,
//! InputValidationService traits
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for parsing GitHub
//! Action inputs, events, comments, and configuration. All methods are async
//! and return domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::action_input::domain::{ActionInputError, CommentCommand};

use super::dto::{
    DetectCiInput, DetectCiOutput, LoadConfigInput, LoadConfigOutput, ParseCommentInput,
    ParseCommentOutput, ParseEventPayloadInput, ParseEventPayloadOutput, ParseInputsInput,
    ParseInputsOutput, ValidateInputsInput, ValidateInputsOutput,
};

/// Application service for parsing GitHub Action inputs from environment variables.
///
/// Implements the contract defined in `InputParser` from the architecture doc.
/// Reads `INPUT_<NAME>` environment variables and converts them to typed
/// `ActionInputs` struct fields.
///
/// # Contract (Frozen)
/// - `parse()` is the primary entry point
/// - Returns structured results with warnings for unparseable values
/// - Missing required inputs are reported, not panicked
#[async_trait]
pub trait InputParsingService: Send + Sync {
    /// Parse all inputs from the environment.
    ///
    /// Reads `INPUT_<NAME>` environment variables where `<NAME>` is the
    /// uppercase version of the input name with hyphens replaced by underscores.
    ///
    /// # Returns
    ///
    /// `ParseInputsOutput` containing:
    /// - The parsed `ActionInputs` (with `None` for missing optional fields)
    /// - Count of populated fields
    /// - Names of missing required fields
    /// - Warnings for values that failed to parse
    async fn parse(&self, input: ParseInputsInput) -> Result<ParseInputsOutput, ActionInputError>;

    /// Parse a single input value into its typed representation.
    ///
    /// Used by implementations to convert raw env var strings into typed values.
    /// Returns `None` if parsing fails (the warning is captured in the output).
    async fn parse_field<T: std::str::FromStr>(
        &self,
        name: &str,
        value: &str,
    ) -> Result<Option<T>, ActionInputError>;

    /// Check if a required input is present.
    ///
    /// Returns an error with the field name if the required input is missing.
    async fn require_input(&self, name: &str) -> Result<String, ActionInputError>;

    /// Read a single environment variable by name.
    ///
    /// Returns `None` if the variable is not set or empty.
    async fn read_env_var(&self, name: &str) -> Option<String>;
}

/// Application service for parsing GitHub event payload JSON.
///
/// Implements the `EventPayloadParser` component from the architecture doc.
/// Reads the `GITHUB_EVENT_PATH` file and deserializes it into structured
/// `GitHubEvent` types based on the event name.
///
/// # Contract (Frozen)
/// - `parse()` reads from `GITHUB_EVENT_PATH` by default
/// - Supports: workflow_dispatch, issue_comment, pull_request, push
/// - Unknown event types return `GitHubEvent::Unknown`
#[async_trait]
pub trait EventParsingService: Send + Sync {
    /// Parse the GitHub event payload from the specified path.
    ///
    /// Reads the JSON file at `GITHUB_EVENT_PATH`, determines the event
    /// type from `GITHUB_EVENT_NAME`, and deserializes into the matching
    /// `GitHubEvent` variant.
    async fn parse(
        &self,
        input: ParseEventPayloadInput,
    ) -> Result<ParseEventPayloadOutput, ActionInputError>;

    /// Get the event name from the environment.
    ///
    /// Reads `GITHUB_EVENT_NAME` env var. Returns error if not in CI.
    async fn get_event_name(&self) -> Result<String, ActionInputError>;

    /// Get the event payload file path from the environment.
    ///
    /// Reads `GITHUB_EVENT_PATH` env var. Returns error if not in CI.
    async fn get_event_path(&self) -> Result<String, ActionInputError>;
}

/// Application service for parsing `/rigorix` slash commands from comments.
///
/// Implements the `CommentParser` component from the architecture doc.
/// Scans issue/PR comment bodies for `/rigorix <command> [args]` patterns.
///
/// # Contract (Frozen)
/// - Commands: run, validate, plan, status, retry
/// - Returns `None` if no command prefix is found
/// - Returns `CommentCommand::Help` for unrecognized commands after `/rigorix`
/// - Case-sensitive matching against `/rigorix` prefix
#[async_trait]
pub trait CommentParsingService: Send + Sync {
    /// Parse a comment body for a `/rigorix` command.
    ///
    /// Scans the comment text for lines starting with `/rigorix`.
    /// Returns the parsed command or `None` if no command is found.
    async fn parse(&self, input: ParseCommentInput)
    -> Result<ParseCommentOutput, ActionInputError>;

    /// Parse a command and validate its arguments.
    ///
    /// Like `parse()`, but also validates that:
    /// - `run`/`validate`/`plan` commands have non-empty intent
    /// - `retry` commands have a valid UUID
    async fn parse_and_validate(
        &self,
        input: ParseCommentInput,
    ) -> Result<ParseCommentOutput, ActionInputError>;

    /// Check if a comment text starts with the command prefix.
    async fn has_command_prefix(&self, comment: &str) -> bool;

    /// Extract the command arguments string after the prefix and command word.
    ///
    /// E.g., for `/rigorix run implement feature X`, returns `"implement feature X"`.
    async fn extract_args(&self, comment: &str) -> Option<String>;

    /// Validate that the commenter has permission to execute a command.
    ///
    /// Checks against allowed actors (repo owner, collaborators, etc.).
    async fn validate_permission(
        &self,
        commenter: &str,
        command: &CommentCommand,
    ) -> Result<bool, ActionInputError>;
}

/// Application service for detecting the CI environment.
///
/// Implements the `CiDetector` component from the architecture doc.
/// Checks for CI-specific environment variables to determine the
/// runtime context and set appropriate permission modes.
///
/// # Contract (Frozen)
/// - Detects `GITHUB_ACTIONS` → `CiEnvironment::GitHubActions`
/// - Falls back to `CiEnvironment::Local`
/// - CI defaults to `workspace_write` permission mode
/// - Local defaults to `prompt` permission mode
#[async_trait]
pub trait CiDetectionService: Send + Sync {
    /// Detect the CI environment type.
    ///
    /// Checks for `GITHUB_ACTIONS` env var to determine if running
    /// inside GitHub Actions. Returns `CiEnvironment::Local` otherwise.
    async fn detect(&self, input: DetectCiInput) -> Result<DetectCiOutput, ActionInputError>;

    /// Get the default permission mode for the current environment.
    ///
    /// Returns `"workspace_write"` in CI, `"prompt"` locally.
    async fn default_permission_mode(&self) -> String;

    /// Check if the current environment is CI.
    async fn is_ci(&self) -> bool;

    /// Get the workspace root path.
    ///
    /// Reads `GITHUB_WORKSPACE` in CI, returns current dir locally.
    async fn workspace_root(&self) -> Result<String, ActionInputError>;
}

/// Application service for loading and merging action configuration.
///
/// Implements the `ConfigLoader` component from the architecture doc.
/// Merges configuration from multiple sources with proper precedence:
/// 1. `INPUT_*` env vars (runtime overrides, highest priority)
/// 2. CLI arguments (if run outside GitHub Actions)
/// 3. `action.yml` defaults
/// 4. Engine defaults
///
/// # Contract (Frozen)
/// - `load()` is the primary entry point
/// - Environment overrides take highest precedence
/// - Missing `action.yml` is non-fatal (uses defaults)
/// - Returns structured output indicating which sources contributed
#[async_trait]
pub trait ConfigLoadingService: Send + Sync {
    /// Load and merge configuration from all available sources.
    ///
    /// Resolution order (highest priority wins):
    /// 1. Environment variable overrides (`INPUT_*`)
    /// 2. CLI argument overrides
    /// 3. `action.yml` default values
    /// 4. Compiled-in defaults
    async fn load(&self, input: LoadConfigInput) -> Result<LoadConfigOutput, ActionInputError>;

    /// Load default values from `action.yml`.
    ///
    /// Parses the `action.yml` file in the current working directory
    /// and extracts the `inputs` section defaults.
    async fn load_yml_defaults(
        &self,
        path_override: Option<String>,
    ) -> Result<crate::action_input::domain::ActionInputs, ActionInputError>;

    /// Merge environment overrides into the base config.
    ///
    /// Applies `INPUT_*` env var values on top of the base config.
    /// Environment values that are set override the base.
    async fn apply_env_overrides(
        &self,
        base: crate::action_input::domain::ActionInputs,
    ) -> Result<crate::action_input::domain::ActionInputs, ActionInputError>;

    /// Merge CLI argument overrides into the base config.
    async fn apply_cli_overrides(
        &self,
        base: crate::action_input::domain::ActionInputs,
        overrides: std::collections::HashMap<String, String>,
    ) -> Result<crate::action_input::domain::ActionInputs, ActionInputError>;

    /// Resolve a merged `ActionInputs` (with `Option` fields) into a
    /// concrete `ActionConfig` (all fields resolved to defaults).
    async fn resolve(
        &self,
        merged: crate::action_input::domain::ActionInputs,
    ) -> Result<crate::action_input::domain::ActionConfig, ActionInputError>;
}

/// Application service for validating parsed inputs.
///
/// Validates that parsed inputs meet semantic constraints before
/// they are passed to the action router or engine.
///
/// # Contract (Frozen)
/// - Validates required fields based on mode
/// - Validates numeric fields are within acceptable bounds
/// - Detects conflicting inputs
/// - Returns structured validation results
#[async_trait]
pub trait InputValidationService: Send + Sync {
    /// Validate parsed inputs against semantic constraints.
    ///
    /// Checks include:
    /// - Mode-specific required fields (e.g., intent required for `run` mode)
    /// - Numeric bounds (e.g., max_llm_calls > 0)
    /// - No conflicting inputs
    async fn validate(
        &self,
        input: ValidateInputsInput,
    ) -> Result<ValidateInputsOutput, ActionInputError>;

    /// Validate that intent is provided when required.
    ///
    /// Intent is required for `run`, `validate`, and `plan` modes.
    async fn require_intent_for_mode(
        &self,
        mode: &str,
        intent: &Option<String>,
    ) -> Result<(), ActionInputError>;
}
