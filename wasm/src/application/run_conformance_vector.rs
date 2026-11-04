//! `RunConformanceVector` — the proof obligation (FR-007, FR-008, FR-012).
//!
//! @canonical .pi/architecture/decisions/ADR-009-conformance-harness.md
//! Implements: Contract Freeze — RunConformanceVector, ConformanceVector, ConformanceReport
//! Issue: #1294 (contract freeze)
//!
//! # Contract (Frozen)
//! - Test-only: gated behind `cfg(any(test, feature = "conformance"))` and **never**
//!   part of the bundle (NFR-002)
//! - Comparison is **byte-exact**, and nothing may normalise it. The report's
//!   `normalised_comparison` field is retained only so the harness can assert it is
//!   `false`: the label-and-continue shape it once described is **refused** (ADR-014,
//!   #1367), because a comparison that normalises a field compares that field, not the
//!   artifacts.
//! - A skipped or missing vector is a failure, never a pass (criterion 20)
//! - `SeededDivergence` must diverge; equality is a build failure (criterion 21)

use serde::{Deserialize, Serialize};

/// One committed vector: inputs plus the bytes they must produce.
///
/// # Contract (Frozen)
/// - Immutable once committed; changing a vector is a reviewed artifact
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConformanceVector {
    /// Stable vector id, used in failure messages.
    pub id: String,
    /// Scenario JSON, in the engine's format.
    pub scenario: serde_json::Value,
    /// Fixture transcript JSON.
    pub fixtures: serde_json::Value,
    /// Clock readings JSON.
    pub clock: serde_json::Value,
    /// Seed JSON.
    pub seed: serde_json::Value,
    /// Expected artifact set bytes.
    pub expected: Vec<u8>,
    /// Whether this vector is the deliberately-diverging one (FR-008).
    pub seeded_divergence: bool,
    /// Where the expected bytes came from: `cli` once FR-011 lands, `fixture` while the
    /// comparison is a self-consistency check.
    ///
    /// A vector must carry its own provenance: an unlabelled golden is a claim nobody can
    /// audit, and the difference between "matches the CLI" and "matches itself" is the
    /// whole value of the harness (ADR-009).
    #[serde(default)]
    pub gate: Option<String>,
}

/// What a replay concluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceReport {
    /// Vector id.
    pub vector_id: String,
    /// Whether the bytes matched.
    pub matched: bool,
    /// Whether the comparison was normalised rather than byte-exact. **Always `false`:**
    /// a normalised comparison is refused (ADR-014, #1367), and the field survives only so
    /// this assertion has somewhere to land.
    pub normalised_comparison: bool,
    /// Whether the vector was skipped. A skip is a failure.
    pub skipped: bool,
}

impl ConformanceReport {
    /// Whether this report satisfies the contract for its vector.
    ///
    /// A seeded-divergence vector passes only when it **did not** match; every other
    /// vector passes only when it did. A skip never passes.
    pub fn is_pass(&self, seeded_divergence: bool) -> bool {
        !self.skipped && self.matched != seeded_divergence
    }
}

/// Replays vectors and compares bytes.
pub trait RunConformanceVector {
    /// Replay one vector.
    fn execute(&self, vector: &ConformanceVector) -> ConformanceReport;
}

// --- the runner -------------------------------------------------------------
//
// Issue: #1312 (RunConformanceVector)
//
// What this does today: builds a session from the vector's four inputs, runs one attempt
// with an **injected** gate, exports the artifact set and compares the canonical bytes to
// the vector's `expected`. What it deliberately does not do: pretend that comparison is CLI
// conformance. The bytes are this side's, so every vector carries a `gate` label — and the
// label is the only interim allowed. `normalised_comparison` is not: a difference is a
// finding, and the one it would absorb (884 B unpinned vs 877 B pinned) is the finding.
//
// The plumbing is the deliverable here: when the engine lands, only the gate factory
// changes.

use crate::application::{
    AttemptRun, AttemptRunService, ExportArtifactSet, ExportArtifactSetService, OpenReplaySession,
    OpenReplaySessionService, OpenSessionInput,
};
use crate::domain::engine_seam::GateEvaluator;
use crate::domain::ports::{IdKind, Ports};
use crate::domain::services::{
    BusinessWording, EngineArtifactStamp, EngineFindingsProjection, EvidenceProjection,
};
use crate::domain::value::{
    ClockReading, ClockTrack, Draw, FixtureWorld, GatedAction, Params, RandomSeed, ScenarioRef,
    Timestamp,
};
use crate::infrastructure::{
    FixtureClock, FixtureProviderTransport, FixtureRandomness, MemoryBusinessSink,
    ScanningContentPolicy, SeededIdSource, SeededKeySource,
};

/// The runner: one vector in, one report out.
pub struct ConformanceRunner<G, M>
where
    G: FnMut() -> Box<dyn GateEvaluator>,
    M: Fn() -> Box<dyn EvidenceProjection>,
{
    /// Builds a fresh gate per replay, so no state leaks between vectors.
    ///
    /// Behind a `RefCell` because `execute` is `&self` (it is a read-shaped operation over
    /// a committed vector) while a gate is stateful by nature. A failed borrow refuses
    /// rather than panicking: a panic here would trap the module (ADR-012).
    pub gate: std::cell::RefCell<G>,
    /// Builds the projection (wording and ordering policy).
    pub projection: M,
}

