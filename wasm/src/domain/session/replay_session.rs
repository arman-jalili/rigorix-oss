//! `ReplaySession` — the aggregate root.
//!
//! @canonical .pi/architecture/modules/portable-session.md#replaysession
//! @canonical .pi/architecture/decisions/ADR-008-proportional-modeling-depth.md
//! Implements: Contract Freeze — ReplaySession, OpenInputs
//! Issue: #1294 (contract freeze)
//!
//! # The eight invariants this object exists to hold
//!
//! | # | Invariant | What it prevents |
//! |---|-----------|------------------|
//! | 1 | No ambient capability: time, entropy, network, fs only through the five ports | two runs of one session produce different bytes |
//! | 2 | Every port answer comes from the transcript; missing/exhausted/malformed/failed ⇒ typed `PortFailure` ⇒ refusal | an unanswered port defaults to allow |
//! | 3 | Host values are data, never authority | a host-minted allow laundered through the gate |
//! | 4 | One run per session, sealed with its evidence; neither is reopened | an outcome without its evidence, or evidence edited after the verdict |
//! | 5 | Timestamps, ids and randomness come only from pinned readings, the declared id sequence and the declared seed | nondeterminism from a clock read or a fresh UUID |
//! | 6 | Verification is three-valued and never collapsed | a record the host could not check reported as tampered |
//! | 7 | No secret, key, absolute path or personal data in artifacts | key material or a host path leaving in a shared record |
//! | 8 | Every set is stamped; an unsupported format is refused, not guessed | a record of unknown provenance verified against today's rules |
//!
//! # Contract (Frozen)
//! - Behaviour only: `open`, `attempt`, `seal`, `take_events`; **no setters**, no
//!   `&mut` access to state, no public fields
//! - `open` fixes the inputs; `attempt` runs at most once; `seal` seals at most once
//! - Read projections (`scenario`, `fixtures`, `outcome`, `sealed`, `id`) are pure

use crate::domain::artifact::{ArtifactSet, EnvelopeBytes};
use crate::domain::engine_seam::{GateEvaluationRequest, GateEvaluator};
use crate::domain::errors::{AttemptError, SealError, SessionOpenError};
use crate::domain::events::{
    ArtifactSetExported, DomainEvent, PortAnswerUnavailable, RunAllowed, RunRefused, SessionOpened,
    StepDecisionRecorded,
};
use crate::domain::outcome::{
    EvidenceRef, OutcomeStatus, RefusalKind, RunOutcome, StepDecision, StepOutcome,
};
use crate::domain::ports::{IdKind, Ports};
use crate::domain::services::{
    ArtifactContentPolicy, ArtifactStamp, BusinessWording, EvidenceProjection, RefusalVocabulary,
};
use crate::domain::session::GateAttempt;

/// The one scenario format this build evaluates.
///
/// # Contract (Frozen)
/// - A scenario declaring anything else is refused **before** evaluation, so an unknown
///   policy format is never judged by today's rules (FR-010, criterion 12)
pub struct SupportedScenarioFormat;

impl SupportedScenarioFormat {
    /// The supported engine scenario format version.
    pub const VERSION: ScenarioFormatVersion = ScenarioFormatVersion(1);
}
use crate::domain::value::{
    ClockTrack, FixtureWorld, GateAttemptId, GatedAction, KeyRef, PortFailure, PortFailureKind,
    PortId, RandomSeed, ReplaySessionId, ScenarioFormatVersion, ScenarioRef,
};

/// The inputs a session is opened from — all four of them pinned.
///
/// # Contract (Frozen)
/// - These are the replay key: equal inputs ⇒ equal id and equal bytes
/// - Nothing here is optional; a missing input is a typed refusal
#[derive(Debug, Clone, PartialEq)]
pub struct OpenInputs {
    /// The engine scenario reference.
    pub scenario: ScenarioRef,
    /// The host-supplied fixture world.
    pub fixtures: FixtureWorld,
    /// Pinned clock readings, in order.
    pub clock: ClockTrack,
    /// The declared seed and recorded draws.
    pub seed: RandomSeed,
}

