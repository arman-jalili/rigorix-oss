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
    assert_eq!(ArtifactSet::FORMAT_VERSION, 1);
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
    assert_eq!(stamp.artifact_format_version, 1);
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
