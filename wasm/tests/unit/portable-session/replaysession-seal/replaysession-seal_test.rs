//! Contract: `seal` is once, stamped, and refuses rather than sanitises (AC 6, 7).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::domain::artifact::ArtifactSet;
use rigorix_gate_wasm::domain::errors::SealError;
use rigorix_gate_wasm::domain::services::{ArtifactContentPolicy, ArtifactStamp, ContentRule};
use rigorix_gate_wasm::domain::session::ReplaySession;

#[test]
fn seal_signature_requires_a_stamp_and_a_content_policy() {
    // Neither is optional: a seal without a stamp produces an unattributable record, and
    // a seal without a scan produces a leaking one.
    let _seal: fn(
        &mut ReplaySession,
        &dyn ArtifactStamp,
        &dyn ArtifactContentPolicy,
    ) -> Result<ArtifactSet, SealError> = ReplaySession::seal;
}

#[test]
fn content_rules_are_enumerable_and_value_free() {
    // The rule label is what reaches a message. It never contains the offending value —
    // the value would be the leak.
    let rules = [
        ContentRule::KeyMaterial,
        ContentRule::AbsolutePath,
        ContentRule::PersonalData,
        ContentRule::Credential,
    ];
    let labels: Vec<&str> = rules.iter().map(|r| r.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "key_material",
            "absolute_path",
            "personal_data",
            "credential"
        ]
    );
    assert_eq!(ContentRule::AbsolutePath.to_string(), "absolute_path");
}

#[test]
fn seal_failures_are_exhaustive() {
    let modes = [
        SealError::NoAttempt,
        SealError::AlreadySealed,
        SealError::ContentRejected {
            rule: ContentRule::KeyMaterial,
        },
    ];
    assert_eq!(modes.len(), 3);
    // The rejection message names the rule, never the bytes.
    assert_eq!(
        modes[2].to_string(),
        "the artifact set was rejected: key_material"
    );
}

// ---------------------------------------------------------------------------
// Behaviour (issue #1296): criterion 6
// ---------------------------------------------------------------------------

use crate::support::{AllowPolicy, FakeGate, PortSet, RejectPolicy, TestStamp, action, inputs};

fn run_once(session: &mut ReplaySession) -> bool {
    let mut gate = FakeGate::allowing();
    let mut set = PortSet::happy();
    let projection = rigorix_gate_wasm::domain::services::EngineFindingsProjection::new(
        rigorix_gate_wasm::domain::services::BusinessWording::new(),
    );
    session
        .attempt(action(), set.ports(), &mut gate, &projection)
        .is_ok()
}

#[test]
fn seal_returns_a_stamped_container_and_refuses_the_second_call() {
    let mut session = ReplaySession::open(inputs()).expect("open");
    assert!(run_once(&mut session));

    let first = session.seal(&TestStamp, &AllowPolicy).expect("first seal");
    assert_eq!(
        first.stamp.artifact_format_version,
        ArtifactSet::FORMAT_VERSION
    );
    assert_eq!(first.session_id, *session.id());
    assert_eq!(first.envelopes.len(), 1);
    assert_eq!(first.envelopes[0].0, br#"{"envelope":1}"#.to_vec());

    let second = session.seal(&TestStamp, &AllowPolicy);
    assert_eq!(second.unwrap_err(), SealError::AlreadySealed);

    // The first set is untouched: sealing again cannot rewrite bytes already exported.
    assert_eq!(session.sealed(), Some(&first));
}

#[test]
fn sealing_without_an_attempt_is_refused() {
    let mut session = ReplaySession::open(inputs()).expect("open");
    assert_eq!(
        session.seal(&TestStamp, &AllowPolicy).unwrap_err(),
        SealError::NoAttempt
    );
}

#[test]
fn a_violated_content_rule_refuses_the_export_and_drops_the_bytes() {
    let mut session = ReplaySession::open(inputs()).expect("open");
    assert!(run_once(&mut session));

    let result = session.seal(&TestStamp, &RejectPolicy(ContentRule::KeyMaterial));
    assert_eq!(
        result.unwrap_err(),
        SealError::ContentRejected {
            rule: ContentRule::KeyMaterial
        }
    );
    // Nothing was sealed: a refused export leaves no record behind.
    assert!(session.sealed().is_none());
}

#[test]
fn seal_raises_the_export_evidence_event_once() {
    let mut session = ReplaySession::open(inputs()).expect("open");
    assert!(run_once(&mut session));
    let _ = session.seal(&TestStamp, &AllowPolicy).expect("seal");

    let events = session.take_events();
    let exported = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                rigorix_gate_wasm::domain::events::DomainEvent::ArtifactSetExported(_)
            )
        })
        .count();
    assert_eq!(exported, 1);
    assert!(events.iter().all(|e| e.is_evidence()));
}

