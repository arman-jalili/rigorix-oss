//! `OpenSessionDto` — the four inputs a host must supply.
//!
//! @canonical .pi/architecture/decisions/ADR-004-api-design.md
//! Implements: Contract Freeze — OpenSessionDto
//! Issue: #1294 (contract freeze)

use serde::{Deserialize, Serialize};

/// The `open_session` request, as one object.
///
/// # Contract (Frozen)
/// - All four fields required; nothing has a default
/// - `scenario` and `fixtures` are engine-shaped JSON; `clock` is an ordered array of
///   readings; `seed` carries the seed digest and the recorded draws
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenSessionDto {
    /// Scenario JSON, in the engine's template/sequence-policy format.
    pub scenario: serde_json::Value,
    /// Fixture transcript JSON.
    pub fixtures: serde_json::Value,
    /// Ordered clock readings.
    pub clock: Vec<ClockReadingDto>,
    /// The declared seed and draws.
    pub seed: SeedDto,
}

/// One pinned instant on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockReadingDto {
    /// RFC 3339 instant.
    pub instant: String,
    /// Declared ordering index.
    pub index: u32,
}

/// The seed and its recorded draws, on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeedDto {
    /// `sha256:<hex>` of the seed material. The seed itself is never supplied.
    pub seed_digest: String,
    /// Recorded draws, in order.
    pub draws: Vec<String>,
}
