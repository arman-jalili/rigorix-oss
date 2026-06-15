//! Observability infrastructure for Rigorix.
//!
//! @canonical .pi/architecture/modules/observability.md
//!
//! Provides structured tracing, health checking, and metrics collection.
//! This module is the centralized observability layer consumed by all
//! other modules.

pub mod health;
pub mod metrics;
pub mod span_privacy;
pub mod tracing_config;

pub use tracing_config::TracingConfig;

/// Initialize the tracing subscriber with the given configuration.
///
/// Sets up:
/// - JSON logging to stdout (production) or human-readable (dev)
/// - Level control via `RIGORIX_LOG` env var (default: `info`)
/// - Span privacy filter to redact sensitive fields
pub fn init_tracing(config: &TracingConfig) -> Result<(), Box<dyn std::error::Error>> {
    let filter = tracing_subscriber::EnvFilter::builder()
        .with_env_var("RIGORIX_LOG")
        .with_default_directive(config.default_level.parse()?)
        .from_env_lossy();

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_line_number(true);

    if config.pretty {
        subscriber.pretty().init();
    } else {
        subscriber.json().init();
    }

    tracing::info!("Tracing initialized (level={})", config.default_level);
    Ok(())
}
