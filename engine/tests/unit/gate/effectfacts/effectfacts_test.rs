//! CONSEQ-1 · a repeated effect is refused by identity, not by name.
//!
//! Issue: #1455 (epic #1443)
//!
//! # Why identity is the assertion
//!
//! "This effect already committed" is not actionable. The refusal has to name *which*
//! commitment — the provider's record id and the instant it happened — because that is what an
//! operator takes to the ledger to reconcile. A seed that only asserted the refusal would pass
//! for a port that refused every effect, including the first one.

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use rigorix_engine::audit::application::dto::PinnedInputs;
use rigorix_engine::gate::application::{GatePorts, GateRequest, evaluate_gate};
use rigorix_engine::gate::domain::{
    CommittedEffect, EffectFacts, GateDecision, GateId, GateVerdict, GatedStep,
};

const EXEC: Uuid = Uuid::from_u128(0x018f_5b8e_0000_7000_8000_0000_0000_0002);

fn pins() -> PinnedInputs {
    PinnedInputs::none()
        .with_execution_id(EXEC)
        .with_timestamp(Utc.with_ymd_and_hms(2026, 11, 4, 14, 5, 0).unwrap())
}

fn step() -> GatedStep {
    GatedStep::new(
        "issue_refund",
        "payments.refund",
        serde_json::json!({"invoice": "INV-88120"}),
    )
}

fn committed() -> CommittedEffect {
    CommittedEffect::new(
        "QR-2041",
        "finance",
        Utc.with_ymd_and_hms(2026, 11, 4, 14, 2, 0).unwrap(),
    )
}

struct Facts(Option<CommittedEffect>);

impl EffectFacts for Facts {
    fn committed(&self) -> Option<CommittedEffect> {
        self.0.clone()
    }
}

async fn evaluate(ports: GatePorts) -> GateDecision {
    evaluate_gate(GateRequest {
        step: &step(),
        proposed_action: "refund invoice INV-88120",
        identity_attested: true,
        pins: pins(),
        ports,
        run_context: None,
    })
    .await
    .expect("the gate answers")
}

fn effect_ports(facts: Facts) -> GatePorts {
    GatePorts {
        effect: Some(std::sync::Arc::new(facts)),
        ..GatePorts::none()
    }
}

/// S3 — a declared prior commitment refuses, and names the record **and** its instant.
#[tokio::test]
async fn a_repeated_effect_is_refused_naming_its_record_and_instant() {
    let decision = evaluate(effect_ports(Facts(Some(committed())))).await;

    let (gate, reason) = match &decision.verdict {
        GateVerdict::Refuse { gate, reason, .. } => (gate, reason),
        other => panic!("expected a refusal, got {other:?}"),
    };
    assert_eq!(*gate, GateId::Effects);
    assert!(reason.contains("QR-2041"), "the record id is named: {reason}");
    assert!(
        reason.contains("2026-11-04T14:02:00Z"),
        "the instant it happened is named, not the instant we looked: {reason}"
    );

    let finding = decision
        .findings
        .iter()
        .find(|f| f.gate_id == GateId::Effects)
        .expect("a refusal is never an unfindable verdict");
    let evidence = finding.evidence.as_ref().expect("the identity survives as evidence");
    assert_eq!(evidence["record_id"], "QR-2041");
    assert_eq!(evidence["instant"], "2026-11-04T14:02:00Z");
}

/// The first commitment of an effect is not a duplicate — a port that refused whenever it was
/// consulted would pass the seed above.
#[tokio::test]
async fn an_effect_that_has_not_committed_is_admitted() {
    let decision = evaluate(effect_ports(Facts(None))).await;
    assert!(
        decision.verdict.permits_dispatch(),
        "a first commitment is the point of the capability: {:?}",
        decision.verdict
    );
    assert!(decision.findings.is_empty());
}

/// Absence is free: no declared effect obligation, no finding.
#[tokio::test]
async fn no_declared_effect_produces_no_finding() {
    let decision = evaluate(GatePorts::none()).await;
    assert!(decision.verdict.permits_dispatch());
    assert!(decision.findings.iter().all(|f| f.gate_id != GateId::Effects));
}

/// The digest binds the identity: two commitments that differ only in the instant are two
/// different refusals, and a digest that ignored the instant would make them interchangeable.
#[tokio::test]
async fn the_decision_digest_distinguishes_commitments_by_instant() {
    let earlier = evaluate(effect_ports(Facts(Some(committed())))).await;
    let later = evaluate(effect_ports(Facts(Some(CommittedEffect::new(
        "QR-2041",
        "finance",
        Utc.with_ymd_and_hms(2026, 11, 4, 14, 3, 0).unwrap(),
    )))))
    .await;

    assert_ne!(
        earlier.envelope.decision_digest, later.envelope.decision_digest,
        "the same record at a different instant is a different refusal"
    );
}
