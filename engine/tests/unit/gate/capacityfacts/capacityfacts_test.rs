//! CONSEQ-1 · the gate decides capacity from declared facts, and owns the arithmetic.
//!
//! Issue: #1455 (epic #1443)
//!
//! # What these tests are for
//!
//! The temptation this child had to refuse is a port that answers `allowed`/`refused`. That
//! would compile, read well, and put the decision in the adapter — two runtimes could then
//! disagree about the same declared cap. So the seeds below do not test "a refusal happened";
//! they test the **operands**. A port that decided would have to re-derive `1 427 420` and
//! `7 420` itself, and S4 (a refusal consumes nothing) would be unimplementable without state.

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use rigorix_engine::audit::application::dto::PinnedInputs;
use rigorix_engine::capacity_ledger::domain::{Exposure, ResourceId, Window};
use rigorix_engine::gate::application::{GatePorts, GateRequest, evaluate_gate};
use rigorix_engine::gate::domain::{CapacityFacts, GateDecision, GateId, GateVerdict, GatedStep};

const EXEC: Uuid = Uuid::from_u128(0x018f_5b8e_0000_7000_8000_0000_0000_0001);

fn pins() -> PinnedInputs {
    PinnedInputs::none()
        .with_execution_id(EXEC)
        .with_timestamp(Utc.with_ymd_and_hms(2026, 11, 3, 12, 0, 0).unwrap())
}

fn step() -> GatedStep {
    GatedStep::new(
        "transfer",
        "payments.transfer",
        serde_json::json!({"amount": 18_420}),
    )
}

/// The cap from the issue: 1 420 000 units, of which 1 409 000 are committed.
fn exposure(committed: u64) -> Exposure {
    Exposure::new(
        ResourceId::new("eur.outflow.daily"),
        1_420_000,
        committed,
        0,
        Window::bounded(86_400),
    )
}

/// Declares facts and nothing else — no predicate, no state, no arithmetic.
struct Facts {
    exposure: Option<Exposure>,
    requested: u64,
}

impl CapacityFacts for Facts {
    fn exposure(&self) -> Option<Exposure> {
        self.exposure.clone()
    }

    fn requested(&self) -> u64 {
        self.requested
    }
}

async fn evaluate(ports: GatePorts) -> GateDecision {
    evaluate_gate(GateRequest {
        step: &step(),
        proposed_action: "move 18420 units",
        identity_attested: true,
        pins: pins(),
        ports,
        run_context: None,
    })
    .await
    .expect("the gate answers")
}

fn capacity_ports(facts: Facts) -> GatePorts {
    GatePorts {
        capacity: Some(std::sync::Arc::new(facts)),
        ..GatePorts::none()
    }
}

fn refusal(decision: &GateDecision) -> (&GateId, &str) {
    match &decision.verdict {
        GateVerdict::Refuse { gate, reason, .. } => (gate, reason),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// S1 — cap 1 420 000, committed 1 409 000, request 18 420: refused, and the refusal carries
/// the three numbers a caller needs. Asserting the word "refused" alone would pass for a port
/// that refused everything.
#[tokio::test]
async fn a_declared_cap_refuses_and_names_the_operands() {
    let decision = evaluate(capacity_ports(Facts {
        exposure: Some(exposure(1_409_000)),
        requested: 18_420,
    }))
    .await;

    let (gate, reason) = refusal(&decision);
    assert_eq!(*gate, GateId::Capacity, "the capacity gate is the one that refused");
    assert!(
        reason.contains("18420"),
        "the requested amount is on the wire: {reason}"
    );
    assert!(
        reason.contains("1427420"),
        "claimed = committed + requested = 1 409 000 + 18 420: {reason}"
    );
    assert!(reason.contains("1420000"), "the cap is named: {reason}");
    assert!(reason.contains("7420"), "the overage is named: {reason}");

    let finding = decision
        .findings
        .iter()
        .find(|f| f.gate_id == GateId::Capacity)
        .expect("a refusal is never an unfindable verdict");
    assert_eq!(finding.outcome, rigorix_engine::gate::domain::GateOutcome::Refused);
    let evidence = finding.evidence.as_ref().expect("the operands survive as evidence");
    assert_eq!(evidence["overage"], 7_420);
    assert_eq!(evidence["available"], 11_000);
    assert_eq!(evidence["committed"], 1_409_000);
    assert_eq!(evidence["outstanding"], 0);
}

/// S2 — the boundary is **inclusive**, and it is asserted rather than left to a reader:
/// `1 401 580 + 18 420 == 1 420 000` exactly, and `Exposure::would_fit` says `available >= amount`.
#[tokio::test]
async fn a_consumption_that_lands_exactly_on_the_cap_is_admitted() {
    let decision = evaluate(capacity_ports(Facts {
        exposure: Some(exposure(1_401_580)),
        requested: 18_420,
    }))
    .await;

    assert!(
        decision.verdict.permits_dispatch(),
        "exactly at the cap is inside it: {:?}",
        decision.verdict
    );
    assert!(
        decision.findings.is_empty(),
        "an admitted step produces no capacity finding"
    );
}

/// S4 — a refusal consumes nothing. A gate that reserved capacity while refusing would pass
/// S1 and fail here on the second attempt, which is the defect the seed exists for.
#[tokio::test]
async fn a_refusal_consumes_nothing_so_the_second_attempt_reports_the_same_overage() {
    let first = evaluate(capacity_ports(Facts {
        exposure: Some(exposure(1_409_000)),
        requested: 18_420,
    }))
    .await;
    let second = evaluate(capacity_ports(Facts {
        exposure: Some(exposure(1_409_000)),
        requested: 18_420,
    }))
    .await;

    assert_eq!(
        refusal(&first).1,
        refusal(&second).1,
        "the same declared facts must refuse with the same numbers"
    );
    assert_eq!(
        first.envelope.decision_digest, second.envelope.decision_digest,
        "and the same digest — a refusal with a side effect is not deterministic"
    );
}

/// S5 — absence is free: no declared obligation produces no finding, no refusal, and the
/// digest of a composition that predates these ports.
#[tokio::test]
async fn no_declared_cap_produces_no_finding() {
    let decision = evaluate(GatePorts::none()).await;
    assert!(decision.verdict.permits_dispatch());
    assert!(decision.findings.is_empty());
    assert!(
        decision
            .findings
            .iter()
            .all(|f| f.gate_id != GateId::Capacity),
        "absence is a decision the operator already made, not a finding"
    );
}

/// A port that declares an exposure but no amount is *not* a refusal: `0` fits anywhere.
/// This is the degenerate case that a predicate-shaped port would have had to special-case.
#[tokio::test]
async fn a_zero_request_fits_an_exhausted_ledger() {
    let mut exhausted = exposure(1_420_000);
    exhausted.outstanding = 0;
    let decision = evaluate(capacity_ports(Facts {
        exposure: Some(exhausted),
        requested: 0,
    }))
    .await;
    assert!(decision.verdict.permits_dispatch());
}
