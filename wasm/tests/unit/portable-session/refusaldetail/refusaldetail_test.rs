//! CONSEQ-4 · the operands the crate already holds reach the wire (#1462).
//!
//! # What this file is for
//!
//! `EngineFinding.evidence` is an `EvidenceRef` — a digest and a kind, a pointer to bytes, by
//! design. That freeze is correct and stays. But the crate already holds the engine's finding
//! in `EngineFinding.opaque`, and that finding carries the operands the engine computed. So a
//! refusal that could only say "capacity exceeded" was a projection gap, not a contract limit.
//!
//! The seeds below assert the numbers, and — the one that matters — that a missing operand is
//! **not** a zero. `overage: 0` reads as "inside the cap", which is the opposite of what the
//! refusal is about, so an incomplete payload has to become "a check could not answer".

use rigorix_gate_wasm::domain::engine_seam::{EngineFinding, EngineFindingOutcome};
use rigorix_gate_wasm::domain::outcome::{RefusalDetail, RefusalKind, StepDecision};
use rigorix_gate_wasm::domain::services::{
    BusinessWording, EngineFindingsProjection, EvidenceProjection,
};

fn projection() -> EngineFindingsProjection<BusinessWording> {
    EngineFindingsProjection::new(BusinessWording::new())
}

/// The `GateFinding` the engine actually produces for a capacity refusal (#1455), serialised
/// the way `engine_gate.rs` stores it: `serde_json::to_value(finding)`.
fn capacity_finding(evidence: serde_json::Value) -> EngineFinding {
    EngineFinding {
        gate_id: "capacity".into(),
        outcome: EngineFindingOutcome::Refused,
        refusal: Some("capacity refused: requested 18420".into()),
        evidence: None,
        opaque: Some(serde_json::json!({
            "gate_id": "capacity",
            "step": "payment_execute",
            "outcome": "refused",
            "rule_id": "capacity.eur.outflow.daily",
            "detail": "capacity refused: requested 18420, claimed 1427420, cap 1420000, overage 7420",
            "evidence": evidence,
        })),
    }
}

fn complete_capacity_evidence() -> serde_json::Value {
    serde_json::json!({
        "resource": "eur.outflow.daily",
        "requested": 18_420u64,
        "committed": 1_409_000u64,
        "outstanding": 0u64,
        "claimed": 1_427_420u64,
        "hard_limit": 1_420_000u64,
        "available": 11_000u64,
        "overage": 7_420u64
    })
}

/// Criterion 1 — the four numbers a demo prints, and nothing to parse.
#[test]
fn a_capacity_refusal_carries_its_four_operands() {
    let outcome = &projection().project(&[capacity_finding(complete_capacity_evidence())])[0];

    assert_eq!(outcome.decision, StepDecision::Refused);
    assert_eq!(outcome.kind, Some(RefusalKind::CapacityExceeded));
    assert_eq!(
        outcome.detail,
        Some(RefusalDetail::Capacity {
            requested: 18_420,
            claimed: 1_427_420,
            hard_limit: 1_420_000,
            overage: 7_420,
        })
    );
}

/// Criterion 2 — the identity of the commitment that blocked it, not the moment of the attempt.
#[test]
fn a_duplicate_effect_refusal_carries_the_record_and_its_instant() {
    let finding = EngineFinding {
        gate_id: "effects".into(),
        outcome: EngineFindingOutcome::Refused,
        refusal: Some("effect refused: already committed as QR-2041".into()),
        evidence: None,
        opaque: Some(serde_json::json!({
            "gate_id": "effects",
            "step": "issue_refund",
            "outcome": "refused",
            "rule_id": "effect.duplicate",
            "detail": "effect refused: already committed as QR-2041 at 2026-11-04T14:02:00Z",
            "evidence": { "record_id": "QR-2041", "actor": "finance", "instant": "2026-11-04T14:02:00Z" }
        })),
    };
    let outcome = &projection().project(&[finding])[0];

    assert_eq!(outcome.kind, Some(RefusalKind::DuplicateEffect));
    assert_eq!(
        outcome.detail,
        Some(RefusalDetail::DuplicateEffect {
            record_id: "QR-2041".into(),
            instant: "2026-11-04T14:02:00Z".into(),
        })
    );
}

/// Criterion 3 — the wording is untouched and still carries no host data. The operands moved
/// to a *different field*; they did not move into the sentence.
#[test]
fn the_wording_still_quotes_no_operand() {
    let outcome = &projection().project(&[capacity_finding(complete_capacity_evidence())])[0];
    let reason = outcome.reason.as_deref().expect("a refusal has wording");

    for number in ["1427420", "1 427 420", "1420000", "7420", "11000"] {
        assert!(
            !reason.contains(number),
            "the wording must stay value-free (NFR-003): {reason}"
        );
    }
}

