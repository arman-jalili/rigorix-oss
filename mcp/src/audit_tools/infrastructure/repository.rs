//! Repository interfaces for the Audit Tools bounded context.
//!
//! @canonical .pi/architecture/modules/audit-tools.md#repositories
//! Implements: Contract Freeze — AuditRepository trait
//!
//! Repositories abstract audit data access behind an interface, allowing
//! implementations to query rigorix-engine directly, use a local cache,
//! or connect to an external audit store — without coupling domain logic
//! to infrastructure.
//!
//! Note: Audit Tools is read-only — repositories only provide query methods,
//! no create/update/delete operations.
//!
//! # Contract (Frozen)
//!
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::audit_tools::domain::error::AuditError;
use crate::audit_tools::domain::value::{AuditEnvelope, AuditFilter, AuditSummary};
use crate::execution_tools::domain::value::ExecutionId;

/// Repository for audit records.
///
/// Abstracts querying audit data behind an interface. All methods are
/// read-only — no mutation of audit data.
///
/// # Contract (Frozen)
///
/// - `find_by_execution_id` returns `Err(AuditError::NotFound)` if no record exists
/// - `find_many` returns results ordered by completion time (newest first)
/// - `compute_summary` aggregates all records in the time range
/// - Implementations MUST be thread-safe (Send + Sync)
#[async_trait]
pub trait AuditRepository: Send + Sync {
    /// Find an audit record by its execution ID.
    ///
    /// # Errors
    /// - `AuditError::NotFound` if the execution ID does not exist
    /// - `AuditError::EngineNotAvailable` if the engine is unreachable
    async fn find_by_execution_id(
        &self,
        execution_id: &ExecutionId,
    ) -> Result<AuditEnvelope, AuditError>;

    /// Find audit records matching the given filter criteria.
    ///
    /// Returns results ordered by completion time (newest first).
    ///
    /// # Errors
    /// - `AuditError::InvalidFilter` if filter parameters are invalid
    /// - `AuditError::EngineNotAvailable` if the engine is unreachable
    async fn find_many(&self, filter: AuditFilter) -> Result<Vec<AuditEnvelope>, AuditError>;

    /// Compute aggregate audit statistics over the given time range.
    ///
    /// # Errors
    /// - `AuditError::InvalidFilter` if time range is invalid
    /// - `AuditError::EngineNotAvailable` if the engine is unreachable
    async fn compute_summary(
        &self,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<AuditSummary, AuditError>;
}
