//! `RandomSeed` and `Draw` — replayable randomness.
//!
//! @canonical .pi/architecture/modules/portable-session.md#value-objects-entity-vs-vo-justification
//! Implements: Contract Freeze — RandomSeed, Draw
//! Issue: #1294 (contract freeze)
//!
//! Nonces and any other entropy the engine needs come from a declared seed and a
//! recorded draw sequence, so two runs produce the same bytes (invariant 5). There is
//! no `thread_rng` in this crate.

use serde::{Deserialize, Serialize};

use super::Digest;

/// One recorded draw from the seed.
///
/// # Contract (Frozen)
/// - Wire format is the hex body of the drawn bytes; the drawn value is recorded so
///   a mismatch between two runs is visible rather than silently different
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Draw(String);

impl Draw {
    /// Wrap a drawn value.
    pub fn from_trusted(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// The hex body.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The declared seed plus the recorded draw sequence.
///
/// # Contract (Frozen)
/// - Equal by seed digest and draw sequence
/// - A draw beyond the recorded sequence is `PortFailure::Exhausted`, never a fresh
///   value (invariant 2)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomSeed {
    /// Digest of the declared seed material. The seed itself is never exported.
    pub seed_digest: Digest,
    /// Draws recorded for this session, in order.
    pub draws: Vec<Draw>,
}

impl RandomSeed {
    /// Pin a seed digest and its recorded draws.
    pub fn new(seed_digest: Digest, draws: Vec<Draw>) -> Self {
        Self { seed_digest, draws }
    }
}
