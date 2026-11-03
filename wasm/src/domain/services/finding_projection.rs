//! `EngineFindingsProjection` — the shipped `EvidenceProjection`.
//!
//! @canonical .pi/architecture/modules/portable-session.md#domain-services--application-services--infrastructure-services
//! Implements: `EvidenceProjection` (AC 9)
//! Issue: #1296
//!
//! One rule, and it is the whole component: **output order is input order**, and
//! `NotReached` stays `NotReached`. No sorting, no grouping, no de-duplication, no
//! collapsing of a step the gate never evaluated into a refusal — collapsing it is how a
//! record starts implying a decision nobody made.

use crate::domain::engine_seam::{EngineFinding, EngineFindingOutcome};
use crate::domain::outcome::{RefusalKind, StepDecision, StepOutcome};
use crate::domain::services::{EvidenceProjection, RefusalVocabulary};

/// Projects findings with business wording.
pub struct EngineFindingsProjection<V: RefusalVocabulary> {
    vocabulary: V,
}

impl<V: RefusalVocabulary> EngineFindingsProjection<V> {
    /// Construct with the vocabulary used for reasons.
    pub const fn new(vocabulary: V) -> Self {
        Self { vocabulary }
    }
}

impl<V: RefusalVocabulary> EvidenceProjection for EngineFindingsProjection<V> {
    fn project(&self, findings: &[EngineFinding]) -> Vec<StepOutcome> {
        findings
            .iter()
            .map(|finding| match finding.outcome {
                EngineFindingOutcome::Passed => StepOutcome {
                    gate_id: finding.gate_id.clone(),
                    decision: StepDecision::Allowed,
                    kind: None,
                    reason: None,
                    evidence: finding.evidence.clone(),
                },
                EngineFindingOutcome::Refused | EngineFindingOutcome::Error => {
                    let kind = refusal_kind(finding);
                    StepOutcome {
                        gate_id: finding.gate_id.clone(),
                        decision: StepDecision::Refused,
                        kind: Some(kind),
                        reason: Some(self.vocabulary.reason(kind).to_string()),
                        evidence: finding.evidence.clone(),
                    }
                }
                // The gate never asked the question; say so rather than pretending it
                // answered no.
                EngineFindingOutcome::NotReached => StepOutcome {
                    gate_id: finding.gate_id.clone(),
                    decision: StepDecision::NotReached,
                    kind: None,
                    reason: None,
                    evidence: finding.evidence.clone(),
                },
            })
            .collect()
    }
}

/// Map the engine's refusal label onto the crate's closed vocabulary.
///
/// An unrecognised label maps to `PreconditionError` — the neutral "a check could not be
/// completed" — rather than inventing a kind or falling through to an allow. The engine's
/// own label is preserved in the evidence payload, so nothing is lost.
fn refusal_kind(finding: &EngineFinding) -> RefusalKind {
    match finding.refusal.as_deref() {
        Some("precondition_failed") => RefusalKind::PreconditionFailed,
        Some("precondition_timeout") => RefusalKind::PreconditionTimeout,
        Some("requirement_unsatisfied") => RefusalKind::RequirementUnsatisfied,
        Some("companion_step_missing") => RefusalKind::CompanionStepMissing,
        Some("sequence_violation") => RefusalKind::SequenceViolation,
        Some("history_integrity_violated") => RefusalKind::HistoryIntegrityViolated,
        Some("approval_unbound") => RefusalKind::ApprovalUnbound,
        Some("approval_expired") => RefusalKind::ApprovalExpired,
        Some("identity_unverified") | Some("unverified") => RefusalKind::IdentityUnverified,
        Some("port_unavailable") => RefusalKind::PortUnavailable,
        Some(_) | None => RefusalKind::PreconditionError,
    }
}
