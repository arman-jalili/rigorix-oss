//! Repository interfaces for the Audit Posting bounded context.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md
//! Implements: Contract Freeze — AuditBackend, FilesystemAuditBackend traits
//! Issue: issue-contract-freeze
//!
//! Repositories abstract data access behind interfaces, allowing
//! implementations to use HTTP, filesystem, or mock storage without
//! coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces
//!
//! # Open-Core Boundary
//!
//! `AuditBackend` is the open-core boundary trait. The OSS default
//! implementation is `FilesystemAuditBackend`. Premium/enterprise
//! implementations may provide HTTP-based backends.

use async_trait::async_trait;

use crate::audit_posting::domain::{AuditPostingError, SignedAuditRecord};

use crate::audit_posting::application::dto::{
    PostRecordInput, PostRecordOutput, LoadRecordInput, LoadRecordOutput,
};

// ---------------------------------------------------------------------------
// AuditBackend — Open-Core Boundary
// ---------------------------------------------------------------------------

/// Open-core boundary trait for posting audit records to a remote backend.
///
/// # Contract (Frozen)
/// - This is the open-core boundary: OSS provides `FilesystemAuditBackend`
/// - Enterprise implementations may provide HTTP, S3, or other backends
/// - All methods are async
/// - All methods return `Result<_, AuditPostingError>`
///
/// # Security
/// - Implementations MUST NOT log record content (may contain sensitive metadata)
/// - Implementations MUST validate paths against directory traversal
/// - Implementations MUST handle HMAC keys securely (not logged, not serialized)
///
/// # OSS Default
///
/// The default OSS implementation is `FilesystemAuditBackend` which stores
/// records as JSON files on the local filesystem with atomic writes.
#[async_trait]
pub trait AuditBackend: Send + Sync {
    /// Post a signed audit record to the backend.
    ///
    /// Persists the record to the backend storage. For the filesystem
    /// implementation, this writes a JSON file. For HTTP implementations,
    /// this sends an HTTP POST request.
    async fn post(&self, input: PostRecordInput) -> Result<PostRecordOutput, AuditPostingError>;

    /// Load an audit record by execution ID.
    ///
    /// Returns `None` if no record exists for this execution ID.
    async fn load(&self, input: LoadRecordInput) -> Result<LoadRecordOutput, AuditPostingError>;

    /// List all records, optionally filtered by date range.
    ///
    /// Results ordered by timestamp (newest first).
    /// `limit` caps the number of results (default 100).
    async fn list(
        &self,
        since: Option<chrono::DateTime<chrono::Utc>>,
        until: Option<chrono::DateTime<chrono::Utc>>,
        limit: Option<u32>,
    ) -> Result<Vec<SignedAuditRecord>, AuditPostingError>;

    /// Delete a record by execution ID.
    ///
    /// No-op if the record doesn't exist.
    async fn delete(&self, execution_id: &uuid::Uuid) -> Result<(), AuditPostingError>;

    /// Check whether the backend is available and reachable.
    ///
    /// For filesystem: checks that the storage directory exists and is writable.
    /// For HTTP: performs a health check request.
    async fn health_check(&self) -> Result<bool, AuditPostingError>;

    /// Count records matching optional filters.
    async fn count(
        &self,
        since: Option<chrono::DateTime<chrono::Utc>>,
        until: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<u64, AuditPostingError>;

    /// Delete records older than the given timestamp.
    ///
    /// Returns the number of deleted records.
    async fn prune(&self, older_than: chrono::DateTime<chrono::Utc>) -> Result<u64, AuditPostingError>;
}

// ---------------------------------------------------------------------------
// FilesystemAuditBackend — OSS Default
// ---------------------------------------------------------------------------

/// OSS default implementation of `AuditBackend` using local filesystem storage.
///
/// Stores signed audit records as JSON files in a configurable directory.
/// Uses atomic write-then-rename to prevent partial writes.
///
/// # Contract (Frozen)
/// - Records are stored as `{execution_id}.json` files
/// - Uses atomic write via `write()` + `rename()` on supported platforms
/// - File naming: `<storage_dir>/<execution_id>/record.json`
/// - Supports directory nesting to avoid too many files in one directory
///
/// # OSS Licensing
///
/// This is the default OSS implementation provided with the open-core
/// distribution. All OSS users get this backend for free. Enterprise
/// users may opt for HTTP-based backends.
#[async_trait]
pub trait FilesystemAuditBackend: AuditBackend {
    /// Get the storage directory path.
    fn storage_dir(&self) -> &str;

    /// Resolve the file path for a given execution ID.
    ///
    /// Returns the absolute path to the record file.
    fn record_path(&self, execution_id: &uuid::Uuid) -> String;

    /// Serialize a record to its JSON string for storage.
    ///
    /// This is separated from `post()` so that tests can verify
    /// serialization without writing to disk.
    fn serialize_record(&self, record: &SignedAuditRecord) -> Result<String, AuditPostingError>;

    /// Deserialize a record from its JSON string.
    fn deserialize_record(&self, json: &str) -> Result<SignedAuditRecord, AuditPostingError>;
}
