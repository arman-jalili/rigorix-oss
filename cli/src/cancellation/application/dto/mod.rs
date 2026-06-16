//! Data Transfer Objects for the CLI Cancellation module.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — CLI cancellation DTO schemas
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for CLI cancellation operations.
//! They are used by the `SignalHandler` trait and cancellation service traits.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for CI/CD output)
//! - Fields use reasonable Rust types (no framework-specific annotations)

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Shutdown DTOs
// ---------------------------------------------------------------------------

/// Input for requesting a graceful shutdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GracefulShutdownInput {
    /// Human-readable reason for the shutdown.
    pub reason: Option<String>,

    /// Timeout in seconds for in-flight tasks to complete.
    pub timeout_secs: u64,
}

impl Default for GracefulShutdownInput {
    fn default() -> Self {
        Self {
            reason: None,
            timeout_secs: 30,
        }
    }
}

/// Input for requesting an immediate abort.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImmediateShutdownInput {
    /// Human-readable reason for the abort.
    pub reason: Option<String>,
}

/// Output from a shutdown operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownOutput {
    /// Whether the shutdown completed successfully.
    pub success: bool,
    /// Number of in-flight tasks that were cancelled.
    pub tasks_cancelled: u32,
    /// Duration of the shutdown process in milliseconds.
    pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// Signal Handler Status DTOs
// ---------------------------------------------------------------------------

/// Input for querying the signal handler status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalStatusInput;

/// Output from querying the signal handler status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalStatusOutput {
    /// Whether the signal handler is installed.
    pub installed: bool,
    /// The current shutdown level (none, graceful, immediate).
    pub current_level: SignalLevel,
    /// The double-press window in seconds.
    pub double_press_window_secs: u64,
    /// Timestamp of the last signal received (ISO 8601).
    pub last_signal_at: Option<String>,
}

/// The current signal/shutdown level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalLevel {
    /// No shutdown in progress.
    None,
    /// Graceful shutdown in progress.
    Graceful,
    /// Immediate abort in progress.
    Immediate,
}
