//! Repository interfaces for the CLI Cancellation module.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — CancellationCliRepository trait
//! Issue: issue-contract-freeze
//!
//! Repositories abstract CLI-level cancellation data storage behind interfaces,
//! allowing implementations to use in-memory state, filesystem persistence,
//! or mock storage without coupling CLI logic to infrastructure.
//!
//! These repositories handle CLI-level concerns (signal state tracking,
//! shutdown configuration). They are distinct from the engine's cancellation
//! repositories which handle execution-level cancellation state.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::cancellation::application::dto::SignalLevel;
use crate::cancellation::domain::CancellationCliError;

/// Repository for CLI-level cancellation state.
///
/// Handles persistence and retrieval of signal handler state,
/// shutdown configuration, and cancellation history at the CLI layer.
///
/// # Contract (Frozen)
/// - Read operations return current state or cached configuration
/// - State mutations are atomic
/// - All methods are safe to call concurrently
#[async_trait]
pub trait CancellationCliRepository: Send + Sync {
    /// Store the current signal level.
    async fn set_signal_level(&self, level: SignalLevel) -> Result<(), CancellationCliError>;

    /// Retrieve the current signal level.
    async fn get_signal_level(&self) -> Result<SignalLevel, CancellationCliError>;

    /// Record a signal event with timestamp.
    async fn record_signal(
        &self,
        level: SignalLevel,
        elapsed_ms: u64,
    ) -> Result<(), CancellationCliError>;

    /// Get the timestamp of the last signal received.
    async fn last_signal_timestamp(&self) -> Result<Option<String>, CancellationCliError>;

    /// Clear all recorded signal state.
    async fn clear(&self) -> Result<(), CancellationCliError>;

    /// Check if the signal handler is installed.
    async fn is_installed(&self) -> Result<bool, CancellationCliError>;
}
