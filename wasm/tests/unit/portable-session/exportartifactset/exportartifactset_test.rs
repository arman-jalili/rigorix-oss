//! Contract: `ExportArtifactSet` seals, stamps and scans before bytes leave (AC 11).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::application::ExportArtifactSet;
use rigorix_gate_wasm::domain::artifact::{ArtifactSet, VersionStamp};
use rigorix_gate_wasm::domain::errors::SealError;
use rigorix_gate_wasm::domain::services::{ArtifactContentPolicy, ArtifactStamp, ContentRule};
use rigorix_gate_wasm::domain::session::ReplaySession;
use rigorix_gate_wasm::domain::value::Digest;

struct Stamp;
struct Policy;

impl ArtifactStamp for Stamp {
    fn stamp(&self, _session: &ReplaySession) -> VersionStamp {
        VersionStamp {
            artifact_format_version: ArtifactSet::FORMAT_VERSION,
            engine_version: "1.12.1".into(),
            scenario_digest: Digest::from_trusted("sha256:".to_string() + &"a".repeat(64)),
        }
    }

    fn outcome_digest(&self, _session: &ReplaySession) -> Digest {
        Digest::from_trusted("sha256:".to_string() + &"b".repeat(64))
    }
}

impl ArtifactContentPolicy for Policy {
    fn check(&self, _artifact: &ArtifactSet) -> Result<(), ContentRule> {
        Err(ContentRule::KeyMaterial)
    }
}

#[test]
fn export_takes_only_a_session() {
    // No format argument, no "skip scan" flag, no destination. Anything else would let a
    // host choose what the record is.
    let _execute: fn(&dyn ExportArtifactSet, &mut ReplaySession) -> Result<ArtifactSet, SealError> =
        |s, session| s.execute(session);
}

#[test]
fn stamp_and_policy_are_constructor_dependencies() {
    let service = rigorix_gate_wasm::application::ExportArtifactSetService::new(Stamp, Policy);
    let _ = &service;
}

#[test]
fn the_policy_refuses_and_the_rule_is_what_surfaces() {
    let policy = Policy;
    let artifact = ArtifactSet {
        stamp: VersionStamp {
            artifact_format_version: ArtifactSet::FORMAT_VERSION,
            engine_version: "1.12.1".into(),
            scenario_digest: Digest::from_trusted("sha256:".to_string() + &"a".repeat(64)),
        },
        fixture_digest: Digest::from_trusted("sha256:".to_string() + &"c".repeat(64)),
        session_id: rigorix_gate_wasm::domain::value::ReplaySessionId::from_digest(
            Digest::from_trusted("sha256:".to_string() + &"d".repeat(64)),
        ),
        key_ref: rigorix_gate_wasm::domain::value::KeyRef::from_trusted("demo:key"),
        envelopes: vec![],
        evidence: vec![],
        outcome_digest: Digest::from_trusted("sha256:".to_string() + &"e".repeat(64)),
    };
    assert_eq!(policy.check(&artifact), Err(ContentRule::KeyMaterial));
}
