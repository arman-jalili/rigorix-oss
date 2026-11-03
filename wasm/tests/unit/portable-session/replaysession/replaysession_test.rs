//! Contract: `ReplaySession` is the aggregate root and exposes behaviour, not state.
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::domain::session::{OpenInputs, ReplaySession};
use rigorix_gate_wasm::domain::value::{
    ClockReading, ClockTrack, Digest, Draw, FixtureTranscript, FixtureWorld, RandomSeed,
    ScenarioFormatVersion, ScenarioRef, Timestamp,
};

pub(crate) fn digest(byte: char) -> Digest {
    Digest::from_trusted(format!("sha256:{}", byte.to_string().repeat(64)))
}

pub(crate) fn inputs(seed_byte: char) -> OpenInputs {
    OpenInputs {
        scenario: ScenarioRef::new(digest('a'), ScenarioFormatVersion(1)),
        fixtures: FixtureWorld::new(
            digest('b'),
            FixtureTranscript(serde_json::json!({ "approvals": [] })),
        ),
        clock: ClockTrack::new(vec![ClockReading {
            instant: Timestamp::from_trusted("2026-11-03T09:00:00Z"),
            index: 0,
        }]),
        seed: RandomSeed::new(digest(seed_byte), vec![Draw::from_trusted("00")]),
    }
}

#[test]
fn aggregate_exposes_behaviour_without_state_access() {
    // The contract is the signature: inputs in, session or typed refusal out.
    let open: fn(OpenInputs) -> Result<ReplaySession, _> = ReplaySession::open;
    let _ = open;
    // Read projections are the only way to see inside, and they are all `&self`.
    let _: fn(&ReplaySession) -> &rigorix_gate_wasm::domain::value::ReplaySessionId =
        ReplaySession::id;
    let _: fn(&ReplaySession) -> Option<&rigorix_gate_wasm::domain::outcome::RunOutcome> =
        ReplaySession::outcome;
    let _: fn(&ReplaySession) -> Option<&rigorix_gate_wasm::domain::artifact::ArtifactSet> =
        ReplaySession::sealed;
}

#[test]
fn open_inputs_are_all_required() {
    // No field is optional and nothing has a default: a session cannot be built without
    // pinning scenario, fixtures, clock and seed. A `Default` here would be the first
    // step toward "a refusal became an allow".
    let pinned = inputs('c');
    assert_eq!(pinned.scenario.format_version, ScenarioFormatVersion(1));
    assert_eq!(pinned.clock.readings().len(), 1);
    assert_eq!(pinned.seed.draws.len(), 1);
    assert_eq!(pinned.fixtures.transcript_digest, digest('b'));
}

#[test]
fn events_are_drained_from_the_aggregate() {
    // `take_events` is the only way events leave: there is no bus and no static channel
    // (ADR-003).
    let _: fn(&mut ReplaySession) -> Vec<rigorix_gate_wasm::domain::events::DomainEvent> =
        ReplaySession::take_events;
}

// ---------------------------------------------------------------------------
// AC 2 — derived identity (issue #1295)
// ---------------------------------------------------------------------------

#[test]
fn identical_inputs_derive_identical_ids() {
    let left = ReplaySession::open(inputs('c')).expect("valid inputs");
    let right = ReplaySession::open(inputs('c')).expect("valid inputs");
    assert_eq!(left.id(), right.id());
    // The id is a digest, not a counter: reopening the same vector in a fresh instance
    // yields the same identity, which is what makes a replay comparable.
    assert!(left.id().as_str().starts_with("sha256:"));
}

#[test]
fn a_different_seed_derives_a_different_id() {
    let same = ReplaySession::open(inputs('c')).expect("valid inputs");
    let other = ReplaySession::open(inputs('d')).expect("valid inputs");
    assert_ne!(same.id(), other.id());
}

#[test]
fn open_raises_the_session_opened_evidence_event() {
    let mut session = ReplaySession::open(inputs('c')).expect("valid inputs");
    let events = session.take_events();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].class(),
        rigorix_gate_wasm::domain::events::EventClass::Evidence
    );
    assert!(events[0].is_evidence());
    // Draining is a move: a second call is empty, so an event cannot be replayed twice
    // into two artifacts.
    assert!(session.take_events().is_empty());
}

#[test]
fn open_refuses_instead_of_defaulting() {
    use rigorix_gate_wasm::domain::errors::SessionOpenError;

    // Empty clock series.
    let mut empty_clock = inputs('c');
    empty_clock.clock = ClockTrack::default();
    assert_eq!(
        ReplaySession::open(empty_clock).unwrap_err(),
        SessionOpenError::ClockSeriesEmpty
    );

    // Unsupported scenario format.
    let mut wrong_version = inputs('c');
    wrong_version.scenario = ScenarioRef::new(digest('a'), ScenarioFormatVersion(99));
    assert_eq!(
        ReplaySession::open(wrong_version).unwrap_err(),
        SessionOpenError::UnsupportedScenarioFormat {
            found: ScenarioFormatVersion(99)
        }
    );

    // A transcript that is not an object cannot be read at all.
    let mut not_an_object = inputs('c');
    not_an_object.fixtures =
        FixtureWorld::new(digest('b'), FixtureTranscript(serde_json::json!(42)));
    assert_eq!(
        ReplaySession::open(not_an_object).unwrap_err(),
        SessionOpenError::FixtureUnparseable
    );

    // An object missing the approvals array is incomplete, and says which field.
    let mut missing = inputs('c');
    missing.fixtures = FixtureWorld::new(digest('b'), FixtureTranscript(serde_json::json!({})));
    assert_eq!(
        ReplaySession::open(missing).unwrap_err(),
        SessionOpenError::FixtureIncomplete { field: "approvals" }
    );
}
