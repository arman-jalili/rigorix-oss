//! Service interfaces for the CLI Observability module.
//!
//! @canonical .pi/architecture/modules/observability.md
//! Implements: Contract Freeze — TracingInitializer trait
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for observability
//! (tracing, health checks, metrics). All methods are async and return
//! domain error types. Implementations reside in the infrastructure layer.
//!
//! # Contract (Frozen)
//! - Every observability use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::cli_boundary::domain::error::CliError;
use crate::configuration::domain::config::{LogFormat, LogLevel};

/// Initializes tracing for the CLI boundary.
///
/// Wraps engine's tracing initialization with CLI-specific config.
/// Respects `RIGORIX_LOG` env var for log level override.
///
/// # Contract (Frozen)
/// - Called once at startup, after config is loaded
/// - Must be idempotent — second call is a no-op
/// - Panics if tracing is already initialized (tracing-subscriber behaviour)
#[async_trait]
pub trait TracingInitializer: Send + Sync {
    /// Initialize tracing with the given log level and format.
    ///
    /// Uses the engine's `observability::init_tracing()` under the hood.
    /// If `RIGORIX_LOG` env var is set, it overrides the `log_level` param.
    async fn init_tracing(
        &self,
        log_level: LogLevel,
        log_format: LogFormat,
    ) -> Result<(), CliError>;

    /// Initialize tracing with safe defaults (pretty, info level).
    async fn init_default_tracing(&self) -> Result<(), CliError>;

    /// Check if tracing has been initialized.
    fn is_initialized(&self) -> bool;

    /// Initialize health checks for the CLI boundary.
    ///
    /// Registers CLI components as health check providers with the
    /// engine's `HealthService`.
    async fn init_health_checks(&self) -> Result<usize, CliError>;
}
