//! Orchestrator-specific configuration.
//!
//! @canonical .pi/architecture/modules/orchestrator.md#config
//! Implements: Contract Freeze — OrchestratorConfig
//! Issue: #338
//!
//! Configuration values specific to the orchestrator's execution lifecycle,
//! independent of sub-service configuration (handled by their own configs).
//!
//! # Contract (Frozen)
//! - All configuration values have sensible defaults
//! - Configuration is validated at builder time, not during orchestration
//! - Fields are public for the builder to construct

use serde::{Deserialize, Serialize};

/// Orchestrator-specific configuration.
///
/// Controls event buffer sizing, audit behaviour, cancellation timeouts,
/// and other orchestrator-level settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// Maximum number of events buffered in the EventBus before draining.
    /// Larger values allow more events to accumulate before the record is built,
    /// but consume more memory. Default: 10_000.
    pub event_buffer_capacity: usize,

    /// Whether to send audit envelopes after execution completes.
    /// Disable for local/development runs where no audit backend is available.
    /// Default: true.
    pub audit_enabled: bool,

    /// Timeout in seconds for the full `run()` lifecycle.
    /// If the execution exceeds this duration, a `Cancelled` state is forced.
    /// Default: 300 (5 minutes).
    pub execution_timeout_secs: u64,

    /// Timeout in seconds for the planning phase specifically.
    /// Default: 60.
    pub planning_timeout_secs: u64,

    /// Timeout in seconds for state persistence operations.
    /// Default: 10.
    pub state_persistence_timeout_secs: u64,

    /// Whether to save intermediate execution state after each DAG node.
    /// Enables resumption after partial failures but adds I/O overhead.
    /// Default: false.
    pub save_intermediate_state: bool,

    /// Whether to propagate cancellation signals to all sub-services
    /// when a single phase fails.
    /// Default: true.
    pub propagate_cancellation: bool,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            event_buffer_capacity: 10_000,
            audit_enabled: true,
            execution_timeout_secs: 300,
            planning_timeout_secs: 60,
            state_persistence_timeout_secs: 10,
            save_intermediate_state: false,
            propagate_cancellation: true,
        }
    }
}
