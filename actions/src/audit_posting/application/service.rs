//! Service interfaces (use cases) for the Audit Posting bounded context.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md
//! Implements: Contract Freeze — AuditPostingService, AuditRecordQueue traits
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for signing, posting,
//! and queuing audit records. All methods are async and return domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::audit_posting::domain::AuditPostingError;

use super::dto::{
    CreateRecordInput, CreateRecordOutput, PostRecordInput,
    PostRecordOutput, QueueRecordInput, QueueRecordOutput, SignRecordInput, SignRecordOutput,
    VerifyRecordInput, VerifyRecordOutput,
};

/// Central service for posting HMAC-signed audit records.
///
/// Orchestrates the full audit posting workflow: creating the signed record,
/// posting it to the configured `AuditBackend`, and managing failed deliveries
/// via `AuditRecordQueue`.
///
/// # Contract (Frozen)
/// - `post_record()` is the primary entry point
/// - Records are signed before posting
/// - On failure, records are automatically enqueued for retry
/// - Configuration errors return `NotConfigured`
#[async_trait]
pub trait AuditPostingService: Send + Sync {
    /// Create a signed audit record and post it to the backend.
    ///
    /// 1. Builds the record from execution metadata
    /// 2. Signs it with HMAC-SHA256
    /// 3. Posts to the configured backend
    /// 4. On failure, enqueues for retry
    ///
    /// Returns the created record and posting result.
    async fn create_and_post(
        &self,
        input: CreateRecordInput,
    ) -> Result<CreateRecordOutput, AuditPostingError>;

    /// Post an existing signed record to the backend.
    ///
    /// If posting fails, returns the appropriate error for the caller
    /// to handle (typically enqueue for retry).
    async fn post_record(
        &self,
        input: PostRecordInput,
    ) -> Result<PostRecordOutput, AuditPostingError>;

    /// Sign an audit record with HMAC-SHA256.
    ///
    /// Requires a signing key to be configured in the environment.
    /// Returns `KeyNotAvailable` if no key is configured.
    async fn sign_record(
        &self,
        input: SignRecordInput,
    ) -> Result<SignRecordOutput, AuditPostingError>;

    /// Verify an audit record's HMAC signature.
    ///
    /// Returns `SignatureMismatch` if the signature is invalid or missing.
    async fn verify_record(
        &self,
        input: VerifyRecordInput,
    ) -> Result<VerifyRecordOutput, AuditPostingError>;

    /// Retry all pending records in the delivery queue.
    ///
    /// Returns the number of records successfully posted
    /// and the number still pending after retry.
    async fn retry_pending(&self) -> Result<RetryPendingOutput, AuditPostingError>;

    /// Get the current posting status (queue depth, backend health).
    async fn status(&self) -> Result<PostingStatusOutput, AuditPostingError>;
}

/// Queue for managing failed audit record deliveries.
///
/// Provides bounded in-memory queueing with capacity limits.
/// When the queue is full, new failed records are dropped.
#[async_trait]
pub trait AuditRecordQueue: Send + Sync {
    /// Enqueue a failed record for later retry.
    ///
    /// Returns `QueueFull` error if the queue is at capacity.
    async fn enqueue(&self, input: QueueRecordInput) -> Result<QueueRecordOutput, AuditPostingError>;

    /// Dequeue the next pending record (FIFO order).
    ///
    /// Returns `None` if the queue is empty.
    async fn dequeue(&self) -> Result<Option<QueueRecordOutput>, AuditPostingError>;

    /// Peek at the front of the queue without removing.
    async fn peek(&self) -> Result<Option<QueueRecordOutput>, AuditPostingError>;

    /// Get the current queue length.
    async fn len(&self) -> Result<u32, AuditPostingError>;

    /// Whether the queue is empty.
    async fn is_empty(&self) -> Result<bool, AuditPostingError>;

    /// Clear all pending items (e.g. on shutdown).
    async fn clear(&self) -> Result<u32, AuditPostingError>;

    /// Load all pending records from the filesystem on startup.
    ///
    /// Used by `FilesystemAuditBackend` to recover queued records
    /// after a process restart.
    async fn recover(&self) -> Result<u32, AuditPostingError>;
}

/// Output for retry_pending operation.
#[derive(Debug, Clone)]
pub struct RetryPendingOutput {
    /// Number of records successfully posted.
    pub delivered: u32,
    /// Number of records still pending after retry.
    pub still_pending: u32,
    /// Number of records permanently dropped due to max retries.
    pub dropped: u32,
}

/// Output for posting status query.
#[derive(Debug, Clone)]
pub struct PostingStatusOutput {
    /// Number of records currently in the retry queue.
    pub pending_count: u32,
    /// Whether the audit backend is configured and reachable.
    pub backend_available: bool,
    /// Total records posted since process start.
    pub total_posted: u64,
    /// Total records failed since process start.
    pub total_failed: u64,
    /// Number of records on disk (if using FilesystemAuditBackend).
    pub on_disk_count: Option<u64>,
}
