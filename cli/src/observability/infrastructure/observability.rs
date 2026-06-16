//! Observability infrastructure interfaces for the CLI boundary.
//!
//! @canonical .pi/architecture/modules/observability.md
//! Implements: Contract Freeze — TracingInitializer trait
//! Issue: #253
//!
//! Initializes and manages observability (tracing, health, metrics)
//! for the CLI boundary. Wraps engine observability contracts with
//! CLI-specific configuration.
//!
//! # Contract (Frozen)
//! - `TracingInitializer` configures and initializes the tracing subscriber
//! - Accepts log level and format from CLI config
//! - Respects `RIGORIX_LOG` env var override
//! - Implementations must be idempotent (calling twice is a no-op after first)
//! - Engine's `init_tracing` is called under the hood

use async_trait::async_trait;

use crate::cli_boundary::domain::error::CliError;
use crate::configuration::domain::config::{LogFormat, LogLevel};

/// Initializes tracing for the CLI boundary.
///
/// Wraps engine's tracing initialization with CLI-specific config.
/// Respects `RIGORIX_LOG` env var for log level override.
///
/// # Lifecycle
/// - Called once at startup, after config is loaded
/// - Must be idempotent — second call is a no-op
/// - Panics if tracing is already initialized (tracing-subscriber behaviour)
#[async_trait]
pub trait TracingInitializer: Send + Sync {
    /// Initialize tracing with the given log level and format.
    ///
    /// Uses the engine's `observability::init_tracing()` under the hood.
    /// If `RIGORIX_LOG` env var is set, it overrides the `log_level` param.
    ///
    /// # Arguments
    /// * `log_level` - Minimum log level (trace, debug, info, warn, error)
    /// * `log_format` - Output format (pretty for dev, json for production)
    ///
    /// # Errors
    /// Returns `CliError::Internal` if tracing cannot be initialized.
    async fn init_tracing(
        &self,
        log_level: LogLevel,
        log_format: LogFormat,
    ) -> Result<(), CliError>;

    /// Initialize tracing with safe defaults (pretty, info level).
    ///
    /// Useful for early initialization before config is fully loaded.
    async fn init_default_tracing(&self) -> Result<(), CliError>;

    /// Check if tracing has been initialized.
    fn is_initialized(&self) -> bool;

    /// Initialize health checks for the CLI boundary.
    ///
    /// Registers CLI components as health check providers with the
    /// engine's `HealthService`. Components include:
    /// - Config loader (can read config files)
    /// - Template engine (templates loaded and valid)
    /// - LLM provider (API key configured, reachable)
    ///
    /// Returns the number of health checks registered.
    async fn init_health_checks(&self) -> Result<usize, CliError>;
}
