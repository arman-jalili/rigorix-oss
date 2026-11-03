//! Contract: conformance never compares approximately and never skips (AC 20, 21).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::application::{ConformanceReport, ConformanceVector};

fn vector(seeded: bool) -> ConformanceVector {
    ConformanceVector {
        id: if seeded { "seeded-0001" } else { "vec-0001" }.into(),
        scenario: serde_json::json!({}),
        fixtures: serde_json::json!({}),
        clock: serde_json::json!([]),
        seed: serde_json::json!({}),
        expected: b"{}".to_vec(),
        seeded_divergence: seeded,
        gate: Some("fixture".to_string()),
    }
}

fn report(matched: bool, skipped: bool, normalised: bool) -> ConformanceReport {
    ConformanceReport {
        vector_id: "vec-0001".into(),
        matched,
        normalised_comparison: normalised,
        skipped,
    }
}

#[test]
fn a_matching_vector_passes_and_a_differing_one_fails() {
    assert!(report(true, false, false).is_pass(false));
    assert!(!report(false, false, false).is_pass(false));
}

#[test]
fn a_skip_is_a_failure_never_a_pass() {
    assert!(!report(true, true, false).is_pass(false));
    assert!(!report(false, true, false).is_pass(false));
}

#[test]
fn the_seeded_vector_passes_only_when_it_diverges() {
    // The proof that the comparator works: if the deliberately-altered vector starts
    // matching, the harness is broken and the build must fail.
    assert!(report(false, false, false).is_pass(true));
    assert!(!report(true, false, false).is_pass(true));
}

#[test]
fn a_normalised_comparison_is_labelled() {
    // Allowed only as an interim while FR-011 is unimplemented — and it is reported as
    // what it is, so nobody can mistake it for byte identity (FR-012).
    let interim = report(true, false, true);
    assert!(interim.normalised_comparison);
    assert!(
        !interim.is_pass(true),
        "a normalised pass is not a conformance pass"
    );
}

#[test]
fn vectors_are_immutable_artifacts_with_an_id() {
    let v = vector(false);
    assert!(!v.id.is_empty());
    assert!(!v.seeded_divergence);
    assert!(vector(true).seeded_divergence);
}

// ---------------------------------------------------------------------------
// The runner (issue #1312)
// ---------------------------------------------------------------------------

use crate::support::FakeGate;
use rigorix_gate_wasm::application::{ConformanceRunner, RunConformanceVector, default_projection};

fn vector_from_disk() -> ConformanceVector {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/conformance/vectors/vec-0001.json");
    let raw = std::fs::read_to_string(path).expect("committed vector exists");
    serde_json::from_str(&raw).expect("vector parses")
}

type GateFactory = fn() -> Box<dyn rigorix_gate_wasm::domain::engine_seam::GateEvaluator>;
type ProjectionFactory = fn() -> Box<dyn rigorix_gate_wasm::domain::services::EvidenceProjection>;

fn runner() -> ConformanceRunner<GateFactory, ProjectionFactory> {
    fn gate() -> Box<dyn rigorix_gate_wasm::domain::engine_seam::GateEvaluator> {
        Box::new(FakeGate::allowing())
    }
    ConformanceRunner::new(gate as fn() -> _, default_projection as fn() -> _)
}

#[test]
fn a_vector_replays_to_a_stamped_artifact() {
    let vector = vector_from_disk();
    let artifact = runner().replay(&vector).expect("replay");
    assert_eq!(artifact.stamp.artifact_format_version, 1);
    assert_eq!(artifact.envelopes.len(), 1);
    assert!(artifact.outcome_digest.as_str().starts_with("sha256:"));
}

#[test]
fn replaying_the_same_vector_twice_produces_identical_bytes() {
    // Determinism is the property the whole harness rests on; if the replay were not
    // byte-stable, a mismatch would be noise rather than a finding.
    let vector = vector_from_disk();
    let first = serde_json::to_vec(&runner().replay(&vector).expect("first")).expect("bytes");
    let second = serde_json::to_vec(&runner().replay(&vector).expect("second")).expect("bytes");
    assert_eq!(first, second);
}

#[test]
fn a_matching_vector_passes_and_a_mutated_expected_fails() {
    let mut vector = vector_from_disk();
    let produced = serde_json::to_vec(&runner().replay(&vector).expect("replay")).expect("bytes");

    vector.expected = produced.clone();
    let report = runner().execute(&vector);
    assert!(report.matched);
    assert!(
        !report.normalised_comparison,
        "never claim a normalised pass"
    );
    assert!(report.is_pass(false));

    // One extra byte in the expected artifact is a divergence.
    let mut wrong = produced.clone();
    wrong.push(b' ');
    vector.expected = wrong;
    let report = runner().execute(&vector);
    assert!(!report.matched);
    assert!(!report.is_pass(false));
}

#[test]
fn an_unreplayable_vector_is_a_skip_and_a_skip_is_a_failure() {
    // A vector whose clock is empty cannot be replayed. Skipping it would read as success,
    // which is the failure mode criterion 20 names.
    let mut vector = vector_from_disk();
    vector.clock = serde_json::json!([]);
    let report = runner().execute(&vector);
    assert!(report.skipped);
    assert!(!report.is_pass(false));
}

#[test]
fn the_committed_vector_carries_its_own_provenance_label() {
    // A vector must say where its expected bytes came from. Ours say `fixture`: produced by
    // this crate, not by the CLI. When FR-011 lands, the CLI vectors say `cli` and this
    // test's expectation moves with them.
    let raw = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/conformance/vectors/vec-0001.json"),
    )
    .expect("vector");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("json");
    assert_eq!(parsed["gate"], "fixture");
    assert!(
        parsed["note"]
            .as_str()
            .expect("note")
            .contains("does NOT prove byte identity with the CLI")
    );
}

#[test]
fn a_report_never_claims_a_normalised_comparison() {
    let mut vector = vector_from_disk();
    let produced = serde_json::to_vec(&runner().replay(&vector).expect("replay")).expect("bytes");
    vector.expected = produced;
    let report: ConformanceReport = runner().execute(&vector);
    assert!(!report.normalised_comparison);
}
