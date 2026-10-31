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

/// Outcome of an explicit consumption-ledger migration (#1247).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerMigration {
    /// A bare legacy map was adopted and written in the sealed form.
    Migrated {
        /// Markers adopted from the legacy map.
        markers: usize,
        /// Terminal records (consumed/expired/superseded) in the store.
        terminal_records: usize,
        /// Terminal records the adopted ledger leaves unprotected, as
        /// `(node_id, step_name)`.
        missing: Vec<(Uuid, String)>,
        /// Whether the store file existed and was read. `false` means the
        /// comparison could not run at all (#1251).
        store_present: bool,
        /// The store path, so a refusal/report can name it.
        store_path: std::path::PathBuf,
    },
    /// The ledger already carried a wrapper and its MAC verified.
    AlreadySealed {
        /// Markers verified.
        markers: usize,
        /// Terminal records (consumed/expired/superseded) in the store.
        terminal_records: usize,
        /// Terminal records the sealed ledger leaves unprotected.
        missing: Vec<(Uuid, String)>,
        /// Whether the store file existed and was read (#1251).
        store_present: bool,
        /// The store path.
        store_path: std::path::PathBuf,
    },
}

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

    /// Verify every persisted record's MAC. Returns the number verified.
    ///
    /// Repositories without a run key have nothing to authenticate and return
    /// `Ok(0)`; the production file store overrides this for `doctor` (#1237).
    async fn verify_integrity(&self) -> Result<usize, ApprovalError> {
        Ok(0)
    }

    /// Number of consumption markers in the anti-rollback ledger.
    ///
    /// `0` means either no ledger (first run) or no terminal transition yet;
    /// `doctor` distinguishes those by the path's existence (#1244).
    async fn consumption_marker_count(&self) -> Result<usize, ApprovalError> {
        Ok(0)
    }

    /// Explicitly migrate a pre-#1244 bare-map ledger to the sealed form.
    ///
    /// This is deliberately **not** the loader's job: adopting any file that
    /// lacks the wrapper lets an attacker turn truncation back into a valid
    /// legacy ledger (#1247).
    async fn migrate_consumption_ledger(&self) -> Result<LedgerMigration, ApprovalError> {
        Err(ApprovalError::Internal(
            "this approval repository has no consumption ledger to migrate".into(),
        ))
    }
}
