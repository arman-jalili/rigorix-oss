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
