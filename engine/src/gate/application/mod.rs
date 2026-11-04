//! Application layer: the composition entry point and its DTOs.
//!
//! The only layer in this module. `compose` is the decision; `dto` is what crosses
//! the boundary; `setup` builds ports from operator configuration for a native
//! runtime and is therefore not part of the portable slice.

pub mod compose;
pub mod dto;
/// Native-only: builds ports from `.rigorix/` operator configuration. The portable
/// slice takes its ports from the host instead, so this file is not compiled there
/// (`--gates` holds the inventory to that).
#[cfg(not(feature = "portable"))]
pub mod setup;

pub use compose::evaluate_gate;
pub use dto::{
    GateDecision, GateEnvelopeBinding, GateError, GateFinding, GateId, GateOutcome, GatePorts,
    GateRequest, GateVerdict, GatedStep,
};
#[cfg(not(feature = "portable"))]
pub use setup::GateSetup;