/// The aggregate root of the Portable Session context.
pub struct ReplaySession {
    id: ReplaySessionId,
    inputs: OpenInputs,
    attempt: Option<GateAttempt>,
    sealed: Option<ArtifactSet>,
    events: Vec<DomainEvent>,
}

/// Redacted: an aggregate holds the host's fixture world, and a `Debug` that printed it
/// would put host data into every `{session:?}` in a log (NFR-003). The shape is shown;
/// the contents never are.
impl std::fmt::Debug for ReplaySession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReplaySession")
            .field("id", &self.id.as_str())
            .field("scenario", &self.inputs.scenario.digest.as_str())
            .field("fixtures", &self.inputs.fixtures.transcript_digest.as_str())
            .field("clock_readings", &self.inputs.clock.readings().len())
            .field("has_attempt", &self.attempt.is_some())
            .field("sealed", &self.sealed.is_some())
            .field("events", &self.events.len())
            .finish()
    }
}

/// Turn a port failure that happened *before* the gate into a refused run.
///
/// The run still names its cause: a synthesized `port.<name>` step (criterion 10). There is
/// no path from here to an allow (invariant 2).
fn refusal_from_port_failure(failure: PortFailure, events: &mut Vec<DomainEvent>) -> RunOutcome {
    let kind = if failure.port == PortId::KeySource {
        RefusalKind::IdentityUnverified
    } else {
        RefusalKind::PortUnavailable
    };
    let wording = BusinessWording::new();
    let reason = wording.port_reason(failure).to_string();
    let step = StepOutcome {
        // #1462: the session's own record has no engine operands to report.
        gate_id: format!("port.{}", failure.port.as_str()),
        decision: StepDecision::Refused,
        kind: Some(kind),
        reason: Some(reason.clone()),
        evidence: None,
        detail: None,
    };
    events.push(DomainEvent::PortAnswerUnavailable(PortAnswerUnavailable {
        attempt_id: GateAttemptId::from_trusted("unminted"),
        failure,
        step_index: 0,
    }));
    events.push(DomainEvent::StepDecisionRecorded(StepDecisionRecorded {
        attempt_id: GateAttemptId::from_trusted("unminted"),
        step_index: 0,
        gate_id: step.gate_id.clone(),
        decision: "refused".to_string(),
    }));
    events.push(DomainEvent::RunRefused(RunRefused {
        attempt_id: GateAttemptId::from_trusted("unminted"),
        caused_by_step: step.gate_id.clone(),
        reason,
        evidence: None,
    }));
    RunOutcome {
        status: OutcomeStatus::Refused,
        steps: vec![step],
        evidence: Vec::new(),
        // No engine decided this one: a port that could not answer refuses before the gate,
        // so there are no engine bytes to hand out (#1442).
        decision: Vec::new(),
    }
}

impl ReplaySession {
    /// Assemble a replayable session from pinned inputs.
    ///
    /// # Contract (Frozen)
    /// - Unparseable scenario ⇒ `ScenarioUnparseable`; incomplete fixtures ⇒
    ///   `FixtureIncomplete`; empty clock ⇒ `ClockSeriesEmpty` (criterion 1)
    /// - The id is derived from (scenario, fixtures, seed) (criterion 2)
    /// - Raises [`DomainEvent::ReplaySessionOpened`]
    pub fn open(inputs: OpenInputs) -> Result<Self, SessionOpenError> {
        Self::validate(&inputs)?;

        let id = ReplaySessionId::derive(
            &inputs.scenario.digest,
            &inputs.fixtures.transcript_digest,
            &inputs.seed.seed_digest,
        );

        let opened = DomainEvent::ReplaySessionOpened(SessionOpened {
            session_id: id.clone(),
            scenario_digest: inputs.scenario.digest.clone(),
            fixture_digest: inputs.fixtures.transcript_digest.clone(),
            seed_digest: inputs.seed.seed_digest.clone(),
            clock_len: inputs.clock.readings().len(),
        });

        Ok(Self {
            id,
            inputs,
            attempt: None,
            sealed: None,
            events: vec![opened],
        })
    }

