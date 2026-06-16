//! CLI configuration value types.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#config
//! Implements: Contract Freeze — CliConfig value objects
//! Issue: issue-contract-freeze
//!
//! CLI-specific configuration that complements the engine's `Config`.
//! These are display/output settings, not engine behaviour settings.
//!
//! # Contract (Frozen)
//! - `CliConfig` is the merged CLI config (flags → env → file → engine defaults)
//! - `OutputFormat` defines how CLI output is rendered
//! - All fields have sensible defaults
//! - Configuration is immutable after loading

use serde::{Deserialize, Serialize};

/// The output rendering format for CLI commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    /// Human-readable, colorized terminal output.
    #[serde(rename = "pretty")]
    Pretty,

    /// Structured JSON output for CI/CD integration.
    #[serde(rename = "json")]
    Json,

    /// Minimal output — only errors and essential status.
    #[serde(rename = "quiet")]
    Quiet,
}

impl OutputFormat {
    /// Returns `true` if this is human-readable output.
    pub fn is_human_readable(&self) -> bool {
        matches!(self, OutputFormat::Pretty)
    }

    /// Returns `true` if this is machine-readable output.
    pub fn is_machine_readable(&self) -> bool {
        matches!(self, OutputFormat::Json)
    }

    /// Returns `true` if this is minimal output.
    pub fn is_quiet(&self) -> bool {
        matches!(self, OutputFormat::Quiet)
    }
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Pretty
    }
}

/// CLI-specific configuration, merged from CLI flags, environment variables,
/// `rigorix.toml`, and engine defaults.
///
/// This struct represents the final resolved CLI configuration. It is
/// immutable after loading and is passed to all CLI components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    /// The output format for CLI commands.
    ///
    /// CLI flag: `--format pretty|json|quiet`
    /// Env var: `RIGORIX_FORMAT`
    /// Config file: `cli.output_format`
    /// Default: `pretty`
    pub output_format: OutputFormat,

    /// Whether to enable the TUI renderer during execution.
    ///
    /// CLI flag: `--tui` / `--no-tui`
    /// Env var: `RIGORIX_TUI_ENABLED`
    /// Default: `true` (auto-detect TTY)
    pub tui_enabled: bool,

    /// Whether to enable colorized output.
    ///
    /// CLI flag: `--color auto|always|never`
    /// Env var: `RIGORIX_COLOR`
    /// Default: `auto`
    pub color: ColorMode,

    /// The log level for the CLI and engine.
    ///
    /// CLI flag: `--log-level trace|debug|info|warn|error`
    /// Env var: `RIGORIX_LOG`
    /// Default: `info` for normal, `debug` for verbose
    pub log_level: LogLevel,

    /// The log format.
    ///
    /// CLI flag: `--log-format pretty|json`
    /// Default: `pretty`
    pub log_format: LogFormat,

    /// Path to a custom `rigorix.toml` config file.
    ///
    /// CLI flag: `--config <PATH>`
    /// Default: `./rigorix.toml` or `.rigorix/config.toml`
    pub config_path: Option<String>,

    /// Whether to use the TUI even if no TTY is detected.
    ///
    /// CLI flag: `--force-tui`
    /// Default: `false`
    pub force_tui: bool,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            output_format: OutputFormat::default(),
            tui_enabled: true,
            color: ColorMode::Auto,
            log_level: LogLevel::Info,
            log_format: LogFormat::Pretty,
            config_path: None,
            force_tui: false,
        }
    }
}

/// Whether to use color in terminal output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorMode {
    /// Automatically enable color if connected to a TTY.
    #[serde(rename = "auto")]
    Auto,

    /// Always enable color output.
    #[serde(rename = "always")]
    Always,

    /// Never use color output.
    #[serde(rename = "never")]
    Never,
}

impl ColorMode {
    /// Returns `true` if color should be used given whether a TTY is available.
    pub fn should_color(&self, is_tty: bool) -> bool {
        match self {
            ColorMode::Auto => is_tty,
            ColorMode::Always => true,
            ColorMode::Never => false,
        }
    }
}

/// Log verbosity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    #[serde(rename = "trace")]
    Trace,
    #[serde(rename = "debug")]
    Debug,
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "warn")]
    Warn,
    #[serde(rename = "error")]
    Error,
}

impl LogLevel {
    /// Returns the corresponding `tracing` level filter string.
    pub fn as_tracing_filter(&self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }

    /// All possible log level variants.
    pub const fn all() -> [LogLevel; 5] {
        [
            LogLevel::Trace,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
        ]
    }
}

/// Log output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogFormat {
    /// Human-readable log lines with timestamps and colors.
    #[serde(rename = "pretty")]
    Pretty,

    /// Structured JSON log lines (one object per line).
    #[serde(rename = "json")]
    Json,
}

impl LogFormat {
    /// Returns `true` if this is JSON log format.
    pub fn is_json(&self) -> bool {
        matches!(self, LogFormat::Json)
    }
}
