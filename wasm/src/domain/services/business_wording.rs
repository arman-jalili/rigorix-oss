//! `BusinessWording` — the shipped `RefusalVocabulary`.
//!
//! @canonical .pi/architecture/modules/portable-session.md#domain-services--application-services--infrastructure-services
//! @canonical .pi/architecture/decisions/ADR-007-compliance-engine.md
//! Implements: `RefusalVocabulary` (AC 8); refuses to accuse (AC 5, 12)
//! Issue: #1296
//!
//! The match is exhaustive on purpose: adding a `RefusalKind` without wording is a
//! compile error, so a new engine outcome cannot reach a host as a generic string.
//!
//! Every string is static and value-free. `IdentityUnverified` says *the identity could
//! not be checked* — the words `unauthenticated`, `forged` and `tampered` are not
//! available here, because "we could not ask" is not an accusation (FR-004).
//!
//! The crate also never says `not_verified` or `unverifiable`: that is the host's
//! three-valued vocabulary, and using it here would blur who decided (ADR-013).

use crate::domain::outcome::RefusalKind;
use crate::domain::services::RefusalVocabulary;
use crate::domain::value::{PortFailure, PortId};

/// The shipped business wording.
#[derive(Debug, Default, Clone, Copy)]
pub struct BusinessWording;

impl BusinessWording {
    /// Construct the vocabulary.
    pub const fn new() -> Self {
        Self
    }
}

impl RefusalVocabulary for BusinessWording {
    fn reason(&self, kind: RefusalKind) -> &'static str {
        match kind {
            RefusalKind::PreconditionFailed => "the required check did not pass",
            RefusalKind::PreconditionError => "the required check could not be completed",
            RefusalKind::PreconditionTimeout => "the required check did not finish in time",
            RefusalKind::RequirementUnsatisfied => "a required earlier step has not completed",
            RefusalKind::CompanionStepMissing => "a required companion step had not run",
            RefusalKind::SequenceViolation => "this step is out of order for this scenario",
            RefusalKind::HistoryIntegrityViolated => "the recorded history did not hold together",
            RefusalKind::ApprovalUnbound => "the approval does not cover this action",
            RefusalKind::ApprovalExpired => "the approval had expired",
            RefusalKind::IdentityUnverified => "the identity could not be checked",
            RefusalKind::PortUnavailable => "a required check was unavailable",
            RefusalKind::ScenarioUnsupported => "this scenario version is not supported",
            RefusalKind::SessionSealed => "this record is already sealed",
        }
    }

    fn port_reason(&self, failure: PortFailure) -> &'static str {
        match failure.port {
            PortId::Clock => "the reference time series was exhausted",
            PortId::IdSource => "the reference identifier series was exhausted",
            PortId::Randomness => "the reference randomness was exhausted",
            // Not "the identity is wrong". We never got to ask.
            PortId::KeySource => "the identity could not be checked",
            PortId::ProviderTransport => "a required check was unavailable",
            PortId::BusinessSink => "the intended write could not be recorded",
        }
    }
}
