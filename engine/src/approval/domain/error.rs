//! ApprovalError — typed error enum for all approval binding failure modes.
//!
//! @canonical .pi/architecture/modules/approval.md#error-handling
//! Canonical Reference: .pi/architecture/modules/approval.md#error-handling
//! Implements: Contract Freeze — ApprovalError enum + `is_retriable()`
//! Issue: #786 (approval epic — contract freeze)
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `ApprovalError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to the orchestrator error type via `#[from]` at the
//!   orchestrator / execution-engine boundary
//!
//! # Recovery
//! - `NotFound`: not retriable — node was never approved; human action required
//! - `AlreadyConsumed` / `Expired`: not retriable — re-approval required
//! - `IntentMismatch`: **not retriable by design** — re-approval is the only
//!   recovery; auto-retry would be a replay loop
//! - `ScopeVerificationUnavailable`: retriable — retry the oracle; skip with
//!   an explicit marker if persistently unavailable
//! - `InvalidState` / `Internal`: unexpected — treat as fatal for the operation

use thiserror::Error;
use uuid::Uuid;

/// Errors that can occur across the approval lifecycle (capture, verify,
/// consume, report).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ApprovalError {
    /// No approval record exists for the node.
    #[error("No approval record for node {0}")]
    NotFound(Uuid),

    /// The approval was already consumed (single-use violated by a replay).
    #[error("Approval already consumed for node {0}")]
    AlreadyConsumed(Uuid),

    /// The approval lapsed before dispatch (TTL enforced at verification).
    #[error("Approval expired for node {0}")]
    Expired(Uuid),

    /// Pre-dispatch verification failed — the executing call no longer matches
    /// what was approved. **Non-retriable**: re-approval is the only recovery.
    #[error("Intent verification failed for node {node_id}: expected {expected}, got {actual}")]
    IntentMismatch {
        /// Node whose intent diverged from the approved digest.
        node_id: Uuid,
        /// The recorded digest at approval time.
        expected: String,
        /// The re-derived digest at dispatch time.
        actual: String,
    },

    /// The record is in a state the requested transition does not permit.
    #[error("Invalid approval state: {0}")]
    InvalidState(String),

    /// The effect-scope oracle (git diff) could not run.
    #[error("Scope verification unavailable: {0}")]
    ScopeVerificationUnavailable(String),

    /// An approver-supplied declared scope is not a safe repo-relative path,
    /// or strict mode was requested with no scope (#1190).
    #[error("Invalid declared scope: {0}")]
    InvalidScope(String),

    /// A persisted record's lifecycle fields do not authenticate under the
    /// run key — it was edited after it was written (#1237).
    #[error(
        "Approval record for step '{step_name}' (node {node_id}) failed integrity verification; re-approval required"
    )]
    RecordIntegrity {
        /// Node whose record failed.
        node_id: Uuid,
        /// Step name the record claims to authorize.
        step_name: String,
    },

    /// The store is older than a consumption already recorded outside it —
    /// a whole-file rollback (#1241).
    #[error(
        "Approval record for step '{step_name}' (node {node_id}) regressed against the consumption ledger; re-approval required"
    )]
    RecordRollback {
        /// Node whose record regressed.
        node_id: Uuid,
        /// Step name the record claims to authorize.
        step_name: String,
    },

    /// A ledger path is configured but the file is absent, while the store
    /// holds terminal records. Absence is not evidence that nothing was spent
    /// (#1264).
    #[error(
        "Consumption ledger '{ledger}' is absent, so {terminal_records} terminal record(s) cannot be \
         verified (first: step '{step_name}', node {node_id}); restore the ledger at that path, or \
         re-approve the affected steps"
    )]
    ConsumptionLedgerMissing {
        /// The configured ledger path that does not exist.
        ledger: String,
        /// How many terminal records could not be checked.
        terminal_records: usize,
        /// The first terminal record that could not be checked.
        node_id: Uuid,
        /// Its step name.
        step_name: String,
    },

    /// A second terminal write for the same `(node_id, nonce)` found its marker
    /// already recorded — a replay, not an idempotent update (#1261).
    #[error(
        "Approval for step '{step_name}' (node {node_id}) was already consumed according to \
         ledger '{ledger}'; a second consumption is refused"
    )]
    ConsumptionReplay {
        /// Node whose approval was already consumed.
        node_id: Uuid,
        /// Step name the record claims to authorize.
        step_name: String,
        /// The ledger holding the first marker.
        ledger: String,
    },

    /// The consumption ledger's authenticated structure does not verify —
    /// markers were removed or the file was truncated (#1244).
    #[error(
        "Approval consumption ledger integrity failure at '{ledger}' for store '{store}': {detail}"
    )]
    ConsumptionLedgerIntegrity {
        /// The approval store the ledger anchors.
        store: String,
        /// The ledger path.
        ledger: String,
        /// What failed, without key material.
        detail: String,
    },

    /// A terminal record has no marker in an existing consumption ledger —
    /// the ledger and the store disagree (#1255).
    #[error(
        "Approval record for step '{step_name}' (node {node_id}) is terminal but has no marker in \
         consumption ledger '{ledger}'; run `rigorix approve ledger migrate` to compare and adopt it, \
         or `rigorix approve ledger migrate --accept-missing` to adopt the store's terminal records"
    )]
    UncoveredTerminalRecord {
        /// Node whose terminal record has no marker.
        node_id: Uuid,
        /// Step name the record claims to authorize.
        step_name: String,
        /// The ledger that should remember it.
        ledger: String,
    },

    /// Migration would seal a ledger that does not cover every terminal record
    /// the authenticated store holds (#1249).
    #[error(
        "consumption ledger migration at '{ledger}' for store '{store}' refuses to seal: \
         {missing_count} terminal record(s) have no marker ({missing}); rerun with \
         `--accept-missing` to adopt them into the ledger explicitly"
    )]
    MigrationIncomplete {
        /// The approval store.
        store: String,
        /// The ledger path.
        ledger: String,
        /// How many terminal records lack a marker.
        missing_count: usize,
        /// The records, as `node (step)`.
        missing: String,
    },

    /// Unexpected internal failure.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl ApprovalError {
    /// Whether the failed operation should be retried with backoff.
    ///
    /// # Recovery (canonical `.pi/architecture/modules/approval.md#error-handling`)
    /// - `IntentMismatch` is **not retriable by design** — auto-retry against a
    ///   mutated intent is a replay loop; the run halts and re-approval is the
    ///   only recovery
    /// - `NotFound` / `AlreadyConsumed` / `Expired`: not retriable — re-approval
    ///   required
    /// - `ScopeVerificationUnavailable`: retriable — retry the oracle; skip
    ///   with an explicit marker if persistently unavailable
    /// - `InvalidState` / `Internal`: not retriable — treat as fatal
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            ApprovalError::ScopeVerificationUnavailable(_) | ApprovalError::Internal(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_messages_are_frozen() {
        let err = ApprovalError::NotFound(Uuid::nil());
        assert_eq!(
            err.to_string(),
            "No approval record for node 00000000-0000-0000-0000-000000000000"
        );

        let err = ApprovalError::IntentMismatch {
            node_id: Uuid::nil(),
            expected: "abc".into(),
            actual: "def".into(),
        };
        assert_eq!(
            err.to_string(),
            "Intent verification failed for node 00000000-0000-0000-0000-000000000000: expected abc, got def"
        );

        let err = ApprovalError::Internal("boom".into());
        assert!(err.to_string().contains("boom"));
    }

    #[test]
    fn is_retriable_classification_is_frozen() {
        assert!(!ApprovalError::NotFound(Uuid::nil()).is_retriable());
        assert!(!ApprovalError::AlreadyConsumed(Uuid::nil()).is_retriable());
        assert!(!ApprovalError::Expired(Uuid::nil()).is_retriable());
        assert!(!ApprovalError::InvalidState("x".into()).is_retriable());
        assert!(
            !ApprovalError::RecordIntegrity {
                node_id: Uuid::nil(),
                step_name: "s".into()
            }
            .is_retriable()
        );
        assert!(
            !ApprovalError::RecordRollback {
                node_id: Uuid::nil(),
                step_name: "s".into()
            }
            .is_retriable()
        );
        assert!(
            !ApprovalError::ConsumptionLedgerIntegrity {
                store: "store".into(),
                ledger: "ledger".into(),
                detail: "missing marker".into(),
            }
            .is_retriable()
        );
        assert!(
            !ApprovalError::MigrationIncomplete {
                store: "store".into(),
                ledger: "ledger".into(),
                missing_count: 1,
                missing: "node (step)".into(),
            }
            .is_retriable()
        );
        // IntentMismatch is non-retriable by design — re-approval is the only recovery.
        assert!(
            !ApprovalError::IntentMismatch {
                node_id: Uuid::nil(),
                expected: "e".into(),
                actual: "a".into(),
            }
            .is_retriable()
        );
        assert!(ApprovalError::ScopeVerificationUnavailable("git".into()).is_retriable());
        assert!(ApprovalError::Internal("x".into()).is_retriable());
    }
}
