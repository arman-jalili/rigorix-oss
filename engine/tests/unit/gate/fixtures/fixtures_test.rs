//! The fixture world: one declaration, one interpretation, one set of bytes.
//!
//! Issue: #1414
//!
//! The subject is not "a world can be built" but "the same declaration produces the same
//! decision, and a declaration that changes produces a different one". Each seed below
//! mutates exactly one declared value and asserts what the decision does — including the two
//! that would pass in a world that quietly read a clock or bypassed a port.
//!
//! The vector under test is the one #1385 will use as its reference
//! (`wasm/tests/conformance/vectors/vec-fixture-0001.json`), read from the repository rather
//! than restated here, so a change to it is a change to this test.

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;

use rigorix_engine::gate::application::fixtures::{FixtureDeclaration, FixtureError, FixtureWorld};
use rigorix_engine::gate::application::{GateRequest, evaluate_gate_sync};
use rigorix_engine::gate::domain::{
    GateDecision, GatedStep, decision_canonical_bytes, decision_digest,
};

#[derive(Clone, Deserialize)]
struct VectorFile {
    #[serde(flatten)]
    declaration: FixtureDeclaration,
    scenario: Scenario,
}

#[derive(Clone, Deserialize)]
struct Scenario {
    action: Action,
}

#[derive(Clone, Deserialize)]
struct Action {
    name: String,
    #[serde(default)]
    params: Value,
}

fn vector_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../wasm/tests/conformance/vectors/vec-fixture-0001.json")
}

fn vector() -> VectorFile {
    let text = std::fs::read_to_string(vector_path()).expect("the reference vector exists");
    serde_json::from_str(&text).expect("the reference vector parses as a declaration")
}

/// Replay a declaration exactly as `rigorix gate-vector` does: world → pins → step → decision.
fn replay(vector: &VectorFile) -> GateDecision {
    let world = FixtureWorld::from_declaration(&vector.declaration)
        .expect("the reference declaration is well formed");
    let parameters = vector
        .scenario
        .action
        .params
        .as_object()
        .cloned()
        .unwrap_or_default();
    let name = vector.scenario.action.name.clone();
    let step = GatedStep::new(name.clone(), name.clone(), Value::Object(parameters));
    evaluate_gate_sync(GateRequest {
        step: &step,
        proposed_action: &name,
        identity_attested: false,
        pins: world.pins(),
        ports: world.ports(),
        run_context: None,
    })
    .expect("a declared world can always evaluate")
}

fn digest(decision: &GateDecision) -> String {
    decision_digest(&decision.verdict, &decision.findings)
}

// ── the declaration becomes ports, and the ports decide ─────────────────────

#[test]
fn the_declared_world_decides_without_reading_anything() {
    let decision = replay(&vector());
    assert!(
        !decision.permits_dispatch(),
        "the reference vector declares one refusing provider and one passing one, so the \
         verdict is a refusal: {:?}",
        decision.verdict
    );
    let bytes = decision_canonical_bytes(&decision.verdict, &decision.findings)
        .expect("the decision canonicalises");
    assert!(!bytes.is_empty());
    // The declared observation instant is in the bytes; the decision instant is not.
    let text = String::from_utf8(bytes).expect("the canonical bytes are UTF-8 JSON");
    assert!(
        text.contains("2026-11-03T08:59:59"),
        "the declared observation instant is in the decision: {text}"
    );
    assert!(
        !text.contains("2026-11-03T09:00:00"),
        "the decision instant is the envelope's pin, not a finding's field: {text}"
    );
}

// ── S1 … S4 ─────────────────────────────────────────────────────────────────

#[test]
fn s1_replaying_the_same_vector_twice_is_the_same_digest() {
    let first = digest(&replay(&vector()));
    let second = digest(&replay(&vector()));
    assert_eq!(
        first, second,
        "two replays of one declaration must produce one digest; a difference means something \
         ambient entered the decision"
    );
}

#[test]
fn s2_changing_the_declared_observation_instant_changes_the_digest() {
    let mut mutated = vector();
    mutated.declaration.fixtures.observed_at = Some(
        "2026-11-03T08:59:58Z"
            .parse::<chrono::DateTime<chrono::Utc>>()
            .expect("an instant"),
    );

    let base = digest(&replay(&vector()));
    let moved = digest(&replay(&mutated));
    assert_ne!(
        base, moved,
        "the observation instant is in `checked_at` (service_impl.rs:177) and the digest covers \
         it; an implementation that ignores it is not reading the declaration"
    );
}

