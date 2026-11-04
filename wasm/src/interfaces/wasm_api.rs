//! The four exported functions — the frozen host contract.
//!
//! @canonical .pi/architecture/decisions/ADR-004-api-design.md
//! @canonical .pi/architecture/decisions/ADR-013-who-verifies.md
//! @canonical .pi/architecture/decisions/ADR-012-failure-semantics-on-wasm32.md
//! Implements: `WasmContract` (AC 15, 16)
//! Issue: #1309 (WasmContract)
//!
//! ```text
//! open_session(scenario, fixtures, clock, seed) -> handle
//! attempt_run(handle, action)                   -> run_outcome
//! export_artifact_set(handle)                   -> artifact_set
//! contract_version()                            -> { contract, engine, artifact_format }
//! ```
//!
//! # Contract
//! - JSON strings in, JSON strings out. The `#[wasm_bindgen]` attribute layer lives in
//!   `mod exported` at the foot of this file (wasm32 only, #1349): these signatures are the
//!   contract, and that module is the binding a host actually calls
//! - Every DTO rejects unknown fields (`deny_unknown_fields`), so a typo is a refusal, not
//!   a silently ignored field (criterion 15)
//! - A refusal or a gate failure is `Ok(json)`. The **only** `Err` is a malformed handle,
//!   because that is a programmer mistake, not a gate decision (criterion 16)
//! - No panic may cross this line (ADR-012): every fallible step is mapped, and `?` is used
//!   only on `Result<_, String>` that the caller receives

use serde::Serialize;

#[cfg(feature = "engine")]
use crate::application::OpenReplaySession;
use crate::application::{
    ArtifactQuery, AttemptRun, AttemptRunService, ExportArtifactSet, ExportArtifactSetService,
    ReplaySessionQuery,
};
#[cfg(feature = "engine")]
use crate::application::{OpenReplaySessionService, OpenSessionInput};
use crate::domain::artifact::ArtifactSet;
use crate::domain::ports::Ports;
use crate::domain::services::{
    ArtifactContentPolicy, ArtifactStamp, BusinessWording, EngineArtifactStamp,
    EngineFindingsProjection, RefusalVocabulary,
};
#[cfg(feature = "engine")]
use crate::domain::value::{
    ClockReading, ClockTrack, Draw, FixtureWorld, GatedAction, Params, RandomSeed, ScenarioRef,
    Timestamp,
};
#[cfg(not(feature = "engine"))]
use crate::domain::value::{GatedAction, Params};
use crate::infrastructure::{
    FixtureClock, FixtureProviderTransport, FixtureRandomness, MemoryBusinessSink,
    ScanningContentPolicy, SeededIdSource, SeededKeySource,
};
use crate::interfaces::dto::{
    ArtifactSetDto, EvidenceRefDto, GatedActionDto, RunOutcomeDto, SessionHandleDto, StepOutcomeDto,
};
#[cfg(feature = "engine")]
use crate::interfaces::dto::{ClockReadingDto, SeedDto};
#[cfg(feature = "engine")]
use crate::interfaces::errors::project_session_open;
use crate::interfaces::errors::{project_attempt, project_handle, project_request, project_seal};
use crate::interfaces::registry;
#[cfg(feature = "engine")]
use crate::interfaces::registry::SessionSlot;

/// The frozen list of exported function names.
///
/// # Contract
/// - Exactly four. A fifth requires an ADR: the fifth was verification, and ADR-013 removed
///   it deliberately
pub const CONTRACT_FUNCTIONS: [&str; 4] = [
    "open_session",
    "attempt_run",
    "export_artifact_set",
    "contract_version",
];

/// The version triple a host checks before it trusts the shape of the JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractVersion {
    /// Semver of this host contract.
    pub contract: String,
    /// Engine version the bundle was built against.
    pub engine: String,
    /// Artifact container format this bundle emits.
    pub artifact_format: u32,
    /// Whether this build can evaluate a gate at all.
    ///
    /// A host learns the build's capability *before* opening a session, rather than
    /// inferring it from a refusal later (#1384). `false` means every `open_session` will
    /// refuse with `gate_unavailable` — the one condition that kind is reserved for.
    pub gate_evaluation: bool,
}

/// Serialise a value for the host. Encoding our own types cannot fail short of a bug, and
/// a bug must not cross the boundary as a panic (ADR-012), so it becomes a refusal string.
fn render<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| {
        r#"{"kind":"request_invalid","message":"the response could not be encoded"}"#.to_string()
    })
}

