//! Data Transfer Objects for the Configuration module.
//!
//! @canonical .pi/architecture/modules/configuration.md#config
//! Implements: Contract Freeze — DTO schemas for Config, Secret, validation
//! Issue: #2
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API, TOML for file config)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types (no framework-specific annotations)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::configuration::domain::{
    AuditConfig, EnforcementPreset, LoggingConfig, OrchestratorConfig, RiskLevel, Secret,
};

// ---------------------------------------------------------------------------
// Config Load DTOs
// ---------------------------------------------------------------------------

/// Input for loading configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoadConfigInput {
    /// Optional explicit path to `rigorix.toml`.
    /// If `None`, searches CWD and then `~/.rigorix/config.toml`.
    pub config_path: Option<String>,

    /// Override environment variable prefix (default: `RIGORIX__`).
    pub env_prefix: Option<String>,

    /// Optional CLI flag overrides applied after file+env loading.
    /// Keys use `__` separator for nesting (e.g. `orchestrator__max_parallel_tasks`).
    pub cli_overrides: Option<HashMap<String, String>>,

    /// Allow empty config (use all defaults).
    pub allow_empty: bool,
}

/// Output from loading configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadConfigOutput {
    /// The fully resolved configuration.
    pub config: ConfigDto,

    /// Ordered list of which sources contributed values.
    /// First = highest priority (overrides later entries).
    pub sources_used: Vec<String>,

    /// Whether validation passed.
    pub valid: bool,
}

/// Flattened DTO representation of the full `Config` aggregate.
///
/// Used for API responses and serialization round-trips.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigDto {
    pub orchestrator: OrchestratorConfig,
    pub logging: LoggingConfig,
    pub tools: ToolsConfigDto,
    pub enforcement: EnforcementPreset,
    pub audit: AuditConfig,
    pub llm: LlmConfigDto,
}

/// Tools sub-configuration DTO.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolsConfigDto {
    /// Per-tool risk overrides.
    pub tool_overrides: HashMap<String, RiskLevel>,
    pub auto_confirm_low: bool,
    pub require_review_medium: bool,
    pub dry_run_high: bool,
}

/// LLM sub-configuration DTO (redacts the API key).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmConfigDto {
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
    pub max_tokens: u32,
    pub temperature: f64,
    /// API key — redacted in display, transparent in serialization.
    pub api_key: Secret,
}

// ---------------------------------------------------------------------------
// Validate Config DTOs
// ---------------------------------------------------------------------------

/// Input for validating configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidateConfigInput {
    /// The configuration to validate.
    pub config: ConfigDto,

    /// Safety hard-caps to validate against.
    /// If `None`, uses built-in defaults.
    pub safety_caps: Option<SafetyCaps>,
}

/// Safety hard-caps for enforcement validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SafetyCaps {
    /// Maximum allowed parallel tasks.
    pub max_parallel_tasks_cap: u32,

    /// Maximum allowed retries per node.
    pub max_retries_cap: u32,

    /// Maximum timeout in seconds.
    pub max_timeout_secs_cap: u64,

    /// Maximum LLM tokens per request.
    pub max_tokens_cap: u32,
}

impl Default for SafetyCaps {
    fn default() -> Self {
        Self {
            max_parallel_tasks_cap: 10,
            max_retries_cap: 5,
            max_timeout_secs_cap: 600,
            max_tokens_cap: 16384,
        }
    }
}

/// Output from validating configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidateConfigOutput {
    /// Whether all validation checks passed.
    pub valid: bool,

    /// List of validation errors (empty if valid).
    pub errors: Vec<ValidationError>,

    /// List of warnings (non-blocking issues).
    pub warnings: Vec<String>,
}

/// A single validation error with structured context.
///
/// TODO: Consider migrating to `crate::common::validation::ValidationError`
/// which provides a shared `rule` + `node_id` pattern.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationError {
    /// The field that failed validation.
    pub field: String,
    /// Human-readable error message.
    pub message: String,
    /// The invalid value, if representable.
    pub value: Option<String>,
}

// ---------------------------------------------------------------------------
// Secret DTOs
// ---------------------------------------------------------------------------

/// Input for loading a secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadSecretInput {
    /// The environment variable name (e.g. "ANTHROPIC_API_KEY").
    pub env_var: String,

    /// Optional fallback value if env var is not set.
    pub fallback: Option<String>,

    /// Whether the secret is required.
    pub required: bool,
}

/// Output from loading a secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadSecretOutput {
    /// The loaded secret (empty if not found and not required).
    pub secret: Secret,

    /// Whether the secret was loaded from environment or fallback.
    pub source: String,
}
