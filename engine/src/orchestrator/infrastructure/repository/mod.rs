//! Repository interfaces for the Orchestrator bounded context.
//!
//! @canonical .pi/architecture/modules/orchestrator.md
//! Implements: Contract Freeze — ExecutionRecordRepository trait
//! Issue: #338
//!
//! Repositories abstract data access behind interfaces, allowing
//! implementations to use local filesystem, database, or mock storage
//! without coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::orchestrator::domain::{ExecutionRecord, OrchestratorError};

/// Repository for persisting and retrieving execution records.
///
/// Implementations may use:
/// - Local filesystem (JSON files per record)
/// - SQLite/Postgres database
/// - In-memory store (for testing)
///
/// # Security
/// - Implementations MUST redact sensitive data in all log output
/// - File paths must be validated against directory traversal
#[async_trait]
pub trait ExecutionRecordRepository: Send + Sync {
    /// Persist an execution record.
    ///
    /// Saves the record for later retrieval or audit replay.
    /// Returns `Internal` error on storage failure.
    async fn save(&self, record: &ExecutionRecord) -> Result<(), OrchestratorError>;

    /// Retrieve an execution record by execution ID.
    ///
    /// Returns `None` if no record exists for this execution ID.
    async fn find_by_execution_id(
        &self,
        execution_id: &uuid::Uuid,
    ) -> Result<Option<ExecutionRecord>, OrchestratorError>;

    /// List all persisted records, optionally filtered by date range.
    ///
    /// Results ordered by started_at (newest first).
    /// `limit` caps the number of results (default 100).
    async fn list(
        &self,
        since: Option<chrono::DateTime<chrono::Utc>>,
        until: Option<chrono::DateTime<chrono::Utc>>,
        limit: Option<u32>,
    ) -> Result<Vec<ExecutionRecord>, OrchestratorError>;

    /// Delete a record by execution ID.
    ///
    /// No-op if the record doesn't exist.
    async fn delete(&self, execution_id: &uuid::Uuid) -> Result<(), OrchestratorError>;

    /// Count records matching optional filters.
    async fn count(
        &self,
        since: Option<chrono::DateTime<chrono::Utc>>,
        until: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<u64, OrchestratorError>;

    /// Delete records older than the given timestamp.
    ///
    /// Returns the number of deleted records.
    async fn prune(&self, older_than: chrono::DateTime<chrono::Utc>) -> Result<u64, OrchestratorError>;
}