/// Assemble a replayable session.
/// Assemble a replayable session — `open_session(scenario, fixtures, clock, seed)`.
///
/// # Contract
/// - Four JSON inputs, all required: scenario, fixtures, clock, seed
/// - Returns a handle, or a refusal when the input cannot be pinned
/// - A build without gate evaluation refuses here, and only here: `gate_unavailable` names
///   "this build cannot decide", so it is emitted before any handle exists (#1384). A run
///   that fails to evaluate on a build that *can* decide is an `attempt_failed`.
pub fn open_session(
    scenario_json: String,
    fixtures_json: String,
    clock_json: String,
    seed_json: String,
) -> Result<String, String> {
    #[cfg(not(feature = "engine"))]
    {
        let _ = (scenario_json, fixtures_json, clock_json, seed_json);
        Ok(render(
            &crate::interfaces::errors::project_gate_unavailable(),
        ))
    }
    #[cfg(feature = "engine")]
    {
        open_session_evaluate(scenario_json, fixtures_json, clock_json, seed_json)
    }
}

/// The same call on a build that can evaluate.
#[cfg(feature = "engine")]
fn open_session_evaluate(
    scenario_json: String,
    fixtures_json: String,
    clock_json: String,
    seed_json: String,
) -> Result<String, String> {
    let scenario: serde_json::Value = match serde_json::from_str(&scenario_json) {
        Ok(value) => value,
        Err(_) => return Ok(render(&project_request("scenario"))),
    };
    let scenario = match ScenarioRef::identify(&scenario) {
        Ok(reference) => reference,
        Err(_) => {
            return Ok(render(&project_session_open(
                &crate::domain::errors::SessionOpenError::ScenarioUnparseable,
            )));
        }
    };

    let fixtures: serde_json::Value = match serde_json::from_str(&fixtures_json) {
        Ok(value) => value,
        Err(_) => return Ok(render(&project_request("fixtures"))),
    };
    let fixtures = match FixtureWorld::identify(fixtures) {
        Ok(world) => world,
        Err(_) => {
            return Ok(render(&project_session_open(
                &crate::domain::errors::SessionOpenError::FixtureUnparseable,
            )));
        }
    };

    let clock: Vec<ClockReadingDto> = match serde_json::from_str(&clock_json) {
        Ok(value) => value,
        Err(_) => return Ok(render(&project_request("clock"))),
    };
    let seed: SeedDto = match serde_json::from_str(&seed_json) {
        Ok(value) => value,
        Err(_) => return Ok(render(&project_request("seed"))),
    };

    let clock = ClockTrack::new(
        clock
            .into_iter()
            .map(|reading| ClockReading {
                instant: Timestamp::from_trusted(reading.instant),
                index: reading.index,
            })
            .collect(),
    );

    let seed_digest = match crate::domain::value::Digest::parse(seed.seed_digest) {
        Ok(digest) => digest,
        Err(_) => return Ok(render(&project_request("seed.seed_digest"))),
    };
    let seed = RandomSeed::new(
        seed_digest,
        seed.draws.into_iter().map(Draw::from_trusted).collect(),
    );

    // The gate is built from the same fixture world the session pins: one transcript, one
    // set of port answers, so the engine evaluates the data the record cites.
    #[cfg(feature = "engine")]
    let gate_transcript = fixtures.transcript.0.clone();

    let opener = OpenReplaySessionService::new();
    let session = match opener.execute(OpenSessionInput {
        scenario,
        fixtures,
        clock,
        seed,
    }) {
        Ok(session) => session,
        Err(error) => return Ok(render(&project_session_open(&error))),
    };

    // C.3: the engine's composition, over four host-supplied ports. A build without the
    // slice answers `EngineGate::unavailable()` — still a refusal that says why, never an
    // allow, and the kind distinguishes "could not answer" from "answered no".
    #[cfg(feature = "engine")]
    let gate = Box::new(crate::infrastructure::EngineGate::new(gate_transcript));
    #[cfg(not(feature = "engine"))]
    let gate = Box::new(crate::infrastructure::EngineGate::unavailable());

    let handle = registry::insert(SessionSlot { session, gate });

    Ok(render(&SessionHandleDto { handle }))
}