    /// Reject inputs the session cannot honestly pin.
    ///
    /// # Contract (Frozen)
    /// - Each check has its own variant, so a caller learns *which* input was wrong
    ///   (criterion 1: a typed refusal, never a default)
    /// - Nothing is repaired, defaulted or trimmed: an input that does not fit is
    ///   refused here, not silently adapted downstream
    fn validate(inputs: &OpenInputs) -> Result<(), SessionOpenError> {
        if inputs.scenario.format_version != SupportedScenarioFormat::VERSION {
            return Err(SessionOpenError::UnsupportedScenarioFormat {
                found: inputs.scenario.format_version,
            });
        }

        if inputs.clock.readings().is_empty() {
            return Err(SessionOpenError::ClockSeriesEmpty);
        }

        // The transcript is host data, so its *shape* is the only thing we can check
        // without owning the business meaning of its contents.
        let Some(object) = inputs.fixtures.transcript.0.as_object() else {
            return Err(SessionOpenError::FixtureUnparseable);
        };

        match object.get("approvals") {
            None => Err(SessionOpenError::FixtureIncomplete { field: "approvals" }),
            Some(serde_json::Value::Array(_)) => Ok(()),
            Some(_) => Err(SessionOpenError::FixtureIncomplete { field: "approvals" }),
        }
    }

