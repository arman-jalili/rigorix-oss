//! Repository interfaces for the CLI Observability module.
//!
//! @canonical .pi/architecture/modules/observability.md
//! Implements: Contract Freeze — ObservabilityCliRepository trait
//! Issue: issue-contract-freeze
//!
//! Repositories abstract CLI-level observability data storage behind interfaces.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions

use async_trait::async_trait;

use crate::observability::domain::ObservabilityCliError;

/// Repository for CLI-level observability state.
#[async_trait]
pub trait ObservabilityCliRepository: Send + Sync {
    /// Record that tracing was initialized.
    async fn record_tracing_init(&self) -> Result<(), ObservabilityCliError>;

    /// Check if tracing has been initialized.
    async fn is_tracing_initialized(&self) -> Result<bool, ObservabilityCliError>;

    /// Record a health check result.
    async fn record_health_check(
        &self,
        check_name: &str,
        healthy: bool,
        duration_ms: u64,
    ) -> Result<(), ObservabilityCliError>;

    /// Get recent health check results.
    async fn get_health_history(
        &self,
        limit: usize,
    ) -> Result<Vec<(String, bool, u64)>, ObservabilityCliError>;

    /// Clear all recorded observability state.
    async fn clear(&self) -> Result<(), ObservabilityCliError>;
}
