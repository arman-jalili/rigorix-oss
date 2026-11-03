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

use crate::domain::artifact::ArtifactSet;
use crate::domain::engine_seam::GateEvaluator;
use crate::domain::errors::{AttemptError, SealError, SessionOpenError};
use crate::domain::events::DomainEvent;
use crate::domain::outcome::RunOutcome;
use crate::domain::ports::Ports;
use crate::domain::services::{ArtifactContentPolicy, ArtifactStamp};
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
use crate::domain::events::SessionOpened;
use crate::domain::value::{
    ClockTrack, FixtureWorld, GatedAction, RandomSeed, ReplaySessionId, ScenarioFormatVersion,
    ScenarioRef,
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
    ) -> Result<RunOutcome, AttemptError> {
        let _ = (action, ports, gate);
        todo!("contract freeze: ReplaySession::attempt lands with the aggregate issue")
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
        let _ = (stamp, policy);
        todo!("contract freeze: ReplaySession::seal lands with the aggregate issue")
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
