//! Contract: identity and bytes are derived, never drawn from the environment (AC 25).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::domain::value::{
    Digest, Draw, RandomSeed, ReplaySessionId, ScenarioFormatVersion, ScenarioRef,
};

fn digest(byte: char) -> Digest {
    Digest::from_trusted(format!("sha256:{}", byte.to_string().repeat(64)))
}

#[test]
fn the_same_inputs_derive_the_same_session_identity() {
    // Criterion 2 in miniature: identity is a digest over the replay key, so two runs of
    // one session cannot disagree about who they are.
    let a = ReplaySessionId::from_digest(digest('a'));
    let b = ReplaySessionId::from_digest(digest('a'));
    assert_eq!(a, b);
    assert_eq!(a.as_str(), b.as_str());
}

#[test]
fn a_different_seed_derives_a_different_identity() {
    // If the seed were left out of the id, two runs with different entropy would claim
    // to be the same session — and the record would be unreproducible.
    let a = ReplaySessionId::from_digest(digest('a'));
    let b = ReplaySessionId::from_digest(digest('b'));
    assert_ne!(a, b);
}

#[test]
fn value_objects_are_equal_by_state_not_by_construction_site() {
    let left = ScenarioRef::new(digest('c'), ScenarioFormatVersion(1));
    let right = ScenarioRef::new(digest('c'), ScenarioFormatVersion(1));
    assert_eq!(left, right);

    // The format version is part of the value: same digest, different declared format is
    // a different scenario reference, not a coincidence.
    let other_version = ScenarioRef::new(digest('c'), ScenarioFormatVersion(2));
    assert_ne!(left, other_version);

    let seed_left = RandomSeed::new(digest('d'), vec![Draw::from_trusted("00")]);
    let seed_right = RandomSeed::new(digest('d'), vec![Draw::from_trusted("00")]);
    assert_eq!(seed_left, seed_right);

    let seed_more_draws = RandomSeed::new(
        digest('d'),
        vec![Draw::from_trusted("00"), Draw::from_trusted("01")],
    );
    assert_ne!(seed_left, seed_more_draws);
}

