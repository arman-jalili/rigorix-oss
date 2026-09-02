//! ApprovalRepository — durable approval-record persistence.
//!
//! @canonical .pi/architecture/modules/approval.md#infrastructure
//! Implements: Contract Freeze — ApprovalRepository trait
//! Issue: #786 (approval epic — contract freeze); implementation in
//!   ISSUE-APPROVALSERVICE (#792)
//!
//! Persists/loads `ApprovalRecord`s via state persistence (`ExecutionState`).
//! Records are node-scoped and single-use.
//!
//! # Contract (Frozen)
//! - `save`: upsert the record for `node_id`
//! - `load`: current record for `node_id`, or `None`
//! - `delete`: remove the record for `node_id`
//! - All operations surface `ApprovalError` (storage failures map to
//!   `ApprovalError::Internal`)

use async_trait::async_trait;
use uuid::Uuid;

use crate::approval::domain::{ApprovalError, ApprovalRecord};

/// Durable repository for approval records.
#[async_trait]
pub trait ApprovalRepository: Send + Sync {
    /// Persist (or replace) the approval record for its node.
    ///
    /// # Errors
    /// - `ApprovalError::Internal` — storage failure
    async fn save(&self, record: &ApprovalRecord) -> Result<(), ApprovalError>;

    /// Load the current approval record for a node.
    ///
    /// Returns `None` when the node was never approved (or the record was
    /// superseded and purged).
    async fn load(&self, node_id: Uuid) -> Result<Option<ApprovalRecord>, ApprovalError>;

    /// Delete the approval record for a node (compaction / cleanup).
    async fn delete(&self, node_id: Uuid) -> Result<(), ApprovalError>;
}