    /// Drive the session's one attempt through the engine's gate.
    ///
    /// TODO(#1296): resolve ports, call the gate, project findings, classify failures.
    ///
    /// # Contract (Frozen)
    /// - At most one attempt: a second call is `AttemptError::AlreadyAttempted`
    ///   (criterion 3); a sealed session is `AlreadySealed`
    /// - A port failure becomes a `Refused` outcome carrying
    ///   [`DomainEvent::PortAnswerUnavailable`], never an allow (criterion 4)
    /// - An identity-port failure is `RefusalKind::IdentityUnverified`, never
    ///   `unauthenticated` (criterion 5)
    /// - Raises `RunAllowed`/`RunRefused` plus one `StepDecisionRecorded` per step
    pub fn attempt(
        &mut self,
        action: GatedAction,
        ports: Ports<'_>,
        gate: &mut dyn GateEvaluator,
        projection: &dyn EvidenceProjection,
    ) -> Result<RunOutcome, AttemptError> {
        // Invariant 4 first: a sealed or already-attempted session has nothing to decide.
        if self.sealed.is_some() {
            return Err(AttemptError::AlreadySealed);
        }
        if self.attempt.is_some() {
            return Err(AttemptError::AlreadyAttempted);
        }

        let mut events = Vec::new();

        // The identity port is consulted before the gate: a run that cannot name the key
        // that will sign it cannot produce a record, and the engine's truthful status for
        // that case is `unverified` — never `unauthenticated` (criterion 5).
        let key_ref = match ports.key_source.key_ref() {
            Ok(key_ref) => key_ref,
            Err(failure) => {
                let outcome = refusal_from_port_failure(failure, &mut events);
                return self.record_attempt(action, outcome, None, Vec::new(), events);
            }
        };

        // The attempt identity comes from the id sequence, never from entropy.
        let attempt_id = match ports.ids.next_id(IdKind::Attempt) {
            Ok(raw) => GateAttemptId::from_trusted(raw),
            Err(failure) => {
                let outcome = refusal_from_port_failure(failure, &mut events);
                return self.record_attempt(action, outcome, Some(key_ref), Vec::new(), events);
            }
        };

        let evaluation = match gate.evaluate(GateEvaluationRequest {
            attempt_id: &attempt_id,
            scenario: &self.inputs.scenario,
            action: &action,
            ports,
        }) {
            Ok(evaluation) => evaluation,
            // A binding failure refuses; it never allows. The host sees a refusal, not an
            // outcome the crate invented.
            Err(source) => return Err(AttemptError::EngineFailure { source }),
        };

        let mut steps = projection.project(&evaluation.findings);

        // A port that could not answer refuses the run even if every finding passed: an
        // unavailable check must not read as a passed one (invariant 2). The refusal names
        // the port, so the run still has a causing step (criterion 10).
        for failure in &evaluation.port_failures {
            let kind = if failure.port == PortId::KeySource {
                RefusalKind::IdentityUnverified
            } else {
                RefusalKind::PortUnavailable
            };
            events.push(DomainEvent::PortAnswerUnavailable(PortAnswerUnavailable {
                attempt_id: attempt_id.clone(),
                failure: *failure,
                step_index: steps.len() as u32,
            }));
            steps.push(StepOutcome {
                gate_id: format!("port.{}", failure.port.as_str()),
                decision: StepDecision::Refused,
                kind: Some(kind),
                reason: Some(BusinessWording::new().port_reason(*failure).to_string()),
                evidence: None,
                // A port that could not answer has no operands to report (#1462).
                detail: None,
            });
        }

        let status = if steps.iter().any(StepOutcome::is_refusal) {
            OutcomeStatus::Refused
        } else {
            OutcomeStatus::Allowed
        };

        for (index, step) in steps.iter().enumerate() {
            events.push(DomainEvent::StepDecisionRecorded(StepDecisionRecorded {
                attempt_id: attempt_id.clone(),
                step_index: index as u32,
                gate_id: step.gate_id.clone(),
                decision: match step.decision {
                    StepDecision::Allowed => "allowed",
                    StepDecision::Promoted => "promoted",
                    StepDecision::Refused => "refused",
                    StepDecision::NotReached => "not_reached",
                }
                .to_string(),
            }));
        }

        let evidence: Vec<EvidenceRef> = steps.iter().filter_map(|s| s.evidence.clone()).collect();

        let outcome = RunOutcome {
            status,
            steps,
            evidence,
            decision: evaluation.decision.clone(),
        };

        match outcome.status {
            OutcomeStatus::Allowed => events.push(DomainEvent::RunAllowed(RunAllowed {
                attempt_id: attempt_id.clone(),
                action: action.clone(),
                evidence: outcome.evidence.clone(),
            })),
            OutcomeStatus::Refused => {
                let causing = outcome.causing_step();
                events.push(DomainEvent::RunRefused(RunRefused {
                    attempt_id: attempt_id.clone(),
                    caused_by_step: causing
                        .map(|s| s.gate_id.clone())
                        .unwrap_or_else(|| "port.unknown".to_string()),
                    reason: causing
                        .and_then(|s| s.reason.clone())
                        .unwrap_or_else(|| "the run was refused".to_string()),
                    evidence: causing.and_then(|s| s.evidence.clone()),
                }));
            }
        }

        let envelopes = evaluation
            .envelopes
            .into_iter()
            .map(EnvelopeBytes)
            .collect::<Vec<_>>();

        self.record_attempt(action, outcome, Some(key_ref), envelopes, events)
    }

    /// Record the one attempt and drain its events into the session.
    fn record_attempt(
        &mut self,
        action: GatedAction,
        outcome: RunOutcome,
        key_ref: Option<KeyRef>,
        envelopes: Vec<EnvelopeBytes>,
        mut events: Vec<DomainEvent>,
    ) -> Result<RunOutcome, AttemptError> {
        let id = self
            .attempt
            .as_ref()
            .map(|a| a.id().clone())
            .unwrap_or_else(|| GateAttemptId::from_trusted("unminted"));

        // A refused run without a key reference cannot be sealed later; the record still
        // stands, and `seal` refuses with `NoAttempt`-adjacent semantics rather than
        // inventing a key.
        self.attempt = Some(GateAttempt::record(
            id,
            action,
            outcome.clone(),
            key_ref.unwrap_or_else(|| KeyRef::from_trusted("unavailable")),
            envelopes,
        ));
        self.events.append(&mut events);
        Ok(outcome)
    }

