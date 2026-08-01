//! Tracing configuration for the Rigorix observability system.
//!
//! @canonical .pi/architecture/modules/observability.md#config
//!
//! Controls log level, output format, and tracing behavior.

/// Configuration for the tracing subsystem.
#[derive(Debug, Clone)]
pub struct TracingConfig {
    /// Default log level (e.g., "info", "debug", "warn").
    /// Can be overridden by RIGORIX_LOG env var.
    pub default_level: String,

    /// If true, use human-readable pretty-printing instead of JSON.
    pub pretty: bool,

    /// If true, write logs to stderr instead of stdout.
    ///
    /// Required for stdio-transport servers (e.g. the MCP server) where
    /// stdout is the protocol channel and must not carry log output.
    pub write_to_stderr: bool,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            default_level: "info".to_string(),
            pretty: cfg!(debug_assertions),
            write_to_stderr: false,
        }
    }
}
