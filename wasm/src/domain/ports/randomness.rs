//! `Randomness` — replayable entropy.
//!
//! @canonical .pi/architecture/modules/portable-session.md#ports-declared-by-the-domain
//! Implements: Contract Freeze — Randomness
//! Issue: #1294 (contract freeze)

use crate::domain::value::{Draw, PortFailure};

/// Draws from the session's declared seed and recorded sequence.
///
/// # Contract (Frozen)
/// - Each call consumes the next recorded draw
/// - Exhaustion is `PortFailure { port: Randomness, kind: Exhausted }`; there is no
///   `thread_rng` fallback (invariant 5)
pub trait Randomness {
    /// Take the next recorded draw.
    fn draw(&mut self) -> Result<Draw, PortFailure>;
}
