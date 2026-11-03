//! Contract: every refusal kind has business wording — exhaustively (AC 8).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::domain::outcome::RefusalKind;
use rigorix_gate_wasm::domain::services::RefusalVocabulary;
use rigorix_gate_wasm::domain::value::{PortFailure, PortFailureKind, PortId};

/// A minimal implementation whose only job is to prove the match is exhaustive.
///
/// If a `RefusalKind` is added without wording, this file stops compiling — which is the
/// guard criterion 8 asks for. A `_ =>` arm would silently defeat it, so there is none.
struct Wording;

impl RefusalVocabulary for Wording {
    fn reason(&self, kind: RefusalKind) -> &'static str {
        match kind {
            RefusalKind::PreconditionFailed => "the required check did not pass",
            RefusalKind::PreconditionError => "the required check could not be completed",
            RefusalKind::PreconditionTimeout => "the required check did not finish in time",
            RefusalKind::RequirementUnsatisfied => "a required step outcome is missing",
            RefusalKind::CompanionStepMissing => "a required companion step had not run",
            RefusalKind::SequenceViolation => "the step is out of order for this scenario",
            RefusalKind::HistoryIntegrityViolated => "the recorded history did not verify",
            RefusalKind::ApprovalUnbound => "the approval does not cover this action",
            RefusalKind::ApprovalExpired => "the approval had expired",
            RefusalKind::IdentityUnverified => "the identity could not be checked",
            RefusalKind::PortUnavailable => "a required check was unavailable",
            RefusalKind::ScenarioUnsupported => "the scenario version is not supported",
            RefusalKind::SessionSealed => "this record is already sealed",
        }
    }

    fn port_reason(&self, failure: PortFailure) -> &'static str {
        match (failure.port, failure.kind) {
            (PortId::Clock, _) => "the reference time series was exhausted",
            (PortId::IdSource, _) => "the reference identifier series was exhausted",
            (PortId::Randomness, _) => "the reference randomness was exhausted",
            (PortId::KeySource, _) => "the key that signs the record could not be read",
            (PortId::ProviderTransport, _) => "a required check was unavailable",
            (PortId::BusinessSink, _) => "the intended write could not be recorded",
        }
    }
}

#[test]
fn every_kind_has_wording() {
    let vocabulary = Wording;
    for kind in [
        RefusalKind::PreconditionFailed,
        RefusalKind::PreconditionError,
        RefusalKind::PreconditionTimeout,
        RefusalKind::RequirementUnsatisfied,
        RefusalKind::CompanionStepMissing,
        RefusalKind::SequenceViolation,
        RefusalKind::HistoryIntegrityViolated,
        RefusalKind::ApprovalUnbound,
        RefusalKind::ApprovalExpired,
        RefusalKind::IdentityUnverified,
        RefusalKind::PortUnavailable,
        RefusalKind::ScenarioUnsupported,
        RefusalKind::SessionSealed,
    ] {
        let reason = vocabulary.reason(kind);
        assert!(!reason.is_empty(), "empty wording for {kind:?}");
        // Value-free and path-free: wording is a sentence, not a dump (NFR-003).
        assert!(!reason.contains('/'), "path in wording for {kind:?}");
        assert!(!reason.contains("sha256"), "digest in wording for {kind:?}");
    }
}

#[test]
fn the_forbidden_words_are_not_in_the_vocabulary() {
    // `unauthenticated` accuses; `not_verified`/`unverifiable` are the host's vocabulary
    // and never the producer's (criterion 5, 12).
    let vocabulary = Wording;
    let forbidden = ["unauthenticated", "not_verified", "unverifiable", "forged"];
    for kind in [
        RefusalKind::PreconditionFailed,
        RefusalKind::IdentityUnverified,
        RefusalKind::PortUnavailable,
    ] {
        let reason = vocabulary.reason(kind).to_lowercase();
        for word in forbidden {
            assert!(
                !reason.contains(word),
                "wording for {kind:?} contains {word}"
            );
        }
    }
}

#[test]
fn a_port_failure_always_has_wording_per_port() {
    let vocabulary = Wording;
    for port in [
        PortId::Clock,
        PortId::IdSource,
        PortId::Randomness,
        PortId::KeySource,
        PortId::ProviderTransport,
        PortId::BusinessSink,
    ] {
        for kind in [
            PortFailureKind::Missing,
            PortFailureKind::Exhausted,
            PortFailureKind::Malformed,
            PortFailureKind::Failed,
        ] {
            let reason = vocabulary.port_reason(PortFailure::new(port, kind));
            assert!(!reason.is_empty(), "empty wording for {port:?}/{kind:?}");
        }
    }
}

// ---------------------------------------------------------------------------
// Shipped wording (issue #1296): criterion 8
// ---------------------------------------------------------------------------

#[test]
fn the_shipped_wording_covers_every_kind_and_accuses_nobody() {
    use rigorix_gate_wasm::domain::services::BusinessWording;

    let shipped = BusinessWording::new();
    let kinds = [
        RefusalKind::PreconditionFailed,
        RefusalKind::PreconditionError,
        RefusalKind::PreconditionTimeout,
        RefusalKind::RequirementUnsatisfied,
        RefusalKind::CompanionStepMissing,
        RefusalKind::SequenceViolation,
        RefusalKind::HistoryIntegrityViolated,
        RefusalKind::ApprovalUnbound,
        RefusalKind::ApprovalExpired,
        RefusalKind::IdentityUnverified,
        RefusalKind::PortUnavailable,
        RefusalKind::ScenarioUnsupported,
        RefusalKind::SessionSealed,
    ];
    let mut seen = std::collections::HashSet::new();
    for kind in kinds {
        let reason = shipped.reason(kind);
        assert!(seen.insert(reason), "two kinds share wording: {kind:?}");
        let lower = reason.to_lowercase();
        for forbidden in [
            "unauthenticated",
            "forged",
            "tampered",
            "not_verified",
            "unverifiable",
        ] {
            assert!(!lower.contains(forbidden), "{kind:?} says {forbidden}");
        }
    }
}

#[test]
fn an_identity_port_failure_says_it_could_not_check() {
    use rigorix_gate_wasm::domain::services::{BusinessWording, RefusalVocabulary};

    // The truthful sentence: we never got to ask. Not "the identity is wrong".
    let reason = BusinessWording::new().port_reason(PortFailure::new(
        PortId::KeySource,
        PortFailureKind::Missing,
    ));
    assert_eq!(reason, "the identity could not be checked");
}
