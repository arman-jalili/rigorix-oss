//! Domain layer: the gate's own language.
//!
//! @canonical .pi/architecture/modules/gate.md
//! Issue: #1406
//!
//! # Why this module has a domain after all
//!
//! The first version said it had none: every finding is a service's finding, and the
//! composition decides nothing of its own. That was true of the *inputs* and false of the
//! *output* — this module owns one language that neither service speaks:
//!
//! - a decision has **three** possible verdicts, and a gate that has two folds `Promote`
//!   into `Dispatch` (#1392);
//! - a gate that *could not evaluate* is not a gate that refused — `CouldNotEvaluate`
//!   exists so "we denied you" and "we could not tell" cannot be written the same;
//! - the digest covers the verdict and the findings and nothing ambient, which is the
//!   determinism claim stated as a value rather than as a sentence.
//!
//! Those are invariants, and invariants that no other context states are what a domain
//! layer is for. The old arrangement put them in `application/dto.rs` beside the request
//! carriers and called them DTOs, which is how a module ends up with one layer and a
//! document that certifies it.
//!
//! # What is not here
//!
//! No persistence, no transport, no clock, no key. The decision is computed from pinned
//! inputs and handed back; signing belongs to the runtime that holds a key (ADR-013).

pub mod binding;
pub mod decision;
pub mod error;
pub mod facts;
pub mod finding;
pub mod outcome;
pub mod step;
pub mod verdict;

pub use binding::{GateEnvelopeBinding, GateEnvelopeEvidence};
pub use decision::{GateDecision, decision_canonical_bytes, decision_digest};
pub use error::GateError;
pub use facts::{CapacityFacts, CommittedEffect, EffectFacts};
pub use finding::{GateFinding, GateId};
pub use outcome::GateOutcome;
pub use step::GatedStep;
pub use verdict::GateVerdict;
