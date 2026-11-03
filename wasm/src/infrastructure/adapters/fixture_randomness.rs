//! `FixtureRandomness` — answers entropy from the recorded draws.
//!
//! @canonical .pi/architecture/modules/portable-session.md#ports-declared-by-the-domain
//! Implements: Contract Freeze — FixtureRandomness
//! Issue: #1294 (contract freeze)

use crate::domain::ports::Randomness;
use crate::domain::value::{Draw, PortFailure, RandomSeed};

/// Answers from the declared seed's recorded draw sequence.
///
/// # Contract (Frozen)
/// - Consumes in order; exhaustion is `PortFailure { Randomness, Exhausted }`
/// - No `thread_rng` fallback and no re-seeding (invariant 5)
pub struct FixtureRandomness {
    seed: RandomSeed,
    cursor: usize,
}

impl FixtureRandomness {
    /// Wrap the pinned seed.
    pub const fn new(seed: RandomSeed) -> Self {
        Self { seed, cursor: 0 }
    }
}

impl Randomness for FixtureRandomness {
    fn draw(&mut self) -> Result<Draw, PortFailure> {
        let _ = (&self.seed, self.cursor);
        todo!("contract freeze: FixtureRandomness::draw lands with #1298")
    }
}
