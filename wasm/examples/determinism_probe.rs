//! Determinism probe — replay a committed vector and print the artifact bytes.
//!
//! @canonical .pi/architecture/decisions/ADR-009-conformance-harness.md
//! Implements: the cross-instance half of criterion 25
//! Issue: #1316 (Determinism)
//!
//! Why a separate process: "the same vector replayed in a freshly instantiated module"
//! cannot be tested from inside one process. This probe is the fresh instance — a new
//! address space, new statics, new thread-locals. The test compares its output to an
//! in-process replay of the same vector; if any state leaked, the bytes differ.
//!
//! The gate is a constant (`FixedGate`), on purpose: if the same gate produces different
//! bytes in a fresh process, the difference came from the crate rather than from a decision.

use rigorix_gate_wasm::application::{
    AttemptRun, AttemptRunService, ExportArtifactSet, ExportArtifactSetService, OpenReplaySession,
    OpenReplaySessionService, OpenSessionInput, default_projection,
};
use rigorix_gate_wasm::domain::engine_seam::{
    EngineBindingError, EngineEvaluation, EngineFinding, EngineFindingOutcome,
    GateEvaluationRequest, GateEvaluator,
};
use rigorix_gate_wasm::domain::ports::{IdKind, Ports};
use rigorix_gate_wasm::domain::value::{
    ClockReading, Draw, FixtureWorld, GatedAction, Params, RandomSeed, ScenarioRef, Timestamp,
};
use rigorix_gate_wasm::infrastructure::{
    FixtureClock, FixtureProviderTransport, FixtureRandomness, MemoryBusinessSink,
    ScanningContentPolicy, SeededIdSource, SeededKeySource,
};

/// A fixed gate: one passed finding and one fixed envelope.
///
/// The probe's subject is determinism, not policy — so the gate is a constant. A constant
/// gate is also the strictest test of the claim: if the *same* gate produces different bytes
/// across processes, the difference came from the crate, not from the decision.
struct FixedGate;

impl GateEvaluator for FixedGate {
    fn evaluate(
        &mut self,
        request: GateEvaluationRequest<'_, '_>,
    ) -> Result<EngineEvaluation, EngineBindingError> {
        let _ = (request.attempt_id, request.scenario, request.action);
        Ok(EngineEvaluation {
            findings: vec![EngineFinding {
                gate_id: "precondition.beneficiary-eligible".to_string(),
                outcome: EngineFindingOutcome::Passed,
                refusal: None,
                evidence: None,
                opaque: None,
            }],
            envelopes: vec![br#"{"envelope":1}"#.to_vec()],
            port_failures: Vec::new(),
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let vector = std::env::args()
        .nth(1)
        .expect("usage: determinism_probe <vector.json>");
    let raw = std::fs::read_to_string(&vector)?;
    let vector: rigorix_gate_wasm::application::ConformanceVector = serde_json::from_str(&raw)?;

    let scenario = ScenarioRef::identify(&vector.scenario)?;
    let fixtures = FixtureWorld::identify(vector.fixtures.clone())?;
    let readings: Vec<serde_json::Value> = serde_json::from_value(vector.clock.clone())?;
    let clock = rigorix_gate_wasm::domain::value::ClockTrack::new(
        readings
            .into_iter()
            .map(|reading| ClockReading {
                instant: Timestamp::from_trusted(
                    reading["instant"].as_str().unwrap_or_default().to_string(),
                ),
                index: reading["index"].as_u64().unwrap_or(0) as u32,
            })
            .collect(),
    );
    let seed_digest = rigorix_gate_wasm::domain::value::Digest::parse(
        vector.seed["seed_digest"].as_str().unwrap_or_default(),
    )?;
    let seed = RandomSeed::new(
        seed_digest,
        vector.seed["draws"]
            .as_array()
            .map(|draws| {
                draws
                    .iter()
                    .filter_map(|draw| draw.as_str().map(Draw::from_trusted))
                    .collect()
            })
            .unwrap_or_default(),
    );

    let mut session = OpenReplaySessionService::new().execute(OpenSessionInput {
        scenario,
        fixtures,
        clock,
        seed,
    })?;

    let mut clock = FixtureClock::new(session.inputs().clock.clone());
    let mut ids = {
        let mut sequences = std::collections::HashMap::new();
        sequences.insert(IdKind::Attempt, vec!["attempt-0".to_string()]);
        SeededIdSource::new(sequences)
    };
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

    let action = GatedAction::new("payment_execute", Params::default());
    let mut gate = FixedGate;
    let runner = AttemptRunService::new(default_projection());
    // A refusal is still a record. The probe keeps going on a gate failure on purpose: the
    // determinism claim must hold for refusals too, and a refusal is the only outcome a
    // build without the engine seam can produce.
    runner.execute(&mut session, action, ports, &mut gate)?;

    let artifact = ExportArtifactSetService::new(
        rigorix_gate_wasm::domain::services::EngineArtifactStamp::new(),
        ScanningContentPolicy::new(),
    )
    .execute(&mut session)?;

    // Print the **wire** shape, not the domain struct: the host verifier reads the DTO, and a
    // probe that printed `{"stamp":{…}}` would make the independence check fail on a field
    // name rather than on the record (found the hard way, in CI).
    let wire = rigorix_gate_wasm::interfaces::dto::ArtifactSetDto::from(&artifact);
    // The probe must not print anything else: the test compares the whole of stdout.
    println!("{}", String::from_utf8(serde_json::to_vec(&wire)?)?);
    Ok(())
}