#[test]
fn s3_changing_a_determination_changes_the_digest_and_the_verdict() {
    // Dispatch: both declared providers permit. The committed vector refuses, so the
    // Dispatch → Refuse transition is asserted on the mutation that causes it.
    let mut permitting = vector();
    for provider in permitting.declaration.fixtures.providers.values_mut() {
        provider.determination = "passed".to_string();
    }
    let dispatched = replay(&permitting);
    assert!(
        dispatched.permits_dispatch(),
        "two permitting declarations must dispatch, or the port is not consulted: {:?}",
        dispatched.verdict
    );

    let mut refusing = permitting.clone();
    refusing
        .declaration
        .fixtures
        .providers
        .get_mut("budget.check")
        .expect("the vector declares budget.check")
        .determination = "denied".to_string();
    let refused = replay(&refusing);

    assert!(
        !refused.permits_dispatch(),
        "a denying provider must refuse"
    );
    assert_ne!(
        digest(&dispatched),
        digest(&refused),
        "the determination is in the findings, so the digest must move with it"
    );
}

#[test]
fn s4_a_provider_without_an_endpoint_is_a_configuration_error_that_names_it() {
    let mut mutated = vector();
    mutated
        .declaration
        .fixtures
        .providers
        .get_mut("budget.check")
        .expect("the vector declares budget.check")
        .endpoint = None;

    match FixtureWorld::from_declaration(&mutated.declaration) {
        Err(FixtureError::MissingEndpoint { provider }) => {
            assert_eq!(provider, "budget.check", "the error must name the provider");
            assert_eq!(
                FixtureError::MissingEndpoint {
                    provider: "budget.check".to_string()
                }
                .to_string(),
                "provider 'budget.check' declares no endpoint — the engine records a redacted \
                 endpoint in evidence"
            );
        }
        other => panic!("expected MissingEndpoint for budget.check, got {other:?}"),
    }
}

// ── D4: the world never falls through to the wall clock ─────────────────────

#[tokio::test]
async fn d4_every_declared_provider_answers_so_the_now_branch_is_unreachable() {
    // `service_impl.rs:214` stamps `Utc::now()` on the *indeterminate* branch — the branch
    // for a provider that answered nothing. A fixture world that reached it would be
    // wall-clock dependent while S1 (two replays in one process) still compared equal, so the
    // property is asserted on the port rather than inferred from the digest.
    use rigorix_engine::precondition::application::dto::DispatchStep;

    // Both determinations permit, so the walk visits every provider: the refusal path stops
    // at the first denial, which would leave the second provider's answer unobserved.
    let mut vector = vector();
    for provider in vector.declaration.fixtures.providers.values_mut() {
        provider.determination = "passed".to_string();
    }
    let world = FixtureWorld::from_declaration(&vector.declaration).expect("well formed");
    let ports = world.ports();
    let precondition = ports.precondition.expect("the world configures the port");
    let step = DispatchStep {
        name: vector.scenario.action.name.clone(),
        tool: vector.scenario.action.name.clone(),
        parameters: vector.scenario.action.params.clone(),
    };
    let (_verdict, findings) = precondition
        .evaluate_with_findings(
            world
                .pins()
                .execution_id()
                .expect("the world pins an execution id"),
            &step,
        )
        .await
        .expect("every declared provider answers; an Err here is the wall-clock branch");

    assert_eq!(
        findings.len(),
        world.declaration().fixtures.providers.len(),
        "one finding per declared provider: a missing one means a port answered nothing"
    );
    for finding in &findings {
        assert_eq!(
            finding.checked_at,
            "2026-11-03T08:59:59Z"
                .parse::<chrono::DateTime<chrono::Utc>>()
                .expect("the declared instant"),
            "every provider's checked_at is the declared observation instant"
        );
    }
}

// ── the declaration's own gaps ─────────────────────────────────────────────

#[test]
fn an_instant_absent_from_everywhere_is_refused_rather_than_defaulted() {
    let mut mutated = vector();
    mutated.declaration.fixtures.observed_at = None;
    for provider in mutated.declaration.fixtures.providers.values_mut() {
        provider.observed_at = None;
    }
    match FixtureWorld::from_declaration(&mutated.declaration) {
        Err(FixtureError::MissingObservationInstant { provider }) => {
            assert!(provider == "budget.check" || provider == "eligibility.check");
        }
        other => panic!("expected MissingObservationInstant, got {other:?}"),
    }
}

