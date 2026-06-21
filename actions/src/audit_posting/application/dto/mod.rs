//! Data Transfer Objects for the Audit Posting module.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md
//! Implements: Contract Freeze — DTO schemas for create, sign, post, queue operations
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::audit_posting::domain::SignedAuditRecord;

// ---------------------------------------------------------------------------
// Create Record DTOs
// ---------------------------------------------------------------------------

/// Input for creating and posting an audit record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRecordInput {
    /// Globally unique execution identifier.
    pub execution_id: uuid::Uuid,

    /// The GitHub Actions workflow run ID, if applicable.
    pub run_id: Option<u64>,

    /// The GitHub Actions workflow name.
    pub workflow_name: Option<String>,

    /// The repository owner/name (e.g. "my-org/my-repo").
    pub repository: String,

    /// The git ref (branch or tag).
    pub git_ref: Option<String>,

    /// The commit SHA.
    pub commit_sha: Option<String>,

    /// The execution mode (run, validate, plan, governance, status).
    pub mode: String,

    /// Human-readable summary of what was executed.
    pub summary: String,

    /// Optional actor identity.
    pub actor: Option<String>,

    /// Optional key-value metadata.
    pub metadata: Option<HashMap<String, String>>,

    /// Whether to sign the record with HMAC.
    pub sign: bool,

    /// Whether to post immediately after creating.
    pub post_immediately: bool,
}

/// Output from creating and optionally posting an audit record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRecordOutput {
    /// The created audit record.
    pub record: SignedAuditRecord,
    /// Whether the record was signed.
    pub signed: bool,
    /// Whether the record was posted.
    pub posted: bool,
    /// Posting result details, if attempted.
    pub post_result: Option<PostResultDto>,
}

// ---------------------------------------------------------------------------
// Post Record DTOs
// ---------------------------------------------------------------------------

/// Input for posting a signed audit record to the backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostRecordInput {
    /// The signed audit record to post.
    pub record: SignedAuditRecord,

    /// Target backend URL. Overrides the configured default if set.
    pub backend_url: Option<String>,

    /// Request timeout in seconds.
    pub timeout_secs: Option<u64>,
}

/// Output from posting an audit record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostRecordOutput {
    /// Whether posting was successful.
    pub success: bool,

    /// HTTP status code from the backend (if received).
    pub http_status: Option<u16>,

    /// Duration of the post attempt in milliseconds.
    pub duration_ms: u64,

    /// Backend URL that was contacted.
    pub backend_url: String,
}

// ---------------------------------------------------------------------------
// Sign Record DTOs
// ---------------------------------------------------------------------------

/// Input for signing an audit record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRecordInput {
    /// The audit record to sign (without signature).
    pub record: SignedAuditRecord,

    /// Key identifier to use for signing.
    /// If `None`, uses the default configured key.
    pub key_id: Option<String>,
}

/// Output from signing an audit record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRecordOutput {
    /// The signed audit record with signature populated.
    pub record: SignedAuditRecord,
    /// The signature hex string.
    pub signature: String,
    /// Key identifier used for signing.
    pub key_id: String,
}

// ---------------------------------------------------------------------------
// Verify Record DTOs
// ---------------------------------------------------------------------------

/// Input for verifying an audit record's signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRecordInput {
    /// The signed audit record to verify.
    pub record: SignedAuditRecord,

    /// Key identifier to use for verification.
    /// If `None`, tries all configured keys.
    pub key_id: Option<String>,
}

/// Output from verifying an audit record's signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRecordOutput {
    /// Whether the signature is valid.
    pub valid: bool,
    /// Key identifier used for verification (if known).
    pub key_id: Option<String>,
    /// Verification details (e.g. which key was used).
    pub detail: Option<String>,
}

// ---------------------------------------------------------------------------
// Queue DTOs
// ---------------------------------------------------------------------------

/// Input for enqueuing a failed record for retry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueRecordInput {
    /// The record that failed posting.
    pub record: SignedAuditRecord,

    /// Reason for the failure.
    pub failure_reason: String,

    /// How many retries have already been attempted.
    pub retry_count: u32,

    /// Maximum retries before dropping.
    pub max_retries: u32,
}

/// Output from an enqueue/dequeue operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueRecordOutput {
    /// The record (for dequeue) or confirmation (for enqueue).
    pub record: Option<SignedAuditRecord>,

    /// Whether the operation succeeded.
    pub success: bool,

    /// Reason if operation failed.
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Load Record DTOs
// ---------------------------------------------------------------------------

/// Input for loading an audit record from the backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadRecordInput {
    /// The execution ID to load.
    pub execution_id: uuid::Uuid,
    /// Whether to verify the signature on load.
    pub verify_signature: bool,
}

/// Output from loading an audit record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadRecordOutput {
    /// The loaded record, if found.
    pub record: Option<SignedAuditRecord>,
    /// Whether signature verification passed (if requested).
    pub signature_valid: Option<bool>,
}

// ---------------------------------------------------------------------------
// Shared DTOs
// ---------------------------------------------------------------------------

/// DTO for posting result details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostResultDto {
    /// Whether posting was successful.
    pub success: bool,
    /// HTTP status code if available.
    pub http_status: Option<u16>,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Error detail if posting failed.
    pub error_detail: Option<String>,
    /// Number of retries attempted.
    pub attempts: u32,
}

/// DTO for audit backend configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditBackendConfig {
    /// Backend URL for HTTP backend.
    pub backend_url: Option<String>,
    /// Directory path for filesystem backend.
    pub filesystem_path: Option<String>,
    /// HMAC signing key hex string.
    pub signing_key: Option<String>,
    /// HMAC signing key identifier.
    pub key_id: Option<String>,
    /// Maximum retry attempts.
    pub max_retries: u32,
    /// Base retry delay in seconds (exponential backoff).
    pub retry_delay_secs: u64,
    /// Queue capacity.
    pub queue_capacity: u32,
}
