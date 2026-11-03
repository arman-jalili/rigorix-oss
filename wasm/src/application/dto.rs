//! Application DTOs — the shapes services accept, independent of JSON or JS.
//!
//! @canonical .pi/architecture/decisions/ADR-004-api-design.md
//! Implements: Contract Freeze — OpenSessionInput
//! Issue: #1294 (contract freeze)
//!
//! # Contract (Frozen)
//! - Application DTOs carry **no serde derives**: serialization is the interface
//!   layer's job, so the wire format can change without touching the application
//! - An all-or-nothing input: nothing here is optional, and nothing has a default
//!   (a default is how a refusal becomes an allow)

use crate::domain::value::{ClockTrack, FixtureWorld, RandomSeed, ScenarioRef};

/// The assembled inputs a session is opened from.
///
/// # Contract (Frozen)
/// - Every field is required; a missing one is `SessionOpenError::FixtureIncomplete`
/// - Supplied by the host as **data**; none of it is authority (FR-009)
#[derive(Debug, Clone, PartialEq)]
pub struct OpenSessionInput {
    /// The engine scenario reference.
    pub scenario: ScenarioRef,
    /// The host-supplied fixture world.
    pub fixtures: FixtureWorld,
    /// Pinned clock readings, in order.
    pub clock: ClockTrack,
    /// The declared seed and recorded draws.
    pub seed: RandomSeed,
}
