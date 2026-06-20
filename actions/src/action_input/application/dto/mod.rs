//! Data Transfer Objects for the Action Input module.
//!
//! @canonical actions/.pi/architecture/modules/action-input.md
//! Implements: Contract Freeze — DTO schemas for input parsing, comment parsing,
//! CI detection, and config loading operations
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for event processing)
//! - Validation constraints are documented in field docs

use serde::{Deserialize, Serialize};

use crate::action_input::domain::{
    ActionConfig, ActionInputs, CiEnvironment, CommentCommand, GitHubEvent,
};

// ---------------------------------------------------------------------------
// Input Parsing DTOs
// ---------------------------------------------------------------------------

/// Input for parsing GitHub Action inputs from the environment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParseInputsInput {
    /// Override the environment variable prefix (default: `INPUT_`).
    pub env_prefix: Option<String>,

    /// Override environment for testing (maps variable name → value).
    /// If `None`, reads from real `std::env::var`.
    pub env_override: Option<std::collections::HashMap<String, String>>,
}

/// Output from parsing GitHub Action inputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseInputsOutput {
    /// The parsed action inputs.
    pub inputs: ActionInputs,
    /// Number of input fields that were populated (non-None).
    pub populated_count: u32,
    /// Names of required inputs that were missing.
    pub missing_required: Vec<String>,
    /// Warnings from parsing (e.g., unparseable numeric fields).
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Event Payload Parsing DTOs
// ---------------------------------------------------------------------------

/// Input for parsing a GitHub event payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseEventPayloadInput {
    /// Path to the `GITHUB_EVENT_PATH` file.
    pub event_path: String,
    /// Override JSON content for testing (if `None`, reads from file).
    pub content_override: Option<String>,
}

/// Output from parsing a GitHub event payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseEventPayloadOutput {
    /// The parsed GitHub event.
    pub event: GitHubEvent,
    /// The raw event name from `GITHUB_EVENT_NAME`.
    pub event_name: String,
    /// File size of the event payload in bytes.
    pub file_size_bytes: u64,
}

// ---------------------------------------------------------------------------
// Comment Parsing DTOs
// ---------------------------------------------------------------------------

/// Input for parsing a comment body for `/rigorix` commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseCommentInput {
    /// The raw comment body text.
    pub comment_body: String,
    /// The command prefix to look for (default: `/rigorix`).
    pub command_prefix: Option<String>,
    /// The issue/PR number where the comment was posted.
    pub issue_number: u64,
    /// The username of the commenter.
    pub commenter: String,
}

/// Output from parsing a comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseCommentOutput {
    /// The parsed command, if any.
    pub command: Option<CommentCommand>,
    /// Whether a command was found.
    pub found: bool,
    /// The matched command type as a string (for event logging).
    pub command_type: Option<String>,
}

// ---------------------------------------------------------------------------
// CI Detection DTOs
// ---------------------------------------------------------------------------

/// Input for detecting the CI environment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DetectCiInput {
    /// Override environment for testing (maps variable name → value).
    /// If `None`, reads from real `std::env::var`.
    pub env_override: Option<std::collections::HashMap<String, String>>,

    /// Override the permission mode.
    /// If `None`, uses CI-aware defaults.
    pub permission_mode_override: Option<String>,
}

/// Output from CI environment detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectCiOutput {
    /// The detected CI environment.
    pub environment: CiEnvironment,
    /// The resolved permission mode for the environment.
    pub permission_mode: String,
    /// Whether the environment is CI.
    pub is_ci: bool,
}

// ---------------------------------------------------------------------------
// Config Loading DTOs
// ---------------------------------------------------------------------------

/// Input for loading merged action configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoadConfigInput {
    /// Override environment for testing (maps variable name → value).
    pub env_override: Option<std::collections::HashMap<String, String>>,

    /// Override action.yml content for testing (YAML string).
    /// If `None`, reads from `action.yml` in CWD.
    pub action_yml_override: Option<String>,

    /// Override CLI arguments for testing.
    pub cli_overrides: Option<std::collections::HashMap<String, String>>,

    /// Allow empty/missing action.yml (use all defaults).
    pub allow_empty_yml: bool,
}

/// Output from loading merged action configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadConfigOutput {
    /// The fully resolved action configuration.
    pub config: ActionConfig,
    /// Whether action.yml defaults were found and applied.
    pub yml_defaults_applied: bool,
    /// Whether environment overrides were applied.
    pub env_overrides_applied: bool,
    /// Whether CLI overrides were applied.
    pub cli_overrides_applied: bool,
    /// List of sources that contributed to the final config (ordered by priority).
    pub sources: Vec<String>,
}

// ---------------------------------------------------------------------------
// Validation DTOs
// ---------------------------------------------------------------------------

/// Input for validating parsed inputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateInputsInput {
    /// The parsed action inputs to validate.
    pub inputs: ActionInputs,
    /// The detected CI environment (for mode-based validation).
    pub ci_environment: CiEnvironment,
}

/// A single validation error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationError {
    /// The field that failed validation.
    pub field: String,
    /// Human-readable error message.
    pub message: String,
    /// The invalid value, if representable.
    pub value: Option<String>,
}

/// Output from validating parsed inputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateInputsOutput {
    /// Whether all validation checks passed.
    pub valid: bool,
    /// List of validation errors (empty if valid).
    pub errors: Vec<ValidationError>,
    /// List of warnings (non-blocking issues).
    pub warnings: Vec<String>,
}
