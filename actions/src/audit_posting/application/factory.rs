//! Factory interfaces for constructing Audit Posting domain objects.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md
//! Implements: Contract Freeze — AuditRecordFactory, AuditBackendFactory traits
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

use crate::audit_posting::domain::{AuditPostingError, SignedAuditRecord};

use crate::audit_posting::infrastructure::repository::AuditBackend;

use super::dto::{AuditBackendConfig, CreateRecordInput, SignRecordInput, SignRecordOutput};

/// Factory for constructing `SignedAuditRecord` values.
///
/// Handles building records from execution metadata, applying defaults,
/// and optionally computing HMAC signatures.
#[async_trait]
pub trait AuditRecordFactory: Send + Sync {
    /// Build a `SignedAuditRecord` from execution event data.
    ///
    /// Populates all required fields, applies defaults for optional fields,
    /// and optionally signs the record with HMAC-SHA256.
    async fn create_record(
        &self,
        input: CreateRecordInput,
    ) -> Result<SignedAuditRecord, AuditPostingError>;

    /// Sign an audit record with HMAC-SHA256.
    ///
    /// Computes the signature over the canonical JSON of the record's
    /// data fields (excluding the `signature` field).
    /// Returns `KeyNotAvailable` if no signing key is configured.
    async fn sign(&self, input: SignRecordInput) -> Result<SignRecordOutput, AuditPostingError>;

    /// Verify an audit record's HMAC signature.
    ///
    /// Recomputes the signature and compares it to the stored signature.
    /// Returns `SignatureMismatch` if they don't match.
    async fn verify(&self, record: &SignedAuditRecord) -> Result<bool, AuditPostingError>;
}

/// Factory for constructing `AuditBackend` instances.
///
/// Creates the appropriate backend type based on configuration.
#[async_trait]
pub trait AuditBackendFactory: Send + Sync {
    /// Create an `AuditBackend` from configuration.
    ///
    /// - If `backend_url` is set, creates an `HttpAuditBackend`
    /// - If `filesystem_path` is set, creates a `FilesystemAuditBackend`
    /// - If neither is set, returns `NotConfigured`
    async fn create(
        &self,
        config: AuditBackendConfig,
    ) -> Result<Box<dyn AuditBackend>, AuditPostingError>;

    /// Create the default filesystem-based audit backend.
    ///
    /// Uses the configured filesystem path or a sensible default.
    async fn create_default(&self) -> Result<Box<dyn AuditBackend>, AuditPostingError>;
}
