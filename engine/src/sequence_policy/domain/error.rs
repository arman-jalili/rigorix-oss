//! SequencePolicyError — typed error enum for all sequence-policy failure
//! modes.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#sequencepolicyerror
//! Implements: Contract Freeze — SequencePolicyError enum + `is_retriable()`
//! Issue: #838 (sequence-policy epic — contract freeze); closed in
//!   ISSUE-SEQUENCE-POLICY-3
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `SequencePolicyError` is the single error type for this module
//! - `InvalidConfig` / `RuleExceedsCaps`: **not retriable** — operator fixes
//!   the rule file; config parse/load failures **fail closed at plan time**
//!   (no plan runs under an unparseable rule set)
//! - `NotFound`: not retriable — a rule was removed mid-run; match decisions
//!   already made stand
//! - `InvalidState`: not retriable — treat as fatal for the operation
//! - `Internal`: the only retriable variant
//! - A missing optional config file is **not** an error (`Ok(None)` — no
//!   rules → no matches → status quo, fail-open)
//!
//! # Recovery
//! - `InvalidConfig` / `RuleExceedsCaps`: not retriable — operator action
//! - `NotFound`: not retriable — rule removed mid-run
//! - `InvalidState`: not retriable — unexpected state
//! - `Internal`: retriable — retry with backoff

use thiserror::Error;

/// Errors that can occur across the sequence-policy lifecycle (config load,
/// plan evaluation, prefix evaluation).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SequencePolicyError {
    /// The rule config file is unparseable or structurally invalid. **Fail
    /// closed** at plan time — no plan runs under an unparseable rule set.
    #[error("Rule config invalid: {0}")]
    InvalidConfig(String),

    /// A rule exceeds the safety caps (max rules, max predicates per rule,
    /// max window, max regex predicates). Operator fixes the rule file.
    #[error("Rule '{rule}' exceeds safety caps: {detail}")]
    RuleExceedsCaps {
        /// Rule id that exceeded the caps.
        rule: String,
        /// Which cap was exceeded and by how much.
        detail: String,
    },

    /// A referenced rule does not exist (removed mid-run; decisions already
    /// made stand).
    #[error("Rule not found: {0}")]
    NotFound(String),

    /// The evaluation state does not permit the requested operation.
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Unexpected internal failure (storage, IO, …). Retriable.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl SequencePolicyError {
    /// Whether the failed operation should be retried with backoff.
    ///
    /// # Recovery (canonical `.pi/architecture/modules/sequence-policy.md#error-handling`)
    /// - `InvalidConfig` / `RuleExceedsCaps`: not retriable — operator fixes
    ///   the rule file (fail-closed at plan time)
    /// - `NotFound`: not retriable — rule removed mid-run
    /// - `InvalidState`: not retriable — treat as fatal
    /// - `Internal`: retriable
    pub fn is_retriable(&self) -> bool {
        matches!(self, SequencePolicyError::Internal(_))
    }
}
