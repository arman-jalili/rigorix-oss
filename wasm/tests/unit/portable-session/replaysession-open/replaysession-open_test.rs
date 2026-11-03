//! Contract: `ReplaySession::open` rejects rather than defaults (module AC 1, 2).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::domain::errors::SessionOpenError;
use rigorix_gate_wasm::domain::session::{OpenInputs, ReplaySession};
use rigorix_gate_wasm::domain::value::ScenarioFormatVersion;

#[test]
fn open_returns_a_typed_refusal_never_a_default() {
    let open: fn(OpenInputs) -> Result<ReplaySession, SessionOpenError> = ReplaySession::open;
    let _ = open;
    // The shape is the guarantee: there is no overload returning a bare session, so a
    // caller cannot obtain one without handling a refusal.
}

#[test]
fn every_open_failure_mode_is_representable() {
    // One variant per way the input can be wrong. No `Other`, no catch-all string: a
    // catch-all is where "the scenario did not parse" becomes "something happened".
    let modes = [
        SessionOpenError::ScenarioUnparseable,
        SessionOpenError::UnsupportedScenarioFormat {
            found: ScenarioFormatVersion(99),
        },
        SessionOpenError::FixtureIncomplete { field: "approvals" },
        SessionOpenError::FixtureUnparseable,
        SessionOpenError::ClockSeriesEmpty,
    ];
    assert_eq!(modes.len(), 5);

    // Wording is business-facing and value-free: a field *name*, never its value.
    assert!(!modes[2].to_string().contains('{'));
    assert!(!modes[1].to_string().contains("sha256"));
    assert!(!modes[1].to_string().contains('/'));
}

// ---------------------------------------------------------------------------
// Identifying host input (issue #1299): criterion 1
// ---------------------------------------------------------------------------

use rigorix_gate_wasm::domain::value::{
    Digest, FixtureTranscript, FixtureWorld, FixtureWorldError, ScenarioRef, ScenarioRefError,
};

#[test]
fn a_scenario_is_identified_by_its_bytes_and_declared_version() {
    let scenario = serde_json::json!({ "format_version": 1, "steps": [] });
    let one = ScenarioRef::identify(&scenario).expect("identified");
    let two = ScenarioRef::identify(&serde_json::json!({ "steps": [], "format_version": 1 }))
        .expect("identified");

    // Key order does not matter: the digest is over canonical bytes.
    assert_eq!(one, two);
    assert_eq!(one.format_version, ScenarioFormatVersion(1));
    assert!(one.digest.as_str().starts_with("sha256:"));
}

#[test]
fn a_scenario_that_cannot_be_identified_is_refused_not_defaulted() {
    // Not an object.
    assert_eq!(
        ScenarioRef::identify(&serde_json::json!([1, 2, 3])).unwrap_err(),
        ScenarioRefError::NotAnObject
    );
    // An object with no declared version: defaulting it would evaluate an unknown format
    // against today's rules (FR-010).
    assert_eq!(
        ScenarioRef::identify(&serde_json::json!({ "steps": [] })).unwrap_err(),
        ScenarioRefError::NoFormatVersion
    );
    // A version that is not an integer is the same refusal, not a cast.
    assert_eq!(
        ScenarioRef::identify(&serde_json::json!({ "format_version": "1" })).unwrap_err(),
        ScenarioRefError::NoFormatVersion
    );
}

#[test]
fn a_transcript_is_identified_by_its_bytes() {
    let one = FixtureWorld::identify(serde_json::json!({ "approvals": [], "providers": {} }))
        .expect("identified");
    let two = FixtureWorld::identify(serde_json::json!({ "providers": {}, "approvals": [] }))
        .expect("identified");
    assert_eq!(one, two);

    assert_eq!(
        FixtureWorld::identify(serde_json::json!("nope")).unwrap_err(),
        FixtureWorldError::NotAnObject
    );
}

#[test]
fn the_open_flow_from_raw_host_json_is_refusal_only() {
    // The whole path a host takes: identify, then open. Nothing here can produce a
    // session from input it could not identify.
    let scenario = ScenarioRef::identify(&serde_json::json!({ "format_version": 1, "steps": [] }))
        .expect("identified");
    let fixtures =
        FixtureWorld::identify(serde_json::json!({ "approvals": [] })).expect("identified");

    let inputs = rigorix_gate_wasm::domain::session::OpenInputs {
        scenario,
        fixtures,
        clock: rigorix_gate_wasm::domain::value::ClockTrack::new(vec![
            rigorix_gate_wasm::domain::value::ClockReading {
                instant: rigorix_gate_wasm::domain::value::Timestamp::from_trusted(
                    "2026-11-03T09:00:00Z",
                ),
                index: 0,
            },
        ]),
        seed: rigorix_gate_wasm::domain::value::RandomSeed::new(
            Digest::of(b"seed"),
            vec![rigorix_gate_wasm::domain::value::Draw::from_trusted("00")],
        ),
    };
    let session = ReplaySession::open(inputs).expect("open");
    assert!(session.id().as_str().starts_with("sha256:"));

    // And the same raw JSON, twice, is the same session.
    let again_scenario =
        ScenarioRef::identify(&serde_json::json!({ "format_version": 1, "steps": [] }))
            .expect("identified");
    let again_fixtures =
        FixtureWorld::identify(serde_json::json!({ "approvals": [] })).expect("identified");
    let mut again = rigorix_gate_wasm::domain::session::OpenInputs {
        scenario: again_scenario,
        fixtures: again_fixtures,
        clock: rigorix_gate_wasm::domain::value::ClockTrack::new(vec![
            rigorix_gate_wasm::domain::value::ClockReading {
                instant: rigorix_gate_wasm::domain::value::Timestamp::from_trusted(
                    "2026-11-03T09:00:00Z",
                ),
                index: 0,
            },
        ]),
        seed: rigorix_gate_wasm::domain::value::RandomSeed::new(
            Digest::of(b"seed"),
            vec![rigorix_gate_wasm::domain::value::Draw::from_trusted("00")],
        ),
    };
    let again_session = ReplaySession::open(std::mem::replace(
        &mut again,
        rigorix_gate_wasm::domain::session::OpenInputs {
            scenario: ScenarioRef::new(Digest::of(b"x"), ScenarioFormatVersion(1)),
            fixtures: FixtureWorld::new(Digest::of(b"y"), FixtureTranscript(serde_json::json!({}))),
            clock: rigorix_gate_wasm::domain::value::ClockTrack::default(),
            seed: rigorix_gate_wasm::domain::value::RandomSeed::new(Digest::of(b"z"), vec![]),
        },
    ))
    .expect("open");
    assert_eq!(session.id(), again_session.id());
}
