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
use crate::domain::outcome::{RefusalDetail, RefusalKind, StepDecision, StepOutcome};
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
                    detail: None,
                },
                EngineFindingOutcome::Refused | EngineFindingOutcome::Error => {
                    // One read of the payload produces both the kind and the operands, so a
                    // refusal cannot name an obligation in `kind` and a different one in
                    // `detail`. A missing operand is a port-failure refusal, never a zero:
                    // `overage: 0` reads as a pass (#1462).
                    let detail = refusal_detail(finding);
                    // A capacity or effect finding whose operands did not survive is **not**
                    // that obligation's refusal any more: the crate cannot say what the
                    // numbers were, so it says a check could not answer. Reporting
                    // `capacity_exceeded` with no operands, or with a zeroed overage, would be
                    // inventing the very facts the refusal is about.
                    let kind = if is_operand_gate(&finding.gate_id) && detail.is_none() {
                        RefusalKind::PortUnavailable
                    } else {
                        refusal_kind(finding)
                    };
                    StepOutcome {
                        gate_id: finding.gate_id.clone(),
                        decision: StepDecision::Refused,
                        kind: Some(kind),
                        reason: Some(self.vocabulary.reason(kind).to_string()),
                        evidence: finding.evidence.clone(),
                        detail,
                    }
                }
                // A promotion is a decision that has not been made yet: not `Allowed`
                // (which proceeds) and not `Refused` (which is final). It carries no
                // refusal kind for the same reason — nothing was refused.
                EngineFindingOutcome::Promoted => StepOutcome {
                    gate_id: finding.gate_id.clone(),
                    decision: StepDecision::Promoted,
                    kind: None,
                    reason: None,
                    evidence: finding.evidence.clone(),
                    detail: None,
                },
                // The gate never asked the question; say so rather than pretending it
                // answered no.
                EngineFindingOutcome::NotReached => StepOutcome {
                    gate_id: finding.gate_id.clone(),
                    decision: StepDecision::NotReached,
                    kind: None,
                    reason: None,
                    evidence: finding.evidence.clone(),
                    detail: None,
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
    // The two capabilities added by #1455 answer by *which gate refused*, not by a label in
    // the reason string: `capacity` and `effects` are gate identities the engine owns, so
    // matching on them cannot be fooled by wording.
    match finding.gate_id.as_str() {
        "capacity" => return RefusalKind::CapacityExceeded,
        "effects" => return RefusalKind::DuplicateEffect,
        _ => {}
    }
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

/// Which gates carry operands on their findings (#1462).
fn is_operand_gate(gate_id: &str) -> bool {
    matches!(gate_id, "capacity" | "effects")
}

/// The typed operands of a refusal, read from the engine's own payload.
///
/// `EngineFinding.evidence` is an [`EvidenceRef`](crate::domain::outcome::EvidenceRef) — a
/// pointer to bytes, by design — but `opaque` holds the finding the engine produced, and that
/// finding carries the structured `evidence` object it built when it refused. Projecting it is
/// a typed read of a payload the crate already has, not a new allowance.
///
/// Every operand is required: a missing one yields `None`, and the caller turns that into a
/// port-failure refusal. A default here would invent a number — `overage: 0` is a *pass*.
fn refusal_detail(finding: &EngineFinding) -> Option<RefusalDetail> {
    let evidence = finding.opaque.as_ref()?.get("evidence")?;
    match finding.gate_id.as_str() {
        "capacity" => Some(RefusalDetail::Capacity {
            requested: evidence.get("requested")?.as_u64()?,
            claimed: evidence.get("claimed")?.as_u64()?,
            hard_limit: evidence.get("hard_limit")?.as_u64()?,
            overage: evidence.get("overage")?.as_u64()?,
        }),
        "effects" => Some(RefusalDetail::DuplicateEffect {
            record_id: evidence.get("record_id")?.as_str()?.to_string(),
            instant: evidence.get("instant")?.as_str()?.to_string(),
        }),
        _ => None,
    }
}