/// Drive the session's one attempt.
///
/// # Contract
/// - Returns `{ status, steps[], evidence[] }`; `steps` preserves engine order
/// - A refusal is a normal return value
pub fn attempt_run(handle_json: String, action_json: String) -> Result<String, String> {
    let handle: SessionHandleDto = match serde_json::from_str(&handle_json) {
        Ok(value) => value,
        Err(_) => return Ok(render(&project_request("handle"))),
    };
    let action: GatedActionDto = match serde_json::from_str(&action_json) {
        Ok(value) => value,
        Err(_) => return Ok(render(&project_request("action"))),
    };

    let action = GatedAction::new(action.name, Params(action.params));
    let wording = BusinessWording::new();
    let projection = EngineFindingsProjection::new(wording);

    registry::with(&handle.handle, |slot| {
        // The ports are rebuilt per attempt from the session's pinned transcript: a port
        // that kept state across attempts would make the second attempt differ.
        let mut clock = FixtureClock::new(slot.session.inputs().clock.clone());
        let mut ids = ids_from_transcript(slot.session.fixtures().transcript.0.clone());
        let mut random = FixtureRandomness::new(slot.session.inputs().seed.clone());
        let key = SeededKeySource::new(crate::domain::value::KeyRef::from_trusted("demo:key"));
        let mut provider =
            FixtureProviderTransport::new(slot.session.fixtures().transcript.0.clone());
        let mut sink = MemoryBusinessSink::new();

        let ports = Ports {
            clock: &mut clock,
            ids: &mut ids,
            randomness: &mut random,
            key_source: &key,
            provider: &mut provider,
            sink: &mut sink,
        };

        let runner = AttemptRunService::new(projection);
        match runner.execute(&mut slot.session, action, ports, slot.gate.as_mut()) {
            Ok(outcome) => render(&RunOutcomeDto::from(&outcome)),
            Err(error) => render(&project_attempt(&error)),
        }
    })
    .map_err(|error| render(&project_handle(&error)))
}

/// Seal and export the artifact set.
///
/// # Contract
/// - Returns the stamped container; the host verifies it with `rigorix-verifier`
/// - A content-rule violation is a refusal, and the bytes do not leave
pub fn export_artifact_set(handle_json: String) -> Result<String, String> {
    let handle: SessionHandleDto = match serde_json::from_str(&handle_json) {
        Ok(value) => value,
        Err(_) => return Ok(render(&project_request("handle"))),
    };

    registry::with(&handle.handle, |slot| {
        let stamp = EngineArtifactStamp::new();
        let policy = ScanningContentPolicy::new();
        let exporter = ExportArtifactSetService::new(stamp, policy);
        // Stamps are `dyn`-safe but the service wants sized types; the concrete ones here
        // are unit structs, so this borrows nothing from the host.
        let _: Option<&dyn ArtifactStamp> = Some(&stamp);
        let _: Option<&dyn ArtifactContentPolicy> = Some(&policy);
        match exporter.execute(&mut slot.session) {
            Ok(artifact) => render(&ArtifactSetDto::from(&artifact)),
            Err(error) => render(&project_seal(&error)),
        }
    })
    .map_err(|error| render(&project_handle(&error)))
}

/// Report the version numbers and the build's gate capability.
pub fn contract_version() -> String {
    render(&ContractVersion {
        contract: env!("CARGO_PKG_VERSION").to_string(),
        engine: EngineArtifactStamp::engine_version().to_string(),
        artifact_format: ArtifactSet::FORMAT_VERSION,
        gate_evaluation: cfg!(feature = "engine"),
    })
}

// --- projections to the wire ------------------------------------------------

impl From<&crate::domain::outcome::RunOutcome> for RunOutcomeDto {
    fn from(outcome: &crate::domain::outcome::RunOutcome) -> Self {
        Self {
            status: match outcome.status {
                crate::domain::outcome::OutcomeStatus::Allowed => "allowed",
                crate::domain::outcome::OutcomeStatus::Refused => "refused",
            }
            .to_string(),
            steps: outcome.steps.iter().map(StepOutcomeDto::from).collect(),
            evidence: outcome.evidence.iter().map(EvidenceRefDto::from).collect(),
        }
    }
}

impl From<&crate::domain::outcome::StepOutcome> for StepOutcomeDto {
    fn from(step: &crate::domain::outcome::StepOutcome) -> Self {
        Self {
            gate_id: step.gate_id.clone(),
            decision: match step.decision {
                crate::domain::outcome::StepDecision::Allowed => "allowed",
                crate::domain::outcome::StepDecision::Promoted => "promoted",
                crate::domain::outcome::StepDecision::Refused => "refused",
                crate::domain::outcome::StepDecision::NotReached => "not_reached",
            }
            .to_string(),
            kind: step.kind.map(|kind| format!("{kind:?}").to_lowercase()),
            reason: step.reason.clone(),
            evidence: step.evidence.as_ref().map(EvidenceRefDto::from),
        }
    }
}

impl From<&crate::domain::outcome::EvidenceRef> for EvidenceRefDto {
    fn from(evidence: &crate::domain::outcome::EvidenceRef) -> Self {
        Self {
            digest: evidence.digest.as_str().to_string(),
            kind: match evidence.kind {
                crate::domain::outcome::EvidenceKind::Precondition => "precondition",
                crate::domain::outcome::EvidenceKind::SequencePolicy => "sequence_policy",
                crate::domain::outcome::EvidenceKind::Requirement => "requirement",
                crate::domain::outcome::EvidenceKind::Approval => "approval",
                crate::domain::outcome::EvidenceKind::Identity => "identity",
                crate::domain::outcome::EvidenceKind::PortFailure => "port_failure",
            }
            .to_string(),
        }
    }
}

