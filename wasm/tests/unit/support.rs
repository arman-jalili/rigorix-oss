//! Test doubles for the capability ports and the engine gate.
//!
//! Issue: #1296 (domain layer)
//!
//! These are deliberately dumb: a fake that guesses is a fake that hides the defect it
//! exists to expose. Each one answers exactly once per call and fails the way its name
//! says it fails.

use std::collections::HashMap;

use rigorix_gate_wasm::domain::engine_seam::{
    EngineBindingError, EngineEvaluation, EngineFinding, EngineFindingOutcome,
    GateEvaluationRequest, GateEvaluator,
};
use rigorix_gate_wasm::domain::outcome::EvidenceRef;
use rigorix_gate_wasm::domain::ports::{
    BusinessSink, Clock, Determination, IdKind, IdSource, KeySource, Ports, ProviderCall,
    ProviderObservation, ProviderTransport, Randomness, SinkWrite,
};
use rigorix_gate_wasm::domain::value::{
    ClockReading, ClockTrack, Digest, Draw, KeyRef, PortFailure, PortFailureKind, PortId,
    RandomSeed, Timestamp,
};

pub fn digest(byte: char) -> Digest {
    Digest::from_trusted(format!("sha256:{}", byte.to_string().repeat(64)))
}

// --- ports -----------------------------------------------------------------

pub struct OkClock(pub ClockTrack);
impl OkClock {
    pub fn new() -> Self {
        Self(ClockTrack::new(vec![ClockReading {
            instant: Timestamp::from_trusted("2026-11-03T09:00:00Z"),
            index: 0,
        }]))
    }
}
impl Clock for OkClock {
    fn advance(&mut self) -> Result<ClockReading, PortFailure> {
        self.0
            .readings()
            .first()
            .cloned()
            .ok_or(PortFailure::new(PortId::Clock, PortFailureKind::Exhausted))
    }
}

pub struct OkIds;
impl IdSource for OkIds {
    fn next_id(&mut self, _kind: IdKind) -> Result<String, PortFailure> {
        Ok("attempt-0".to_string())
    }
}

/// An id source that cannot mint — the fault case for the aggregate's own port call.
pub struct FailingIds(pub PortFailureKind);
impl IdSource for FailingIds {
    fn next_id(&mut self, _kind: IdKind) -> Result<String, PortFailure> {
        Err(PortFailure::new(PortId::IdSource, self.0))
    }
}

/// Either a working or a failing id source.
pub enum AnyIds {
    Ok(OkIds),
    Failing(FailingIds),
}

impl IdSource for AnyIds {
    fn next_id(&mut self, kind: IdKind) -> Result<String, PortFailure> {
        match self {
            Self::Ok(source) => source.next_id(kind),
            Self::Failing(source) => source.next_id(kind),
        }
    }
}

pub struct OkRandom;
impl Randomness for OkRandom {
    fn draw(&mut self) -> Result<Draw, PortFailure> {
        Ok(Draw::from_trusted("00"))
    }
}

pub struct OkKey;
impl KeySource for OkKey {
    fn key_ref(&self) -> Result<KeyRef, PortFailure> {
        Ok(KeyRef::from_trusted("demo:sha256:abcd"))
    }
}

/// A key source that cannot resolve a key — the identity-port fault case.
pub struct FailingKey(pub PortFailureKind);
impl KeySource for FailingKey {
    fn key_ref(&self) -> Result<KeyRef, PortFailure> {
        Err(PortFailure::new(PortId::KeySource, self.0))
    }
}

pub struct OkProvider;
impl ProviderTransport for OkProvider {
    fn call(&mut self, _call: &ProviderCall) -> Result<ProviderObservation, PortFailure> {
        Ok(ProviderObservation {
            determination: Determination::Passed,
            payload: None,
        })
    }
}

pub struct OkSink {
    pub writes: Vec<SinkWrite>,
}
impl OkSink {
    pub fn new() -> Self {
        Self { writes: Vec::new() }
    }
}
impl BusinessSink for OkSink {
    fn record(&mut self, write: &SinkWrite) -> Result<(), PortFailure> {
        self.writes.push(write.clone());
        Ok(())
    }
}

