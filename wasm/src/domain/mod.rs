//! Domain layer — the only place this crate owns a decision's shape.
//!
//! @canonical .pi/architecture/modules/portable-session.md#domain-layer-domain
//! @canonical .pi/architecture/decisions/ADR-008-proportional-modeling-depth.md
//! Implements: Contract Freeze — domain layer
//! Issue: #1294 (contract freeze)
//!
//! Pure: no `std::time`, no `std::env`, no `std::fs`, no random source, no
//! `infrastructure`/`interfaces` path. Capability access is only ever a port.
//!
//! # Contract (Frozen)
//! - One aggregate root: [`ReplaySession`](session::ReplaySession)
//! - 12 value objects, each immutable and equal by state
//! - Six engine-declared ports + a domain-owned session store
//! - Seven domain events, classified evidence vs telemetry
//! - Eight invariants, one per `prevented` clause from the exploration

pub mod artifact;
pub mod engine_seam;
pub mod errors;
pub mod events;
pub mod outcome;
pub mod ports;
pub mod services;
pub mod session;
pub mod value;

pub use artifact::{ArtifactSet, VersionStamp};
pub use errors::{AttemptError, SealError, SessionOpenError};
pub use events::{DomainEvent, EventClass};
pub use outcome::{EvidenceRef, OutcomeStatus, RunOutcome, StepDecision, StepOutcome};
pub use services::{
    ArtifactContentPolicy, ArtifactStamp, ContentRule, EvidenceProjection, RefusalVocabulary,
};
pub use session::{GateAttempt, ReplaySession};
pub use value::{
    ClockReading, ClockTrack, Digest, Draw, FixtureTranscript, FixtureWorld, GateAttemptId,
    GatedAction, KeyRef, Params, PortFailure, PortFailureKind, PortId, RandomSeed, ReplaySessionId,
    ScenarioFormatVersion, ScenarioRef, Timestamp,
};
