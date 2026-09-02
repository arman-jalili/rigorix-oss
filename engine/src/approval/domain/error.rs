//! ApprovalError — typed error enum for all approval binding failure modes.
//!
//! @canonical .pi/architecture/modules/approval.md#error-handling
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