/// Bundles the happy-path ports plus every port needed to build `Ports`.
/// Either a working or a failing key source, so a test can fault the identity port
/// without duplicating the whole bundle.
pub enum AnyKey {
    Ok(OkKey),
    Failing(FailingKey),
}

impl KeySource for AnyKey {
    fn key_ref(&self) -> Result<KeyRef, PortFailure> {
        match self {
            Self::Ok(source) => source.key_ref(),
            Self::Failing(source) => source.key_ref(),
        }
    }
}

impl FailingKey {
    /// Lift a failing key into the bundle's key slot.
    pub fn into_key(self) -> AnyKey {
        AnyKey::Failing(self)
    }
}

pub struct PortSet {
    pub clock: OkClock,
    pub ids: AnyIds,
    pub random: OkRandom,
    pub key: AnyKey,
    pub provider: OkProvider,
    pub sink: OkSink,
}

impl PortSet {
    pub fn happy() -> Self {
        Self {
            clock: OkClock::new(),
            ids: AnyIds::Ok(OkIds),
            random: OkRandom,
            key: AnyKey::Ok(OkKey),
            provider: OkProvider,
            sink: OkSink::new(),
        }
    }

    pub fn ports(&mut self) -> Ports<'_> {
        Ports {
            clock: &mut self.clock,
            ids: &mut self.ids,
            randomness: &mut self.random,
            key_source: &self.key,
            provider: &mut self.provider,
            sink: &mut self.sink,
        }
    }
}

// --- gate ------------------------------------------------------------------

/// A gate that answers with a fixed evaluation.
pub struct FakeGate {
    pub findings: Vec<EngineFinding>,
    pub port_failures: Vec<PortFailure>,
    pub envelopes: Vec<Vec<u8>>,
}

