//! Config domain entity.
//!
//! Top-level configuration with multi-source loading from `rigorix.toml`,
//! environment variables (`RIGORIX__*`), and programmatic defaults with
//! layered merging.
//!
//! # Contract (Frozen)
//! - `Config` is the root aggregate holding all sub-configurations
//! - All fields are public for direct access by application services
//! - Construction happens via `Config::load()` (defined in service trait)
//!   or directly from deserialized TOML

use serde::{Deserialize, Serialize};

/// Top-level configuration aggregate.
///
/// Loaded at startup and shared across all components via the orchestrator.
/// Multi-source merging: CLI flags > ENV vars > rigorix.toml (CWD) >
/// ~/.rigorix/config.toml > compiled defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// Execution parameters (parallelism, retries, timeouts).
    pub orchestrator: OrchestratorConfig,

    /// Logging configuration (level, format, destination).
    pub logging: LoggingConfig,

    /// Tool settings including risk configuration.
    pub tools: ToolsConfig,

    /// Enforcement preset selection.
    pub enforcement: EnforcementPreset,

    /// Audit backend configuration.
    pub audit: AuditConfig,

    /// LLM provider settings.
    pub llm: LlmConfig,
}

/// Orchestrator execution parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// Maximum number of parallel tasks to run simultaneously.
    pub max_parallel_tasks: u32,

    /// Maximum retry attempts per node.
    pub max_retries: u32,

    /// Default timeout in seconds for node execution.
    pub default_timeout_secs: u64,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_parallel_tasks: 4,
            max_retries: 3,
            default_timeout_secs: 120,
        }
    }
}

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error).
    pub level: LogLevel,

    /// Output format (text, json).
    pub format: LogFormat,

    /// Log destination (stderr, stdout, file path).
    pub destination: LogDestination,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            format: LogFormat::Text,
            destination: LogDestination::Stderr,
        }
    }
}

/// Supported log levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Supported log output formats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogFormat {
    Text,
    Json,
}

/// Log output destination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogDestination {
    Stderr,
    Stdout,
    File(String),
}

/// Tool settings including risk configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolsConfig {
    /// Risk configuration for tool execution.
    pub risk: RiskConfig,
}

/// Per-tool risk overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    /// Map of tool name to risk level override.
    pub tool_overrides: std::collections::HashMap<String, RiskLevel>,

    /// Automatically confirm low-risk operations.
    pub auto_confirm_low: bool,

    /// Require user review for medium-risk operations.
    pub require_review_medium: bool,

    /// Dry-run high-risk operations instead of executing.
    pub dry_run_high: bool,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            tool_overrides: std::collections::HashMap::new(),
            auto_confirm_low: true,
            require_review_medium: true,
            dry_run_high: true,
        }
    }
}

/// Risk level classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

/// Enforcement preset selection.
///
/// Controls how aggressively the execution enforcer applies safety limits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum EnforcementPreset {
    /// Standard safety limits — suitable for normal operation.
    #[default]
    Default,
    /// Stricter limits — suitable for production or untrusted code.
    Advanced,
    /// Maximum safety limits — suitable for high-risk operations.
    Aggressive,
}

/// Audit backend configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    /// Enable audit logging.
    pub enabled: bool,

    /// Backend URL for audit event submission.
    pub backend_url: Option<String>,

    /// Maximum retry attempts for audit delivery.
    pub max_retries: u32,

    /// Circuit breaker threshold (failures before opening).
    pub circuit_breaker_threshold: u32,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            backend_url: None,
            max_retries: 3,
            circuit_breaker_threshold: 5,
        }
    }
}

/// LLM provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Provider name (e.g. "anthropic", "openai", "deepseek").
    pub provider: LlmProvider,

    /// Model identifier (e.g. "claude-sonnet-4-6", "gpt-4o").
    pub model: String,

    /// Base URL for API (optional, for custom endpoints).
    pub base_url: Option<String>,

    /// Maximum tokens per request.
    pub max_tokens: u32,

    /// Temperature for generation (0.0 — 1.0).
    pub temperature: f64,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: LlmProvider::Anthropic,
            model: String::from("claude-sonnet-4-6"),
            base_url: None,
            max_tokens: 4096,
            temperature: 0.7,
        }
    }
}

/// Supported LLM providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmProvider {
    Anthropic,
    OpenAI,
    DeepSeek,
    #[serde(other)]
    Custom,
}
