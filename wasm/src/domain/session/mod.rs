//! The aggregate: `ReplaySession` and its one entity, `GateAttempt`.
//!
//! @canonical .pi/architecture/modules/portable-session.md#aggregates
//! Implements: Contract Freeze — ReplaySession, GateAttempt
//! Issue: #1294 (contract freeze)

mod gate_attempt;
mod replay_session;

pub use gate_attempt::GateAttempt;
pub use replay_session::{OpenInputs, ReplaySession};