impl From<&ArtifactSet> for ArtifactSetDto {
    fn from(artifact: &ArtifactSet) -> Self {
        Self {
            artifact_format_version: artifact.stamp.artifact_format_version,
            engine_version: artifact.stamp.engine_version.clone(),
            scenario_digest: artifact.stamp.scenario_digest.as_str().to_string(),
            fixture_digest: artifact.fixture_digest.as_str().to_string(),
            session_id: artifact.session_id.as_str().to_string(),
            key_ref: artifact.key_ref.as_str().to_string(),
            envelopes: artifact
                .envelopes
                .iter()
                .map(|envelope| String::from_utf8_lossy(&envelope.0).into_owned())
                .collect(),
            evidence: artifact.evidence.iter().map(EvidenceRefDto::from).collect(),
            outcome_digest: artifact.outcome_digest.as_str().to_string(),
        }
    }
}

/// Build the id sequences from the fixture transcript.
///
/// The transcript supplies them as data:
/// `{ "ids": { "attempt": ["a-0"], "step": ["s-0"], "envelope": ["e-0"] } }`. A transcript
/// with no `ids` yields empty sequences, so a missing sequence is `PortFailure::Missing` at
/// the moment it is needed — refused, never fabricated (invariant 5).
fn ids_from_transcript(transcript: serde_json::Value) -> SeededIdSource {
    use crate::domain::ports::IdKind;
    use std::collections::HashMap;

    let mut sequences: HashMap<IdKind, Vec<String>> = HashMap::new();
    let Some(ids) = transcript.get("ids").and_then(|value| value.as_object()) else {
        return SeededIdSource::new(sequences);
    };

    for (key, kind) in [
        ("attempt", IdKind::Attempt),
        ("step", IdKind::Step),
        ("envelope", IdKind::Envelope),
    ] {
        if let Some(values) = ids.get(key).and_then(|value| value.as_array()) {
            sequences.insert(
                kind,
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_string))
                    .collect(),
            );
        }
    }
    SeededIdSource::new(sequences)
}

/// The read helper, exposed for the host's polling UI.
pub fn artifact_query() -> &'static dyn ArtifactQuery {
    &ReplaySessionQueryHolder::QUERY
}

/// Keeps a `'static` query value without a `static mut`.
struct ReplaySessionQueryHolder;

impl ReplaySessionQueryHolder {
    const QUERY: ReplaySessionQuery = ReplaySessionQuery::new();
}

/// Silence the unused-import lint for the vocabulary trait: it is used through the
/// projection type, and naming it here documents why the import exists.
const _: fn(&BusinessWording, crate::domain::outcome::RefusalKind) -> &'static str =
    RefusalVocabulary::reason;

// ── The host-facing export layer (#1349) ─────────────────────────────────────
//
// `cargo build --target wasm32-unknown-unknown` produced a 368-byte module with
// no exported symbol: the functions above are the contract, but nothing bound
// them to a name a host could call, so every gate passed on a module that did
// nothing. This module is that binding — thin wrappers with the same signatures,
// mapping the one `Err` (a malformed handle: a programmer mistake, not a gate
// decision) to a JS exception and everything else to JSON as before.
//
// Compiled only for wasm32: the crate's native API and its tests are unchanged,
// and the native dependency graph stays empty of wasm-bindgen.
#[cfg(target_arch = "wasm32")]
pub mod exported {
    use wasm_bindgen::prelude::*;

    /// Assemble a replayable session — `open_session(scenario, fixtures, clock, seed)`.
    ///
    /// Returns a handle, or a refusal as a return value.
    #[wasm_bindgen]
    pub fn open_session(
        scenario_json: String,
        fixtures_json: String,
        clock_json: String,
        seed_json: String,
    ) -> Result<String, JsValue> {
        super::open_session(scenario_json, fixtures_json, clock_json, seed_json)
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Evaluate a proposed action inside the session — `attempt_run(handle, action)`.
    ///
    /// A refusal is `Ok(json)`. The only exception is a malformed handle.
    #[wasm_bindgen]
    pub fn attempt_run(handle_json: String, action_json: String) -> Result<String, JsValue> {
        super::attempt_run(handle_json, action_json).map_err(|e| JsValue::from_str(&e))
    }

    /// Export the session's artifact set — `export_artifact_set(handle)`.
    ///
    /// Before an attempt this is a refusal (`seal_failed`), not an error.
    #[wasm_bindgen]
    pub fn export_artifact_set(handle_json: String) -> Result<String, JsValue> {
        super::export_artifact_set(handle_json).map_err(|e| JsValue::from_str(&e))
    }

    /// The three version numbers — `contract_version()`.
    #[wasm_bindgen]
    pub fn contract_version() -> String {
        super::contract_version()
    }
}