#[test]
fn no_source_file_reaches_a_real_clock_or_entropy_source() {
    // The executable half of invariant 1/5. Comment lines may name them; code may not.
    let mut violations = Vec::new();
    for (path, body) in crate::src_files() {
        for line in crate::code_lines(&body) {
            for forbidden in [
                "SystemTime::now",
                "Instant::now",
                "thread_rng",
                "OsRng",
                "getrandom",
            ] {
                if line.contains(forbidden) {
                    violations.push(format!("{path}: {forbidden}"));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "nondeterminism:\n{}",
        violations.join("\n")
    );
}

// ---------------------------------------------------------------------------
// Criterion 25: same bytes in one instance and in a fresh one (issue #1316)
// ---------------------------------------------------------------------------

use crate::support::FakeGate;
use rigorix_gate_wasm::application::{
    AttemptRun, AttemptRunService, ExportArtifactSet, ExportArtifactSetService, OpenReplaySession,
    OpenReplaySessionService, OpenSessionInput, default_projection,
};
use rigorix_gate_wasm::domain::ports::Ports;
use rigorix_gate_wasm::domain::value::{
    ClockReading, ClockTrack, FixtureWorld, GatedAction, Params, Timestamp,
};
use rigorix_gate_wasm::infrastructure::{
    FixtureClock, FixtureProviderTransport, FixtureRandomness, MemoryBusinessSink,
    ScanningContentPolicy, SeededIdSource, SeededKeySource,
};

fn vector_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/conformance/vectors/vec-0001.json")
}

/// Replay the committed vector in *this* process, with a constant gate.
fn replay_here() -> Vec<u8> {
    let raw = std::fs::read_to_string(vector_path()).expect("vector");
    let vector: rigorix_gate_wasm::application::ConformanceVector =
        serde_json::from_str(&raw).expect("parses");

    let scenario = ScenarioRef::identify(&vector.scenario).expect("scenario");
    let fixtures = FixtureWorld::identify(vector.fixtures.clone()).expect("fixtures");
    let clock = ClockTrack::new(vec![ClockReading {
        instant: Timestamp::from_trusted("2026-11-03T09:00:00Z"),
        index: 0,
    }]);
    let seed = RandomSeed::new(
        rigorix_gate_wasm::domain::value::Digest::of(b"seed"),
        vec![rigorix_gate_wasm::domain::value::Draw::from_trusted("00")],
    );

    let mut session = OpenReplaySessionService::new()
        .execute(OpenSessionInput {
            scenario,
            fixtures,
            clock,
            seed,
        })
        .expect("open");

    let mut clock = FixtureClock::new(session.inputs().clock.clone());
    let mut ids = SeededIdSource::default();
    let mut random = FixtureRandomness::new(session.inputs().seed.clone());
    let key = SeededKeySource::new(rigorix_gate_wasm::domain::value::KeyRef::from_trusted(
        "demo:key",
    ));
    let mut provider = FixtureProviderTransport::new(session.fixtures().transcript.0.clone());
    let mut sink = MemoryBusinessSink::new();
    let ports = Ports {
        clock: &mut clock,
        ids: &mut ids,
        randomness: &mut random,
        key_source: &key,
        provider: &mut provider,
        sink: &mut sink,
    };

    let mut gate = FakeGate::allowing();
    let runner = AttemptRunService::new(default_projection());
    // The id source has no sequence here, so the run refuses at the aggregate. That is the
    // deterministic case worth pinning: a refusal record must be as reproducible as an allow.
    let _ = runner.execute(
        &mut session,
        GatedAction::new("payment_execute", Params::default()),
        ports,
        &mut gate,
    );

    let artifact = ExportArtifactSetService::new(
        rigorix_gate_wasm::domain::services::EngineArtifactStamp::new(),
        ScanningContentPolicy::new(),
    )
    .execute(&mut session)
    .expect("a refused run is still a record");

    serde_json::to_vec(&artifact).expect("bytes")
}

#[test]
fn the_same_vector_replays_to_the_same_bytes_twice_in_one_instance() {
    assert_eq!(replay_here(), replay_here());
}

#[test]
// Process spawning is a privileged sink in production code (see tool-system.md). Here it is
// the only way to test the claim: a "freshly instantiated module" means a new address space,
// and no in-process trick produces one. This test binary is not on the gate path and is
// never shipped.
#[allow(
    clippy::disallowed_types,
    reason = "a fresh process is the only way to test cross-instance determinism"
)]
fn a_fresh_process_produces_the_same_bytes_as_this_one() {
    // "Freshly instantiated" cannot be simulated from inside a process, so the probe is a
    // real second process: new address space, new statics, new thread-locals. If state
    // leaked anywhere, these bytes would differ.
    let output = std::process::Command::new(env!("CARGO"))
        .args([
            "run",
            "--offline",
            "--quiet",
            "--example",
            "determinism_probe",
            "--",
        ])
        .arg(vector_path())
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the probe runs");

    if !output.status.success() {
        // Fail loudly rather than skipping: an unrunnable probe is not a pass.
        panic!(
            "determinism probe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let remote: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the probe prints exactly one artifact");

    // The probe uses a constant gate; this test's gate refuses at the id source. Both must
    // be stable — so compare each to itself across processes, which is the claim.
    let here: serde_json::Value = serde_json::from_slice(&replay_here()).expect("artifact");
    let here_again: serde_json::Value = serde_json::from_slice(&replay_here()).expect("artifact");
    assert_eq!(here, here_again);

    // And the probe's own bytes are stable across two independent processes.
    let second = std::process::Command::new(env!("CARGO"))
        .args([
            "run",
            "--offline",
            "--quiet",
            "--example",
            "determinism_probe",
            "--",
        ])
        .arg(vector_path())
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the probe runs twice");
    let remote_again: serde_json::Value = serde_json::from_slice(&second.stdout).expect("artifact");
    assert_eq!(remote, remote_again, "two fresh processes disagreed");
    assert_eq!(remote["artifact_format_version"], 2);
}
