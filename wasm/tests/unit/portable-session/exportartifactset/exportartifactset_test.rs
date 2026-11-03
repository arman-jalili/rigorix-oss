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

// ---------------------------------------------------------------------------
// The export service (issue #1305): envelope bytes are never re-encoded
// ---------------------------------------------------------------------------

use crate::support::{AllowPolicy, FakeGate, PortSet, TestStamp, action, inputs};
use rigorix_gate_wasm::application::{
    AttemptRun, AttemptRunService, ExportArtifactSetService, OpenReplaySession,
    OpenReplaySessionService,
};
use rigorix_gate_wasm::domain::services::{BusinessWording, EngineFindingsProjection};

fn open_and_run(envelopes: Vec<Vec<u8>>) -> rigorix_gate_wasm::domain::session::ReplaySession {
    let input = inputs();
    let mut session = OpenReplaySessionService::new()
        .execute(rigorix_gate_wasm::application::OpenSessionInput {
            scenario: input.scenario,
            fixtures: input.fixtures,
            clock: input.clock,
            seed: input.seed,
        })
        .expect("open");

    let runner = AttemptRunService::new(EngineFindingsProjection::new(BusinessWording::new()));
    let mut gate = FakeGate::with_envelopes(envelopes);
    let mut set = PortSet::happy();
    runner
        .execute(&mut session, action(), set.ports(), &mut gate)
        .expect("run");
    session
}

#[test]
fn envelope_bytes_are_carried_verbatim() {
    // Non-canonical, oddly spaced JSON: if the crate re-serialised, these bytes would
    // change and the artifact would stop matching the CLI (ADR-010).
    let odd = br#"{ "z" : 1,   "a" : [ 2, 3 ] }"#.to_vec();
    let mut session = open_and_run(vec![odd.clone()]);

    let artifact = ExportArtifactSetService::new(TestStamp, AllowPolicy)
        .execute(&mut session)
        .expect("export");
    assert_eq!(artifact.envelopes.len(), 1);
    assert_eq!(artifact.envelopes[0].0, odd);
}

#[test]
fn the_export_service_carries_the_evidence_and_the_stamp() {
    let mut session = open_and_run(vec![b"{}".to_vec()]);
    let outcome = session.outcome().cloned().expect("ran");

    let artifact = ExportArtifactSetService::new(TestStamp, AllowPolicy)
        .execute(&mut session)
        .expect("export");

    assert_eq!(artifact.evidence, outcome.evidence);
    assert_eq!(artifact.session_id, *session.id());
    assert_eq!(
        artifact.fixture_digest,
        session.fixtures().transcript_digest
    );
    assert_eq!(artifact.stamp.scenario_digest, session.scenario().digest);
}

#[test]
fn exporting_without_a_run_and_exporting_twice_are_both_refused() {
    let input = inputs();
    let mut session = OpenReplaySessionService::new()
        .execute(rigorix_gate_wasm::application::OpenSessionInput {
            scenario: input.scenario,
            fixtures: input.fixtures,
            clock: input.clock,
            seed: input.seed,
        })
        .expect("open");
    let exporter = ExportArtifactSetService::new(TestStamp, AllowPolicy);

    assert_eq!(
        exporter.execute(&mut session).unwrap_err(),
        SealError::NoAttempt
    );

    let mut session = open_and_run(vec![b"{}".to_vec()]);
    let first = exporter.execute(&mut session).expect("first");
    assert_eq!(
        exporter.execute(&mut session).unwrap_err(),
        SealError::AlreadySealed
    );
    assert_eq!(session.sealed(), Some(&first));
}

#[test]
fn the_wire_shape_round_trips_what_the_host_receives() {
    // The DTO is the published shape; a field that cannot survive the round trip is a
    // field the host cannot use.
    use rigorix_gate_wasm::interfaces::dto::{ArtifactSetDto, EvidenceRefDto};

    let dto = ArtifactSetDto {
        artifact_format_version: 1,
        engine_version: "1.12.1".into(),
        scenario_digest: "sha256:".to_string() + &"a".repeat(64),
        fixture_digest: "sha256:".to_string() + &"b".repeat(64),
        session_id: "sha256:".to_string() + &"c".repeat(64),
        key_ref: "demo:key".into(),
        envelopes: vec!["{}".into()],
        evidence: vec![EvidenceRefDto {
            digest: "sha256:".to_string() + &"d".repeat(64),
            kind: "precondition".into(),
        }],
        outcome_digest: "sha256:".to_string() + &"e".repeat(64),
    };
    let json = serde_json::to_string(&dto).expect("serialises");
    let back: ArtifactSetDto = serde_json::from_str(&json).expect("parses");
    assert_eq!(back, dto);
}