/// Criterion 5 — **the seed that matters.** A missing operand must not become a zero, and must
/// not keep claiming the obligation answered: `overage: 0` is a pass, and a `capacity_exceeded`
/// with no operands is indistinguishable from a form validator again.
#[test]
fn a_missing_operand_refuses_as_a_port_failure_never_as_zero() {
    let mut incomplete = complete_capacity_evidence();
    incomplete
        .as_object_mut()
        .expect("object")
        .remove("overage");

    let outcome = &projection().project(&[capacity_finding(incomplete)])[0];

    assert!(
        outcome.detail.is_none(),
        "no operands: {:?} would be an invented number",
        outcome.detail
    );
    assert_eq!(
        outcome.kind,
        Some(RefusalKind::PortUnavailable),
        "a refusal whose operands did not survive is a check that could not answer"
    );
    assert!(!outcome.reason.as_deref().unwrap_or_default().contains('0'));
}

/// The same for an effect: a record id without its instant is not the identity.
#[test]
fn a_duplicate_effect_without_its_instant_carries_no_detail() {
    let finding = EngineFinding {
        gate_id: "effects".into(),
        outcome: EngineFindingOutcome::Refused,
        refusal: Some("effect refused".into()),
        evidence: None,
        opaque: Some(serde_json::json!({
            "gate_id": "effects",
            "evidence": { "record_id": "QR-2041" }
        })),
    };
    let outcome = &projection().project(&[finding])[0];

    assert!(outcome.detail.is_none());
    assert_eq!(outcome.kind, Some(RefusalKind::PortUnavailable));
}

/// A gate outside the two that carry operands is unaffected: no detail, and its own kind.
#[test]
fn a_precondition_refusal_is_unchanged_by_the_new_field() {
    let finding = EngineFinding {
        gate_id: "preconditions".into(),
        outcome: EngineFindingOutcome::Refused,
        refusal: Some("precondition_failed".into()),
        evidence: None,
        opaque: None,
    };
    let outcome = &projection().project(&[finding])[0];

    assert!(outcome.detail.is_none());
    assert_eq!(outcome.kind, Some(RefusalKind::PreconditionFailed));
}

// ── the wire ───────────────────────────────────────────────────────────────────────
//
// The layer the demo reads. Without these, the mapping is dead code and the interface
// coverage drops — which is how this file came to exist twice over (#1462).

use rigorix_gate_wasm::interfaces::dto::StepOutcomeDto;

#[test]
fn a_capacity_detail_reaches_the_wire_as_numbers() {
    let outcome =
        projection().project(&[capacity_finding(complete_capacity_evidence())])[0].clone();
    let json = serde_json::to_value(StepOutcomeDto::from(&outcome)).expect("serialises");

    assert_eq!(json["detail"]["kind"], "capacity");
    assert_eq!(json["detail"]["requested"], 18_420);
    assert_eq!(json["detail"]["claimed"], 1_427_420);
    assert_eq!(json["detail"]["hard_limit"], 1_420_000);
    assert_eq!(json["detail"]["overage"], 7_420);
}

#[test]
fn a_duplicate_effect_detail_reaches_the_wire_with_its_identity() {
    let finding = EngineFinding {
        gate_id: "effects".into(),
        outcome: EngineFindingOutcome::Refused,
        refusal: Some("effect refused".into()),
        evidence: None,
        opaque: Some(serde_json::json!({
            "evidence": { "record_id": "QR-2041", "instant": "2026-11-04T14:02:00Z" }
        })),
    };
    let outcome = projection().project(&[finding])[0].clone();
    let json = serde_json::to_value(StepOutcomeDto::from(&outcome)).expect("serialises");

    assert_eq!(json["detail"]["kind"], "duplicate_effect");
    assert_eq!(json["detail"]["record_id"], "QR-2041");
    assert_eq!(json["detail"]["instant"], "2026-11-04T14:02:00Z");
}

#[test]
fn a_step_without_operands_omits_the_field_entirely() {
    // Absent, not `null` and not `{}`: a host that checks for the field must not have to
    // distinguish "no operands" from "operands that render as nothing".
    let finding = EngineFinding {
        gate_id: "preconditions".into(),
        outcome: EngineFindingOutcome::Refused,
        refusal: Some("precondition_failed".into()),
        evidence: None,
        opaque: None,
    };
    let outcome = projection().project(&[finding])[0].clone();
    let json = serde_json::to_value(StepOutcomeDto::from(&outcome)).expect("serialises");
    assert!(
        json.get("detail").is_none(),
        "an absent detail is omitted: {json}"
    );
}