    /// Seal the run into an immutable, stamped artifact set.
    ///
    /// TODO(#1297): stamp, assemble, scan, seal.
    ///
    /// # Contract (Frozen)
    /// - No attempt ⇒ `SealError::NoAttempt`; second seal ⇒ `AlreadySealed`
    ///   (criterion 6)
    /// - The stamp carries format version, engine version, scenario digest, fixture
    ///   digest and outcome digest (criterion 11)
    /// - A violated content rule ⇒ `SealError::ContentRejected`; bytes never leave
    ///   (criterion 7)
    /// - Raises [`DomainEvent::ArtifactSetExported`]
    pub fn seal(
        &mut self,
        stamp: &dyn ArtifactStamp,
        policy: &dyn ArtifactContentPolicy,
    ) -> Result<ArtifactSet, SealError> {
        // Invariant 4: sealed is sealed. The first artifact set is returned untouched.
        if let Some(existing) = &self.sealed {
            let _ = existing;
            return Err(SealError::AlreadySealed);
        }

        let Some(attempt) = &self.attempt else {
            return Err(SealError::NoAttempt);
        };

        // A run whose identity port failed cannot produce a signable record. Refusing here
        // is the honest end of that run, not a key invented at seal time.
        if attempt.key_ref().as_str() == "unavailable" {
            return Err(SealError::PortFailure {
                failure: PortFailure::new(PortId::KeySource, PortFailureKind::Missing),
            });
        }

        let artifact = ArtifactSet {
            stamp: stamp.stamp(self),
            fixture_digest: self.inputs.fixtures.transcript_digest.clone(),
            session_id: self.id.clone(),
            key_ref: attempt.key_ref().clone(),
            envelopes: attempt.envelopes().to_vec(),
            evidence: attempt.outcome().evidence.clone(),
            outcome_digest: stamp.outcome_digest(self),
        };

        // Refuse, never sanitise: a scrubbed record is an altered record (criterion 7).
        if let Err(rule) = policy.check(&artifact) {
            return Err(SealError::ContentRejected { rule });
        }

        self.events
            .push(DomainEvent::ArtifactSetExported(ArtifactSetExported {
                artifact_format_version: artifact.stamp.artifact_format_version,
                engine_version: artifact.stamp.engine_version.clone(),
                scenario_digest: artifact.stamp.scenario_digest.clone(),
                outcome_digest: artifact.outcome_digest.clone(),
                key_ref: artifact.key_ref.clone(),
            }));

        self.sealed = Some(artifact.clone());
        Ok(artifact)
    }

    /// Drain the events raised so far, in order.
    ///
    /// # Contract (Frozen)
    /// - Order is the order they were raised; draining does not reorder
    /// - Telemetry events are included (the observer filters); they never reach an
    ///   artifact set (criterion 17)
    pub fn take_events(&mut self) -> Vec<DomainEvent> {
        std::mem::take(&mut self.events)
    }

    /// The session's derived identity.
    pub fn id(&self) -> &ReplaySessionId {
        &self.id
    }

    /// The pinned inputs.
    pub fn inputs(&self) -> &OpenInputs {
        &self.inputs
    }

    /// The scenario this session is bound to.
    pub fn scenario(&self) -> &ScenarioRef {
        &self.inputs.scenario
    }

    /// The fixture world this session is bound to.
    pub fn fixtures(&self) -> &FixtureWorld {
        &self.inputs.fixtures
    }

    /// The run's outcome, once it has run.
    pub fn outcome(&self) -> Option<&RunOutcome> {
        self.attempt.as_ref().map(GateAttempt::outcome)
    }

    /// The sealed artifact set, once sealed.
    pub fn sealed(&self) -> Option<&ArtifactSet> {
        self.sealed.as_ref()
    }
}
