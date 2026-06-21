//! SignedAuditRecord domain entity.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md#signed-audit-record
//! Implements: Contract Freeze — SignedAuditRecord value object with HMAC signature
//! Issue: issue-contract-freeze
//!
//! HMAC-signed audit record that wraps execution metadata with integrity
//! protection. Used by `AuditPoster` for delivery to remote audit backends
//! and by `FilesystemAuditBackend` (OSS default) for local persistence.
//!
//! # Contract (Frozen)
//! - `SignedAuditRecord` is the value object for all audit records
//! - HMAC-SHA256 signing is mandatory via `signature` field
//! - All fields are public for direct construction by the application layer
//! - Construction happens via `AuditRecordFactory`
//! - Verification of integrity happens via `AuditBackend::verify_record`

use serde::{Deserialize, Serialize};

/// HMAC-signed audit record wrapping execution metadata.
///
/// Built after execution completes by the application layer and delivered
/// to the configured audit backend via `AuditPoster`. The HMAC signature
/// provides integrity verification against tampering.
///
/// # Signing
///
/// The signature is computed over the canonical JSON serialization of the
/// record fields (excluding the `signature` field itself) using HMAC-SHA256
/// with a shared secret key configured in the action environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedAuditRecord {
    /// Globally unique execution identifier (UUID v4).
    pub execution_id: uuid::Uuid,

    /// Timestamp when the record was created (ISO 8601 / UTC).
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// The GitHub Actions workflow run ID, if applicable.
    pub run_id: Option<u64>,

    /// The GitHub Actions workflow name.
    pub workflow_name: Option<String>,

    /// The repository owner/name (e.g. "my-org/my-repo").
    pub repository: String,

    /// The git ref (branch or tag) that triggered this execution.
    pub git_ref: Option<String>,

    /// The commit SHA this execution ran on.
    pub commit_sha: Option<String>,

    /// The execution mode (run, validate, plan, governance, status).
    pub mode: String,

    /// Human-readable summary of what was executed.
    pub summary: String,

    /// HMAC-SHA256 signature for record integrity verification.
    ///
    /// Computed over the canonical JSON of all other fields.
    /// `None` if the record has not been signed yet (pre-signing state).
    pub signature: Option<String>,

    /// Optional actor identity (user or bot that triggered the action).
    pub actor: Option<String>,

    /// Optional key-value metadata for extensibility.
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

/// Posting status of an audit record.
///
/// Tracks the lifecycle of a record from creation through delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostingStatus {
    /// Record created but not yet posted.
    Pending,
    /// Record successfully posted to backend.
    Posted,
    /// Record failed to post and is queued for retry.
    Failed,
    /// Record was permanently dropped after exhausting retries.
    Dropped,
}
