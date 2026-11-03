//! `FixtureRandomness` — answers entropy from the recorded draws.
//!
//! @canonical .pi/architecture/modules/portable-session.md#ports-declared-by-the-domain
//! Implements: `Randomness`
//! Issue: #1298 (infrastructure layer)

use crate::domain::ports::Randomness;
use crate::domain::value::{Draw, PortFailure, PortFailureKind, PortId, RandomSeed};

/// Answers from the declared seed's recorded draw sequence.
#[derive(Debug)]
pub struct FixtureRandomness {
    seed: RandomSeed,
    cursor: usize,
}

impl FixtureRandomness {
    /// Wrap the pinned seed.
    pub const fn new(seed: RandomSeed) -> Self {
        Self { seed, cursor: 0 }
    }

    /// How many draws remain unconsumed.
    pub fn remaining(&self) -> usize {
        self.seed.draws.len().saturating_sub(self.cursor)
    }
}

impl Randomness for FixtureRandomness {
    fn draw(&mut self) -> Result<Draw, PortFailure> {
        let draw = self.seed.draws.get(self.cursor).cloned();
        match draw {
            Some(draw) => {
                self.cursor += 1;
                Ok(draw)
            }
            None => Err(PortFailure::new(
                PortId::Randomness,
                PortFailureKind::Exhausted,
            )),
        }
    }
}