#[test]
fn the_execution_id_is_never_minted() {
    let mut mutated = vector();
    mutated.declaration.fixtures.ids.attempt.clear();
    assert_eq!(
        FixtureWorld::from_declaration(&mutated.declaration).unwrap_err(),
        FixtureError::MissingExecutionId
    );

    mutated.declaration.fixtures.ids.attempt = vec!["attempt-0".to_string()];
    match FixtureWorld::from_declaration(&mutated.declaration) {
        Err(FixtureError::InvalidExecutionId { value }) => assert_eq!(value, "attempt-0"),
        other => panic!("expected InvalidExecutionId, got {other:?}"),
    }
}

// ── #1456 · the declared cap and the declared prior commitment ─────────────────────
//
// The world declares *facts*; the engine decides. These seeds exist so that neither half can
// drift: a declaration that was silently ignored would pass every test above, because the
// world would simply produce no port.

fn declared(fixtures: serde_json::Value) -> FixtureWorld {
    let mut mutated = vector();
    mutated.declaration.fixtures = serde_json::from_value(fixtures).expect("declaration");
    FixtureWorld::from_declaration(&mutated.declaration).expect("valid declaration")
}

#[tokio::test]
async fn a_declared_cap_reaches_the_composition_and_refuses_with_its_numbers() {
    let world = declared(serde_json::json!({
        "approvals": [],
        "providers": {},
        "ids": { "attempt": ["018f5b8e-0000-7000-8000-000000000001"] },
        "capacity": {
            "resource": "eur.outflow.daily",
            "hard_limit": 1_420_000u64,
            "committed": 1_409_000u64,
            "requested": 18_420u64,
            "window_secs": 86_400u64
        }
    }));
    let ports = world.ports();
    assert!(
        ports.capacity.is_some(),
        "the declaration must reach the port"
    );

    let decision = rigorix_engine::gate::application::evaluate_gate(
        rigorix_engine::gate::application::GateRequest {
            step: &rigorix_engine::gate::domain::GatedStep::new(
                "transfer",
                "payments.transfer",
                serde_json::json!({}),
            ),
            proposed_action: "move 18420 units",
            identity_attested: true,
            pins: world.pins(),
            ports,
            run_context: None,
        },
    )
    .await
    .expect("the gate answers");

    match decision.verdict {
        rigorix_engine::gate::domain::GateVerdict::Refuse { gate, reason, .. } => {
            assert_eq!(gate, rigorix_engine::gate::domain::GateId::Capacity);
            assert!(reason.contains("1427420"), "{reason}");
            assert!(reason.contains("7420"), "{reason}");
        }
        other => panic!("expected a capacity refusal, got {other:?}"),
    }
}

#[test]
fn a_world_that_declares_no_cap_produces_no_capacity_port() {
    let world = declared(serde_json::json!({
        "approvals": [],
        "providers": {},
        "ids": { "attempt": ["018f5b8e-0000-7000-8000-000000000001"] }
    }));
    let ports = world.ports();
    assert!(
        ports.capacity.is_none() && ports.effect.is_none(),
        "absence is a decision the operator already made, not a missing wire"
    );
}

#[tokio::test]
async fn a_declared_prior_commitment_reaches_the_composition() {
    let world = declared(serde_json::json!({
        "approvals": [],
        "providers": {},
        "ids": { "attempt": ["018f5b8e-0000-7000-8000-000000000001"] },
        "effects": { "record_id": "QR-2041", "actor": "finance", "instant": "2026-11-04T14:02:00Z" }
    }));
    let ports = world.ports();
    assert!(
        ports.effect.is_some(),
        "the declaration must reach the port"
    );

    let decision = rigorix_engine::gate::application::evaluate_gate(
        rigorix_engine::gate::application::GateRequest {
            step: &rigorix_engine::gate::domain::GatedStep::new(
                "issue_refund",
                "payments.refund",
                serde_json::json!({}),
            ),
            proposed_action: "refund",
            identity_attested: true,
            pins: world.pins(),
            ports,
            run_context: None,
        },
    )
    .await
    .expect("the gate answers");

    match decision.verdict {
        rigorix_engine::gate::domain::GateVerdict::Refuse { gate, reason, .. } => {
            assert_eq!(gate, rigorix_engine::gate::domain::GateId::Effects);
            assert!(reason.contains("QR-2041"), "{reason}");
            assert!(reason.contains("2026-11-04T14:02:00Z"), "{reason}");
        }
        other => panic!("expected an effect refusal, got {other:?}"),
    }
}
