//! Application layer: the use case, its two entry points, and what they take.
//!
//! @canonical .pi/architecture/modules/gate.md
//!
//! `compose` is the decision; `sync` is its one-poll bridge for a host with no executor;
//! `envelope` assembles and signs the record when the caller supplies the run's facts;
//! `request` is what a caller hands over — the step, the pins, and the ports.

pub mod compose;
/// The envelope step: facts the caller has, assembly and signing done here (#1389).
pub mod envelope;
/// The fixture world: a declaration of what a host would observe, and the ports that
/// answer it (#1414). Slice-resident, so both runtimes read one interpretation.
pub mod fixtures;
pub mod request;
/// The sync bridge for a runtime with no executor (#1389).
pub mod sync;

pub use compose::evaluate_gate;
pub use fixtures::{FixtureDeclaration, FixtureError, FixtureWorld};
pub use request::{GatePorts, GateRequest, RunContext};
pub use sync::evaluate_gate_sync;
