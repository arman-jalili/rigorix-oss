//! Repository interfaces for the Audit bounded context.
//!
//! @canonical .pi/architecture/modules/audit.md
//! Implements: Contract Freeze — AuditEnvelopeRepository trait
//! Issue: #13
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

use crate::audit::domain::{AuditEnvelope, AuditError};

use crate::audit::application::dto::RecordDeliveryInput;

/// Repository for persisting and retrieving audit envelopes.
///
/// Implementations may use:
/// - Local filesystem (JSON files per envelope)
/// - SQLite/Postgres database
/// - In-memory store (for testing)
///
/// # Security
/// - Implementations MUST redact sensitive event data in all log output
/// - File paths must be validated against directory traversal
#[async_trait]
pub trait AuditEnvelopeRepository: Send + Sync {
    /// Persist an audit envelope.
    ///
    /// Saves the envelope for later retrieval or replay.
    /// Returns `Internal` error on storage failure.
    async fn save(&self, envelope: &AuditEnvelope) -> Result<(), AuditError>;

    /// Retrieve an audit envelope by execution ID.
    ///
    /// Returns `None` if no envelope exists for this execution ID.
    async fn find_by_execution_id(
        &self,
        execution_id: &uuid::Uuid,
    ) -> Result<Option<AuditEnvelope>, AuditError>;

    /// List all persisted envelopes, optionally filtered by date range.
    ///
    /// Results ordered by timestamp (newest first).
    /// `limit` caps the number of results (default 100).
    async fn list(
        &self,
        since: Option<chrono::DateTime<chrono::Utc>>,
        until: Option<chrono::DateTime<chrono::Utc>>,
        limit: Option<u32>,
    ) -> Result<Vec<AuditEnvelope>, AuditError>;

    /// Delete an envelope by execution ID.
    ///
    /// No-op if the envelope doesn't exist.
    async fn delete(&self, execution_id: &uuid::Uuid) -> Result<(), AuditError>;

    /// Record delivery status for an envelope.
    ///
    /// Updates the stored envelope with delivery metadata.
    async fn record_delivery(&self, input: &RecordDeliveryInput) -> Result<(), AuditError>;

    /// Count envelopes matching optional filters.
    async fn count(
        &self,
        since: Option<chrono::DateTime<chrono::Utc>>,
        until: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<u64, AuditError>;

    /// Delete envelopes older than the given timestamp.
    ///
    /// Returns the number of deleted envelopes.
    async fn prune(&self, older_than: chrono::DateTime<chrono::Utc>) -> Result<u64, AuditError>;
}
