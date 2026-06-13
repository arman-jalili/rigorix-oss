//! Factory interfaces for constructing Cancellation domain objects.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — CancellationManagerFactory trait
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of complex domain objects,
//! allowing implementations to inject dependencies and apply defaults
//! without exposing construction logic to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured domain object
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::cancellation::domain::CancellationError;

use super::service::CancellationService;

/// Factory for constructing `CancellationService` instances.
///
/// Handles creation of the cancellation manager with appropriate
/// `CancellationToken` wiring, watch channel setup, and defaults
/// for graceful shutdown timeouts.
#[async_trait]
pub trait CancellationManagerFactory: Send + Sync {
    /// Create a new `CancellationService` with default settings.
    ///
    /// Uses a default graceful shutdown timeout of 30 seconds.
    async fn create_default(&self) -> Result<Box<dyn CancellationService>, CancellationError>;

    /// Create a `CancellationService` with an explicit graceful timeout.
    ///
    /// `graceful_timeout_secs` controls how long `await_shutdown` will
    /// wait for running tasks before force-aborting.
    async fn create_with_timeout(
        &self,
        graceful_timeout_secs: u64,
    ) -> Result<Box<dyn CancellationService>, CancellationError>;

    /// Create a `CancellationService` that is already linked to an
    /// existing parent `CancellationToken`.
    ///
    /// Useful when the orchestrator wants to create child cancellation
    /// scopes that propagate from a parent token.
    async fn create_child(
        &self,
        parent_token: tokio_util::sync::CancellationToken,
        graceful_timeout_secs: u64,
    ) -> Result<Box<dyn CancellationService>, CancellationError>;

    /// Register a `CleanupHandler` for a specific task type.
    ///
    /// During shutdown, all registered handlers are invoked with
    /// their associated task IDs. Multiple handlers may be registered
    /// for the same task type.
    async fn register_cleanup_handler(
        &self,
        task_type: &str,
        handler: Box<dyn super::service::CleanupHandler>,
    );
}
