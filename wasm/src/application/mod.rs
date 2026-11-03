//! Application layer — orchestration only.
//!
//! @canonical .pi/architecture/modules/portable-session.md#application-services-from-exploration-use-cases
//! @canonical .pi/architecture/decisions/ADR-008-proportional-modeling-depth.md
//! Implements: Contract Freeze — application layer
//! Issue: #1294 (contract freeze)
//!
//! # Contract (Frozen)
//! - The four catalog services below are the **only** application services. Their
//!   names, use-case text and FR mappings are frozen by the exploration, not by
//!   preference:
//!
//! | Service | Use case | Maps to FR |
//! |---------|----------|-----------|
//! | `OpenReplaySession` | Assemble a replayable session from an engine scenario reference and a host fixture transcript: parse with engine types, pin the clock series and seed, reject anything incomplete with a typed refusal | FR-001, FR-003, FR-011 |
//! | `AttemptRun` | Drive one attempt through the engine's gate: resolve every port answer from the transcript, hand the run to engine policy evaluation, project the RunOutcome with its ordered StepOutcomes and evidence references; a missing or failed port answer becomes a refusal, never an allow | FR-002, FR-004, FR-009, FR-013 |
//! | `ExportArtifactSet` | Serialize the sealed envelopes and evidence references into a self-contained artifact set stamped with scenario digest, engine version and format version, scanned for secrets, absolute paths and personal data before it leaves | FR-005, FR-010, FR-014 |
//! | `RunConformanceVector` | Replay a versioned vector through the wasm seam and through a CLI built from the same commit, compare artifacts byte for byte, fail the build on divergence or on a seeded divergence that does not diverge | FR-007, FR-008, FR-012 |
//!
//! `VerifyExportedArtifacts` was removed by ADR-013: the crate produces, the host
//! verifies. There is no verification service here, and no verifier dependency.
//!
//! - No ambient capability: a service receives ports, it never creates them
//! - No business rule: if a branch would decide something, it belongs in `domain`
//! - No clock, no id minting, no persistence

pub mod artifact_query;
pub mod attempt_run;
pub mod dto;
pub mod export_artifact_set;
pub mod open_replay_session;
pub mod run_conformance_vector;

pub use artifact_query::{ArtifactQuery, ReplaySessionQuery};
pub use attempt_run::{AttemptRun, AttemptRunService};
pub use dto::OpenSessionInput;
pub use export_artifact_set::{ExportArtifactSet, ExportArtifactSetService};
pub use open_replay_session::{OpenReplaySession, OpenReplaySessionService};
pub use run_conformance_vector::{
    ConformanceReport, ConformanceRunner, ConformanceVector, RunConformanceVector,
    default_projection,
};
