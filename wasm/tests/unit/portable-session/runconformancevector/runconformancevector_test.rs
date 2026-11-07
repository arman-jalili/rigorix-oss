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
        expected_decision: rigorix_gate_wasm::application::ExpectedDecision {
            hex: "7b7d".to_string(),
            sha256: "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
                .to_string(),
            envelopes: Vec::new(),
        },
        produced_by: None,
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
        decision_digest: None,
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
fn a_normalised_comparison_is_never_a_pass() {
    // No longer a labelled interim: the permission that allowed one — "while FR-011 is
    // unimplemented" — lost its precondition when ADR-014 superseded FR-011 (#1367).
    // The field survives so this assertion has somewhere to land, and the runner always
    // sets it false; the gate refuses a vector that asks for the comparison at all.
    let interim = report(true, false, true);
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

use rigorix_gate_wasm::application::{ConformanceRunner, RunConformanceVector, default_projection};

fn vector_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/conformance/vectors")
}

fn vector_from_disk() -> ConformanceVector {
    let raw = std::fs::read_to_string(vector_dir().join("vec-0001.json"))
        .expect("committed vector exists");
    serde_json::from_str(&raw).expect("vector parses")
}

/// **Every** committed vector, not the one this file happens to name.
///
/// #1457 found the gap the hard way: `run-conformance.sh` globbed all the vectors for the
/// provenance check — printing "3 committed vector(s)" — and then replayed exactly one, by a
/// path written into this test. Recording a fourth vector with a deliberately wrong golden
/// and watching the suite report 12/12 is what made it visible. A vector that is not replayed
/// is decoration, and a harness that skips one reads as success.
fn all_vectors() -> Vec<ConformanceVector> {
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(vector_dir())
        .expect("the vector directory exists")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    paths.sort();
    assert!(
        paths.len() >= 3,
        "a skip is a failure: {} vector(s) found",
        paths.len()
    );
    paths
        .iter()
        .map(|path| {
            let raw = std::fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("{} is unreadable: {error}", path.display()));
            serde_json::from_str(&raw)
                .unwrap_or_else(|error| panic!("{} is not a vector: {error}", path.display()))
        })
        .collect()
}

type GateFactory =
    fn(&ConformanceVector) -> Box<dyn rigorix_gate_wasm::domain::engine_seam::GateEvaluator>;
type ProjectionFactory = fn() -> Box<dyn rigorix_gate_wasm::domain::services::EvidenceProjection>;

/// The real gate, built per vector from that vector's own declaration (#1385).
///
/// The factory takes the vector because the world does: a gate constructed without one is a
/// gate that cannot decide, and a conformance run that never decides certifies the plumbing.
fn runner() -> ConformanceRunner<GateFactory, ProjectionFactory> {
    fn gate(
        vector: &ConformanceVector,
    ) -> Box<dyn rigorix_gate_wasm::domain::engine_seam::GateEvaluator> {
        #[cfg(feature = "engine")]
        {
            use rigorix_gate_wasm::infrastructure::EngineGate;
            Box::new(EngineGate::new(declaration_of(vector)))
        }
        #[cfg(not(feature = "engine"))]
        {
            let _ = vector;
            Box::new(EngineGate::unavailable())
        }
    }
    ConformanceRunner::new(
        gate as fn(&ConformanceVector) -> _,
        default_projection as fn() -> _,
    )
}

/// The declaration a host sends: `fixtures`, `clock`, `seed`.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn declaration_of(vector: &ConformanceVector) -> serde_json::Value {
    serde_json::json!({
        "fixtures": vector.fixtures,
        "clock": vector.clock,
        "seed": vector.seed,
    })
}

