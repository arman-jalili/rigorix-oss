//! `RefusalKind` — the engine's refusal vocabulary, carried without rewording.
//!
//! @canonical .pi/architecture/modules/portable-session.md#domain-services--application-services--infrastructure-services
//! @canonical .pi/architecture/decisions/ADR-007-compliance-engine.md
//! Implements: Contract Freeze — RefusalKind
//! Issue: #1294 (contract freeze)
//!
//! Each kind is a fact the engine can produce. [`RefusalVocabulary`] maps every one of
//! them to business wording, and the match is exhaustive: adding a kind without wording
//! is a compile error (criterion 8), never a generic string.
//!
//! # Contract (Frozen)
//! - One variant per engine outcome the gate can report
//! - `IdentityUnverified` is a distinct kind precisely so that "we could not check the
//!   identity" never reads as "the identity is wrong" (FR-004)

use serde::{Deserialize, Serialize};

/// Why a step was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalKind {
    /// A precondition check ran and returned a negative determination.
    PreconditionFailed,
    /// A precondition check could not run.
    PreconditionError,
    /// A precondition check exceeded its budget.
    PreconditionTimeout,
    /// An operator step requirement was not satisfied.
    RequirementUnsatisfied,
    /// A required companion step had not run.
    CompanionStepMissing,
    /// The step violated the scenario's sequence policy.
    SequenceViolation,
    /// The history the policy depends on did not verify.
    HistoryIntegrityViolated,
    /// An approval claim was supplied that does not bind to this intent.
    ApprovalUnbound,
    /// An approval claim binds, but had expired.
    ApprovalExpired,
    /// Identity could not be checked. **Never** rendered as "unauthenticated".
    IdentityUnverified,
    /// A capability port could not answer (invariant 2).
    PortUnavailable,
    /// The scenario's declared format version is not supported.
    ScenarioUnsupported,
    /// The session was already sealed; nothing may be appended.
    SessionSealed,
}