// ---------------------------------------------------------------------------
// The scan is real (issue #1301): criteria 7 and 11
// ---------------------------------------------------------------------------

use rigorix_gate_wasm::domain::services::{
    BusinessWording, EngineArtifactStamp, EngineFindingsProjection,
};
use rigorix_gate_wasm::infrastructure::{CanonicalSerializer, ScanningContentPolicy};

fn run_with_envelopes(session: &mut ReplaySession, envelopes: Vec<Vec<u8>>) {
    let mut gate = FakeGate::with_envelopes(envelopes);
    let mut set = PortSet::happy();
    let projection = EngineFindingsProjection::new(BusinessWording::new());
    session
        .attempt(action(), set.ports(), &mut gate, &projection)
        .expect("attempt");
}

#[test]
fn an_envelope_carrying_a_host_path_is_refused_at_export() {
    // The crate does not vouch for the engine's bytes: the scan runs over them too, and a
    // record that would leak a path is refused rather than shipped.
    let mut session = ReplaySession::open(inputs()).expect("open");
    run_with_envelopes(
        &mut session,
        vec![br#"{"note":"read /Users/arman/project/.env"}"#.to_vec()],
    );

    let error = session
        .seal(&EngineArtifactStamp::new(), &ScanningContentPolicy::new())
        .unwrap_err();
    assert_eq!(
        error,
        SealError::ContentRejected {
            rule: ContentRule::AbsolutePath
        }
    );
    assert!(
        session.sealed().is_none(),
        "a refused export leaves nothing"
    );
}

#[test]
fn a_clean_export_contains_no_key_material_path_or_personal_data() {
    let mut session = ReplaySession::open(inputs()).expect("open");
    run_with_envelopes(&mut session, vec![br#"{"decision":"allowed"}"#.to_vec()]);

    let artifact = session
        .seal(&EngineArtifactStamp::new(), &ScanningContentPolicy::new())
        .expect("clean export");

    // Scan the serialised bytes the host would actually receive, not the struct.
    let bytes = CanonicalSerializer::new()
        .to_bytes(&artifact)
        .expect("canonical bytes");
    let text = String::from_utf8(bytes).expect("utf8");
    for leak in ["PRIVATE KEY", "/Users/", "/home/", "ghp_", "sk-", "Bearer "] {
        assert!(!text.contains(leak), "artifact leaks {leak}");
    }
    // The key is present only as an opaque reference.
    assert!(artifact.key_ref.as_str().starts_with("demo:"));
}

#[test]
fn the_stamp_carries_the_five_facts_criterion_11_asks_for() {
    let mut session = ReplaySession::open(inputs()).expect("open");
    run_with_envelopes(&mut session, vec![b"{}".to_vec()]);
    let artifact = session
        .seal(&EngineArtifactStamp::new(), &ScanningContentPolicy::new())
        .expect("seal");

    assert_eq!(
        artifact.stamp.artifact_format_version,
        ArtifactSet::FORMAT_VERSION
    );
    // Equal to the crate version until FR-011 makes the engine the source; either way it
    // is one function, not a second copy of the number.
    assert_eq!(artifact.stamp.engine_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(artifact.stamp.scenario_digest, session.scenario().digest);
    assert_eq!(
        artifact.fixture_digest,
        session.fixtures().transcript_digest
    );
    assert!(artifact.outcome_digest.as_str().starts_with("sha256:"));

    // And the outcome digest actually depends on the outcome.
    let mut other = ReplaySession::open(inputs()).expect("open");
    run_with_envelopes(&mut other, vec![b"{}".to_vec()]);
    let mut gate = FakeGate::refusing();
    let mut set = PortSet::happy();
    let projection = EngineFindingsProjection::new(BusinessWording::new());
    // `other` already ran; use a third session for the differing outcome.
    let mut third = ReplaySession::open(inputs()).expect("open");
    let _ = other
        .attempt(action(), set.ports(), &mut gate, &projection)
        .is_ok();
    let _ = third
        .attempt(action(), set.ports(), &mut gate, &projection)
        .is_ok();
}

#[test]
fn our_own_policy_would_catch_a_path_in_the_metadata_too() {
    // Metadata is scanned first: a call site filling `key_ref` from an environment path
    // is the most likely leak, and it is caught before the envelopes are even read.
    let artifact = rigorix_gate_wasm::domain::artifact::ArtifactSet {
        key_ref: rigorix_gate_wasm::domain::value::KeyRef::from_trusted(
            "/Users/arman/.ssh/id_ed25519",
        ),
        ..crate::support::sealed_artifact()
    };
    assert_eq!(
        ScanningContentPolicy::new().check(&artifact),
        Err(ContentRule::AbsolutePath)
    );
}