impl<G, M> ConformanceRunner<G, M>
where
    G: FnMut() -> Box<dyn GateEvaluator>,
    M: Fn() -> Box<dyn EvidenceProjection>,
{
    /// Construct a runner.
    pub fn new(gate: G, projection: M) -> Self {
        Self {
            gate: std::cell::RefCell::new(gate),
            projection,
        }
    }
}

impl<G, M> RunConformanceVector for ConformanceRunner<G, M>
where
    G: FnMut() -> Box<dyn GateEvaluator>,
    M: Fn() -> Box<dyn EvidenceProjection>,
{
    fn execute(&self, vector: &ConformanceVector) -> ConformanceReport {
        // A vector that cannot be replayed is a failure, not a skip: silence would read as
        // success (criterion 20).
        let Ok(artifact) = self.replay(vector) else {
            return ConformanceReport {
                vector_id: vector.id.clone(),
                matched: false,
                normalised_comparison: false,
                skipped: true,
            };
        };

        let produced = serde_json::to_vec(&artifact).unwrap_or_default();
        ConformanceReport {
            vector_id: vector.id.clone(),
            matched: produced == vector.expected,
            // Always false: a normalised comparison would be a claim we cannot make.
            normalised_comparison: false,
            skipped: false,
        }
    }
}

impl<G, M> ConformanceRunner<G, M>
where
    G: FnMut() -> Box<dyn GateEvaluator>,
    M: Fn() -> Box<dyn EvidenceProjection>,
{
    /// Replay a vector into an artifact set.
    pub fn replay(
        &self,
        vector: &ConformanceVector,
    ) -> Result<crate::domain::artifact::ArtifactSet, String> {
        let scenario = ScenarioRef::identify(&vector.scenario).map_err(|e| e.to_string())?;
        let fixtures =
            FixtureWorld::identify(vector.fixtures.clone()).map_err(|e| e.to_string())?;

        let readings: Vec<ClockReadingDto> = serde_json::from_value(vector.clock.clone())
            .map_err(|_| "clock vector is malformed".to_string())?;
        let clock = ClockTrack::new(
            readings
                .into_iter()
                .map(|reading| ClockReading {
                    instant: Timestamp::from_trusted(reading.instant),
                    index: reading.index,
                })
                .collect(),
        );

        let seed_dto: SeedDto = serde_json::from_value(vector.seed.clone())
            .map_err(|_| "seed vector is malformed".to_string())?;
        let seed_digest = crate::domain::value::Digest::parse(seed_dto.seed_digest)
            .map_err(|_| "seed digest is malformed".to_string())?;
        let seed = RandomSeed::new(
            seed_digest,
            seed_dto.draws.into_iter().map(Draw::from_trusted).collect(),
        );
        let transcript = vector.fixtures.clone();

        let mut session = OpenReplaySessionService::new()
            .execute(OpenSessionInput {
                scenario,
                fixtures,
                clock,
                seed,
            })
            .map_err(|e| e.to_string())?;

        // Ports from the transcript, exactly as the host contract builds them.
        let mut clock = FixtureClock::new(session.inputs().clock.clone());
        let mut ids = id_sequences(&transcript);
        let mut random = FixtureRandomness::new(session.inputs().seed.clone());
        let key = SeededKeySource::new(crate::domain::value::KeyRef::from_trusted("demo:key"));
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

        // The action shape is declared in this layer rather than imported from the outer
        // one: an application-to-outer-layer import is a layering violation, and the
        // architecture conformance gate checks for it.
        let action: VectorAction =
            serde_json::from_value(vector.scenario.get("action").cloned().unwrap_or(
                serde_json::json!({
                    "name": "vector.action",
                    "params": {}
                }),
            ))
            .map_err(|_| "vector action is malformed".to_string())?;
        let action = GatedAction::new(action.name, Params(action.params));

        let runner = AttemptRunService::new((self.projection)());
        let mut gate = match self.gate.try_borrow_mut() {
            Ok(mut factory) => factory(),
            Err(_) => return Err("the gate factory is busy".to_string()),
        };
        runner
            .execute(&mut session, action, ports, gate.as_mut())
            .map_err(|e| e.to_string())?;

        let exporter =
            ExportArtifactSetService::new(EngineArtifactStamp::new(), ScanningContentPolicy::new());
        exporter.execute(&mut session).map_err(|e| e.to_string())
    }
}

/// The action a vector proposes, as the application layer sees it.
#[derive(Debug, Clone, serde::Deserialize)]
struct VectorAction {
    name: String,
    #[serde(default)]
    params: serde_json::Map<String, serde_json::Value>,
}

/// The wire shapes a vector carries, mirrored from the interface layer.
#[derive(Debug, Clone, serde::Deserialize)]
struct ClockReadingDto {
    instant: String,
    index: u32,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct SeedDto {
    seed_digest: String,
    draws: Vec<String>,
}

/// Read the id sequences from a transcript, the same way the host contract does.
fn id_sequences(transcript: &serde_json::Value) -> SeededIdSource {
    use std::collections::HashMap;
    let mut sequences: HashMap<IdKind, Vec<String>> = HashMap::new();
    if let Some(ids) = transcript.get("ids").and_then(|value| value.as_object()) {
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
    }
    SeededIdSource::new(sequences)
}

/// The default projection a vector replay uses.
pub fn default_projection() -> Box<dyn EvidenceProjection> {
    Box::new(EngineFindingsProjection::new(BusinessWording::new()))
}
