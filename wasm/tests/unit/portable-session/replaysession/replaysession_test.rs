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
