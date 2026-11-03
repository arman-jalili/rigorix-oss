//! Domain events, classified **evidence** vs **telemetry** (ADR-003).
//!
//! @canonical .pi/architecture/decisions/ADR-003-cross-context-events.md
//! @canonical .pi/architecture/modules/portable-session.md#domain-events
//! Implements: Contract Freeze — DomainEvent, EventClass and the seven payloads
//! Issue: #1294 (contract freeze)
//!
//! There is one bounded context, so "cross-context" here means cross *boundary*: which
//! events may enter a signed artifact and which are diagnostics that never do.
//!
//! # Contract (Frozen)
//! - `RandomDrawConsumed` is telemetry: it is never published, never signed, and is
//!   compiled out of a release build (criterion 17)
//! - The other six are evidence and may be carried into the artifact set
//! - Events are in-process values collected by the aggregate and returned; there is no
//!   bus, no registry and no `static` side channel

use serde::{Deserialize, Serialize};

use crate::domain::outcome::EvidenceRef;
use crate::domain::value::{
    Digest, GateAttemptId, GatedAction, KeyRef, PortFailure, ReplaySessionId,
};

/// Which class an event belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventClass {
    /// A fact a record may carry, and an auditor may rely on.
    Evidence,
    /// A diagnostic. Never signed, never exported.
    Telemetry,
}

/// `ReplaySessionOpened` — the replay key was fixed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionOpened {
    /// Session identity.
    pub session_id: ReplaySessionId,
    /// Scenario digest.
    pub scenario_digest: Digest,
    /// Fixture transcript digest.
    pub fixture_digest: Digest,
    /// Seed digest (never the seed itself).
    pub seed_digest: Digest,
    /// Number of pinned clock readings.
    pub clock_len: usize,
}

/// `RunAllowed` — the gate permitted every step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunAllowed {
    /// Attempt identity.
    pub attempt_id: GateAttemptId,
    /// The action that was permitted.
    pub action: GatedAction,
    /// Evidence produced by the run, ordered.
    pub evidence: Vec<EvidenceRef>,
}

/// `RunRefused` — the gate refused, and here is the step that did it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRefused {
    /// Attempt identity.
    pub attempt_id: GateAttemptId,
    /// Stable id of the step that refused.
    pub caused_by_step: String,
    /// Business-worded reason.
    pub reason: String,
    /// Evidence for the refusal, when one was produced.
    pub evidence: Option<EvidenceRef>,
}

/// `StepDecisionRecorded` — one step's decision, inside the run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepDecisionRecorded {
    /// Attempt identity.
    pub attempt_id: GateAttemptId,
    /// Position in engine evaluation order.
    pub step_index: u32,
    /// The gate that decided.
    pub gate_id: String,
    /// The decision, as a snake_case label so telemetry cannot re-encode it.
    pub decision: String,
}

/// `PortAnswerUnavailable` — a capability could not answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortAnswerUnavailable {
    /// Attempt identity.
    pub attempt_id: GateAttemptId,
    /// Which port, and how it failed.
    pub failure: PortFailure,
    /// Index of the step that needed it.
    pub step_index: u32,
}

/// `RandomDrawConsumed` — telemetry only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomDrawConsumed {
    /// Position in the recorded draw sequence.
    pub draw_index: u32,
    /// What the draw was for.
    pub purpose: String,
}

/// `ArtifactSetExported` — the sealed set left the crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSetExported {
    /// Container format version.
    pub artifact_format_version: u32,
    /// Engine version.
    pub engine_version: String,
    /// Scenario digest.
    pub scenario_digest: Digest,
    /// Digest over the run outcome.
    pub outcome_digest: Digest,
    /// Opaque key reference.
    pub key_ref: KeyRef,
}

/// Every event this context produces.
///
/// # Contract (Frozen)
/// - Seven variants, matching the exploration's seven captured events
/// - `class()` is the single place the evidence/telemetry split is decided
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum DomainEvent {
    /// The session was accepted.
    ReplaySessionOpened(SessionOpened),
    /// The run was permitted.
    RunAllowed(RunAllowed),
    /// The run was refused.
    RunRefused(RunRefused),
    /// A step decision was recorded.
    StepDecisionRecorded(StepDecisionRecorded),
    /// A port could not answer.
    PortAnswerUnavailable(PortAnswerUnavailable),
    /// A draw was consumed (telemetry).
    RandomDrawConsumed(RandomDrawConsumed),
    /// The artifact set was exported.
    ArtifactSetExported(ArtifactSetExported),
}

impl DomainEvent {
    /// Evidence or telemetry.
    pub const fn class(&self) -> EventClass {
        match self {
            Self::RandomDrawConsumed(_) => EventClass::Telemetry,
            Self::ReplaySessionOpened(_)
            | Self::RunAllowed(_)
            | Self::RunRefused(_)
            | Self::StepDecisionRecorded(_)
            | Self::PortAnswerUnavailable(_)
            | Self::ArtifactSetExported(_) => EventClass::Evidence,
        }
    }

    /// Whether this event may enter a signed artifact set.
    pub const fn is_evidence(&self) -> bool {
        matches!(self.class(), EventClass::Evidence)
    }
}
