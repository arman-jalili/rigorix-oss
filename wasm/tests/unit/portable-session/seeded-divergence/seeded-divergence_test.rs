//! Contract: the must-fail vector fails the build if it stops diverging (AC 21; FR-008).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::application::{ConformanceReport, ConformanceVector};

fn seeded_vector() -> ConformanceVector {
    ConformanceVector {
        id: "seeded-0001".into(),
        scenario: serde_json::json!({ "steps": [] }),
        fixtures: serde_json::json!({ "providers": {} }),
        clock: serde_json::json!([{ "instant": "2026-11-03T09:00:00Z", "index": 0 }]),
        seed: serde_json::json!({ "seed_digest": "sha256:00", "draws": [] }),
        expected: b"{}".to_vec(),
        seeded_divergence: true,
        gate: Some("fixture".to_string()),
    }
}

#[test]
fn the_seeded_vector_is_marked_as_such() {
    // If it were silently dropped from the vector set, the guard would disappear without
    // a red build — so the flag is part of the committed artifact.
    assert!(seeded_vector().seeded_divergence);
}

#[test]
fn equality_on_the_seeded_vector_is_a_failure() {
    let matched_when_it_must_not = ConformanceReport {
        vector_id: "seeded-0001".into(),
        matched: true,
        normalised_comparison: false,
        skipped: false,
    };
    assert!(
        !matched_when_it_must_not.is_pass(true),
        "a comparator that cannot fail is not a comparator"
    );

    let diverged = ConformanceReport {
        vector_id: "seeded-0001".into(),
        matched: false,
        normalised_comparison: false,
        skipped: false,
    };
    assert!(diverged.is_pass(true));
}

#[test]
fn a_skipped_seeded_vector_cannot_hide() {
    let skipped = ConformanceReport {
        vector_id: "seeded-0001".into(),
        matched: false,
        normalised_comparison: false,
        skipped: true,
    };
    assert!(!skipped.is_pass(true));
}

// ---------------------------------------------------------------------------
// The committed must-fail vector (issue #1313)
// ---------------------------------------------------------------------------

use crate::support::RunnerStubGate;
use rigorix_gate_wasm::application::{ConformanceRunner, RunConformanceVector, default_projection};

type GateFactory = fn() -> Box<dyn rigorix_gate_wasm::domain::engine_seam::GateEvaluator>;
type ProjectionFactory = fn() -> Box<dyn rigorix_gate_wasm::domain::services::EvidenceProjection>;

fn runner() -> ConformanceRunner<GateFactory, ProjectionFactory> {
    fn gate() -> Box<dyn rigorix_gate_wasm::domain::engine_seam::GateEvaluator> {
        Box::new(RunnerStubGate::allowing())
    }
    ConformanceRunner::new(gate as fn() -> _, default_projection as fn() -> _)
}

fn load(name: &str) -> ConformanceVector {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/conformance/vectors")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).expect("vector exists"))
        .expect("vector parses")
}

#[test]
fn the_committed_seeded_vector_must_diverge() {
    // The single most important assertion in the harness: a vector that is *known* to
    // differ must be reported as differing. If this passes with `matched: true`, the
    // comparator is broken and the suite must go red.
    let seeded = load("seeded-0001.json");
    assert!(
        seeded.seeded_divergence,
        "the must-fail vector lost its flag"
    );

    let report = runner().execute(&seeded);
    assert!(!report.skipped, "the seeded vector must be replayable");
    assert!(
        !report.matched,
        "the seeded vector matched: the comparator cannot fail"
    );
    assert!(report.is_pass(true), "a divergence is the expected outcome");
}

#[test]
fn the_seeded_vector_differs_from_its_sibling_by_the_fixture_only() {
    // The divergence is *seeded*, not accidental: same scenario, same clock, same seed,
    // same expected bytes — one provider determination flipped. That makes a mismatch
    // attributable to the change rather than to noise.
    let plain = load("vec-0001.json");
    let seeded = load("seeded-0001.json");

    assert_eq!(plain.scenario, seeded.scenario);
    assert_eq!(plain.clock, seeded.clock);
    assert_eq!(plain.seed, seeded.seed);
    assert_eq!(plain.expected, seeded.expected);
    assert_ne!(plain.fixtures, seeded.fixtures);
}

#[test]
fn a_seeded_vector_that_starts_matching_fails_the_build() {
    // Simulate the regression: if the two became identical, `is_pass(true)` is false and
    // the suite goes red — which is what keeps the guard from rotting into a green tick.
    let mut seeded = load("seeded-0001.json");
    let produced = serde_json::to_vec(&runner().replay(&seeded).expect("replay")).expect("bytes");

    // Force a match: this is exactly the state a broken comparator would report.
    seeded.expected = produced;
    let report = runner().execute(&seeded);
    assert!(report.matched);
    assert!(
        !report.is_pass(true),
        "equality on the seeded vector must never be a pass"
    );
}