#[test]
fn a_vector_replays_to_a_stamped_artifact() {
    let vector = vector_from_disk();
    let artifact = runner().replay(&vector).expect("replay");
    assert_eq!(artifact.stamp.artifact_format_version, 2);
    // The engine assembles no envelope without a run context, and the vector supplies none:
    // the artifact therefore carries the findings, not a signed envelope it would have had
    // to invent (ADR-010). It carried one while the gate was a stub answering with one.
    assert_eq!(artifact.envelopes.len(), 0);
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
    // The golden is the CLI's, recorded in the vector; the engine re-derives it here. That
    // is the comparison `gate: "cli"` claims (#1385), not a crate-vs-itself mismatch.
    let mut vector = vector_from_disk();
    let report = runner().execute(&vector);
    assert!(
        report.matched,
        "the committed golden must match the engine's decision ({:?})",
        report
    );
    assert!(
        !report.normalised_comparison,
        "never claim a normalised pass"
    );
    assert!(report.is_pass(false));

    // #1442: the report carries the digest of the decision the crate produced, so a vector
    // proves the boundary handed the host what the engine computed — not a digest the harness
    // recomputed for the occasion.
    let digest = report
        .decision_digest
        .as_deref()
        .expect("a vector that decided carries its decision digest");
    assert_eq!(
        digest,
        format!(
            "sha256:{}",
            sha256_hex(&vector.expected_decision.bytes().expect("recorded bytes"))
        ),
        "the report's digest must name the bytes the vector records as the engine's"
    );

    // One added byte in the recorded decision is a divergence, and the digest the vector
    // carries goes stale with it — both are findings.
    vector.expected_decision.hex.push_str("20");
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
    // A vector must say where its expected bytes came from, and a `cli` label must name the
    // command and the commit that produced them (#1385).
    let raw = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/conformance/vectors/vec-0001.json"),
    )
    .expect("vector");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("json");
    assert_eq!(parsed["gate"], "cli");
    assert!(
        parsed["produced_by"]["command"]
            .as_str()
            .expect("command")
            .contains("gate-vector"),
        "the provenance names the command that produced the bytes"
    );
    assert_eq!(
        parsed["produced_by"]["commit"]
            .as_str()
            .expect("commit")
            .len(),
        40,
        "the provenance names a commit"
    );
}

#[test]
fn a_report_never_claims_a_normalised_comparison() {
    let vector = vector_from_disk();
    let report: ConformanceReport = runner().execute(&vector);
    assert!(!report.normalised_comparison);
}

#[test]
fn the_runner_decides_with_the_engine_rather_than_a_fixture() {
    // #1385: the harness had never run the real gate. A runner that answers from a stub
    // cannot notice an engine that changed its mind, so the divergence below is the proof
    // that this one can.
    let mut vector = vector_from_disk();
    let report = runner().execute(&vector);
    assert!(report.matched);
    assert!(!report.skipped);

    // Flip the declared determination: the engine now refuses where it passed, so the
    // committed golden no longer matches — and it is the *engine's* change that is detected.
    vector.fixtures["providers"]["eligibility.check"]["determination"] =
        serde_json::json!("denied");
    let report = runner().execute(&vector);
    assert!(
        !report.matched,
        "a changed determination must change the decision the runner compares"
    );
}

// ── #1457 · the harness replays what it claims to replay ───────────────────────────

#[test]
fn every_committed_vector_replays_to_the_bytes_it_recorded() {
    for vector in &all_vectors() {
        let report = runner().execute(vector);
        assert!(
            report.is_pass(vector.seeded_divergence),
            "{} did not replay to its recorded bytes: {report:?}",
            vector.id
        );
    }
}

#[test]
fn a_wrong_golden_fails_for_every_committed_vector() {
    // The harness's own seed. Without it, "the vectors replay" is a claim about whichever
    // vector this file names, and a broken golden gets a shrug (#1457).
    for vector in &all_vectors() {
        let mut mutated = vector.clone();
        mutated.expected_decision.sha256 = format!("sha256:{}", "0".repeat(64));
        assert!(
            !runner().execute(&mutated).is_pass(vector.seeded_divergence),
            "{} passed with a zeroed golden — the comparison was never reached",
            vector.id
        );
    }
}
