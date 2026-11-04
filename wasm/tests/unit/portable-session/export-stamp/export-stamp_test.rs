//! Contract: the stamp names the producer and the format (AC 11, 12; FR-010, FR-014).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::domain::artifact::{ArtifactSet, VersionStamp};
use rigorix_gate_wasm::domain::value::Digest;

fn digest(byte: char) -> Digest {
    Digest::from_trusted(format!("sha256:{}", byte.to_string().repeat(64)))
}

#[test]
fn the_container_has_a_format_version() {
    // A container without a version is a record nobody can decide how to read.
    assert_eq!(ArtifactSet::FORMAT_VERSION, 2);
}

#[test]
fn the_stamp_carries_three_facts() {
    let stamp = VersionStamp {
        artifact_format_version: ArtifactSet::FORMAT_VERSION,
        engine_version: "1.12.1".into(),
        scenario_digest: digest('a'),
    };
    // Engine provenance, container version, and which scenario produced it: a record
    // missing any of the three cannot be re-read honestly.
    assert_eq!(stamp.artifact_format_version, 2);
    assert!(!stamp.engine_version.is_empty());
    assert_eq!(stamp.scenario_digest, digest('a'));
}

#[test]
fn an_artifact_set_carries_both_digests_and_an_opaque_key_ref() {
    let artifact = ArtifactSet {
        stamp: VersionStamp {
            artifact_format_version: ArtifactSet::FORMAT_VERSION,
            engine_version: "1.12.1".into(),
            scenario_digest: digest('a'),
        },
        fixture_digest: digest('b'),
        session_id: rigorix_gate_wasm::domain::value::ReplaySessionId::from_digest(digest('c')),
        key_ref: rigorix_gate_wasm::domain::value::KeyRef::from_trusted("demo:sha256:deadbeef"),
        envelopes: vec![rigorix_gate_wasm::domain::artifact::EnvelopeBytes(vec![
            1, 2, 3,
        ])],
        evidence: vec![],
        outcome_digest: digest('d'),
    };
    // Envelopes are bytes, not structures: re-encoding is how byte identity dies.
    assert_eq!(artifact.envelopes[0].0, vec![1, 2, 3]);
    // The key is a reference. Nothing key-shaped can be put here.
    assert!(artifact.key_ref.as_str().starts_with("demo:"));
}

// ---------------------------------------------------------------------------
// Criterion 12: the format version is a shape question, not a verdict
// ---------------------------------------------------------------------------

#[test]
fn the_crate_knows_which_container_shapes_it_understands() {
    assert!(ArtifactSet::supports(ArtifactSet::FORMAT_VERSION));
    // A future container shape is not readable by this build, and says so.
    assert!(!ArtifactSet::supports(ArtifactSet::FORMAT_VERSION + 1));
    assert!(!ArtifactSet::supports(0));
}

#[test]
fn an_unknown_format_is_identifiable_without_any_verdict_from_the_crate() {
    // A host that receives a record from a newer producer can ask this one question and
    // then hand the bytes to its own verifier. Nothing here says "not_verified": the crate
    // has no vocabulary for authenticity (ADR-013).
    let foreign = 7_u32;
    assert!(!ArtifactSet::supports(foreign));

    // The stamp still names the producer honestly; the host decides what to do with it.
    let stamp = VersionStamp {
        artifact_format_version: foreign,
        engine_version: "9.9.9".into(),
        scenario_digest: digest('a'),
    };
    assert_eq!(stamp.artifact_format_version, foreign);
}

#[test]
fn the_crate_never_emits_the_hosts_verdict_vocabulary() {
    // The source scan in the vocabulary test covers `src/`; this pins the *published
    // surface*: the four exports and their DTOs.
    let surface = format!(
        "{:?}{:?}",
        rigorix_gate_wasm::interfaces::CONTRACT_FUNCTIONS,
        {
            let dto = rigorix_gate_wasm::interfaces::dto::ArtifactSetDto {
                artifact_format_version: 2,
                engine_version: "1.12.1".into(),
                scenario_digest: "sha256:a".into(),
                fixture_digest: "sha256:b".into(),
                session_id: "sha256:c".into(),
                key_ref: "demo:key".into(),
                envelopes: vec![],
                evidence: vec![],
                outcome_digest: "sha256:d".into(),
            };
            serde_json::to_string(&dto).expect("serialises")
        }
    )
    .to_lowercase();

    for forbidden in [
        "not_verified",
        "unverifiable",
        "unauthenticated",
        "verified\"",
    ] {
        assert!(!surface.contains(forbidden), "surface exposes {forbidden}");
    }
}
