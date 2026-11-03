//! Value objects of the Portable Session context.
//!
//! @canonical .pi/architecture/modules/portable-session.md#value-objects-entity-vs-vo-justification
//! Implements: Contract Freeze — ScenarioRef, FixtureWorld, ClockReading, ClockTrack,
//!   RandomSeed, Draw, Timestamp, Digest, ReplaySessionId, GateAttemptId, KeyRef,
//!   GatedAction, Params, PortId, PortFailureKind, PortFailure
//! Issue: #1294 (contract freeze)
//!
//! Every type here is an immutable value: no identity, equal by state. That is what
//! makes the determinism invariant checkable — two runs with equal values produce
//! equal bytes.
//!
//! # Contract (Frozen)
//! - All types are `Clone` and comparable; all serialise canonically
//! - No type holds a clock, an environment handle or a file path
//! - `Digest` is `sha256:<hex>`; `KeyRef` is opaque (never key material)

mod action;
mod clock;
mod digest;
mod fixture;
mod ids;
mod key;
mod ports;
mod randomness;
mod scenario;

pub use action::{GatedAction, Params};
pub use clock::{ClockReading, ClockTrack, Timestamp};
pub use digest::Digest;
pub use fixture::{FixtureTranscript, FixtureWorld, FixtureWorldError};
pub use ids::{GateAttemptId, ReplaySessionId};
pub use key::KeyRef;
pub use ports::{PortFailure, PortFailureKind, PortId};
pub use randomness::{Draw, RandomSeed};
pub use scenario::{ScenarioFormatVersion, ScenarioRef, ScenarioRefError};
