//! Application layer: the composition entry point and its DTOs.
//!
//! The only layer in this module. `compose` is the decision; `dto` is what crosses
//! the boundary; `setup` builds ports from operator configuration for a native
//! runtime and is therefore not part of the portable slice.

pub mod compose;
pub mod dto;
/// The envelope step: facts the caller has, assembly and signing done here (#1389).
pub mod envelope;
/// Native-only: builds ports from `.rigorix/` operator configuration. The portable
/// slice takes its ports from the host instead, so this file is not compiled there
/// (`--gates` holds the inventory to that).
#[cfg(not(feature = "portable"))]
pub mod setup;
/// The sync bridge for a runtime with no executor (#1389).
pub mod sync;

pub use compose::evaluate_gate;
pub use dto::{
    GateDecision, GateEnvelopeBinding, GateEnvelopeEvidence, GateError, GateFinding, GateId,
    GateOutcome, GatePorts, GateRequest, GateVerdict, GatedStep, RunContext,
};
#[cfg(not(feature = "portable"))]
pub use setup::GateSetup;
pub use sync::evaluate_gate_sync;
