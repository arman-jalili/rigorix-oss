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