impl FakeGate {
    pub fn allowing() -> Self {
        Self {
            findings: vec![finding(
                "precondition.beneficiary-eligible",
                EngineFindingOutcome::Passed,
            )],
            port_failures: Vec::new(),
            envelopes: vec![br#"{"envelope":1}"#.to_vec()],
        }
    }

    pub fn refusing() -> Self {
        Self {
            findings: vec![
                finding(
                    "precondition.beneficiary-eligible",
                    EngineFindingOutcome::Passed,
                ),
                EngineFinding {
                    gate_id: "precondition.budget".into(),
                    outcome: EngineFindingOutcome::Refused,
                    refusal: Some("precondition_failed".into()),
                    evidence: Some(evidence('7')),
                    opaque: None,
                },
                finding("requirement.receipt", EngineFindingOutcome::NotReached),
            ],
            port_failures: Vec::new(),
            envelopes: vec![br#"{"envelope":2}"#.to_vec()],
        }
    }

    pub fn with_port_failure(failure: PortFailure) -> Self {
        Self {
            findings: vec![finding("precondition.a", EngineFindingOutcome::Passed)],
            port_failures: vec![failure],
            envelopes: Vec::new(),
        }
    }

    /// A gate whose envelopes carry arbitrary bytes — for the export-scan tests.
    pub fn with_envelopes(envelopes: Vec<Vec<u8>>) -> Self {
        Self {
            findings: vec![finding("precondition.a", EngineFindingOutcome::Passed)],
            port_failures: Vec::new(),
            envelopes,
        }
    }

    pub fn aborting() -> Self {
        Self {
            findings: Vec::new(),
            port_failures: Vec::new(),
            envelopes: Vec::new(),
        }
    }
}

impl GateEvaluator for FakeGate {
    fn evaluate(
        &mut self,
        request: GateEvaluationRequest<'_, '_>,
    ) -> Result<EngineEvaluation, EngineBindingError> {
        if self.findings.is_empty() && self.envelopes.is_empty() && self.port_failures.is_empty() {
            return Err(EngineBindingError::Aborted);
        }
        let _ = (request.attempt_id, request.scenario, request.action);
        Ok(EngineEvaluation {
            findings: self.findings.clone(),
            envelopes: self.envelopes.clone(),
            port_failures: self.port_failures.clone(),
        })
    }
}

pub fn finding(gate_id: &str, outcome: EngineFindingOutcome) -> EngineFinding {
    EngineFinding {
        gate_id: gate_id.to_string(),
        outcome,
        refusal: None,
        evidence: None,
        opaque: None,
    }
}

pub fn evidence(byte: char) -> EvidenceRef {
    EvidenceRef {
        digest: digest(byte),
        kind: rigorix_gate_wasm::domain::outcome::EvidenceKind::Precondition,
    }
}

// --- session fixture -------------------------------------------------------

pub fn inputs() -> rigorix_gate_wasm::domain::session::OpenInputs {
    rigorix_gate_wasm::domain::session::OpenInputs {
        scenario: rigorix_gate_wasm::domain::value::ScenarioRef::new(
            digest('a'),
            rigorix_gate_wasm::domain::value::ScenarioFormatVersion(1),
        ),
        fixtures: rigorix_gate_wasm::domain::value::FixtureWorld::new(
            digest('b'),
            rigorix_gate_wasm::domain::value::FixtureTranscript(
                serde_json::json!({ "approvals": [] }),
            ),
        ),
        clock: OkClock::new().0,
        seed: RandomSeed::new(digest('d'), vec![Draw::from_trusted("00")]),
    }
}

pub fn action() -> rigorix_gate_wasm::domain::value::GatedAction {
    rigorix_gate_wasm::domain::value::GatedAction::new(
        "payment_execute",
        rigorix_gate_wasm::domain::value::Params(HashMap::new().into_iter().collect()),
    )
}

// --- stamp / content policy ------------------------------------------------

pub struct TestStamp;
impl rigorix_gate_wasm::domain::services::ArtifactStamp for TestStamp {
    fn stamp(
        &self,
        session: &rigorix_gate_wasm::domain::session::ReplaySession,
    ) -> rigorix_gate_wasm::domain::artifact::VersionStamp {
        rigorix_gate_wasm::domain::artifact::VersionStamp {
            artifact_format_version:
                rigorix_gate_wasm::domain::artifact::ArtifactSet::FORMAT_VERSION,
            engine_version: "1.12.1".to_string(),
            scenario_digest: session.scenario().digest.clone(),
        }
    }

    fn outcome_digest(
        &self,
        session: &rigorix_gate_wasm::domain::session::ReplaySession,
    ) -> Digest {
        let material = match session.outcome() {
            Some(outcome) => serde_json::to_vec(outcome).unwrap_or_default(),
            None => Vec::new(),
        };
        Digest::of(&material)
    }
}

pub struct AllowPolicy;
impl rigorix_gate_wasm::domain::services::ArtifactContentPolicy for AllowPolicy {
    fn check(
        &self,
        _artifact: &rigorix_gate_wasm::domain::artifact::ArtifactSet,
    ) -> Result<(), rigorix_gate_wasm::domain::services::ContentRule> {
        Ok(())
    }
}

pub struct RejectPolicy(pub rigorix_gate_wasm::domain::services::ContentRule);
impl rigorix_gate_wasm::domain::services::ArtifactContentPolicy for RejectPolicy {
    fn check(
        &self,
        _artifact: &rigorix_gate_wasm::domain::artifact::ArtifactSet,
    ) -> Result<(), rigorix_gate_wasm::domain::services::ContentRule> {
        Err(self.0)
    }
}

/// A sealed artifact set, for adapter and policy tests.
pub fn sealed_artifact() -> rigorix_gate_wasm::domain::artifact::ArtifactSet {
    use rigorix_gate_wasm::domain::artifact::{ArtifactSet, EnvelopeBytes, VersionStamp};

    ArtifactSet {
        stamp: VersionStamp {
            artifact_format_version: ArtifactSet::FORMAT_VERSION,
            engine_version: "1.12.1".to_string(),
            scenario_digest: digest('a'),
        },
        fixture_digest: digest('b'),
        session_id: rigorix_gate_wasm::domain::value::ReplaySessionId::from_digest(digest('c')),
        key_ref: KeyRef::from_trusted("demo:sha256:abcd"),
        envelopes: vec![EnvelopeBytes(br#"{"decision":"refused"}"#.to_vec())],
        evidence: Vec::new(),
        outcome_digest: digest('d'),
    }
}
