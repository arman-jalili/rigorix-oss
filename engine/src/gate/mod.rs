//! Gate composition: the one place a gate decision is ordered, and the language it speaks.
//!
//! @canonical .pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md#2
//! @canonical .pi/architecture/modules/gate.md
//! Implements: Phase C.1 (#1381) — one entry point both runtimes call
//! Issue: #1381 (Phase C.1), #1406 (the module's definition)
//!
//! # What this module owns
//!
//! Two things, and they are different in kind.
//!
//! **The order.** The two halves of a gate decision are other contexts' code: the
//! `precondition` service decides whether the step may dispatch at all, and the
//! `sequence_policy` requirements decide which operator obligations the step leaves
//! unmet. What was *not* another context's code was the order they run in — that lived
//! in each caller, so "the browser decides the same as the engine" could only ever be
//! true by imitation. This module holds one function that composes them in a fixed,
//! fail-closed order, and its two entry points: `evaluate_gate` for an executor,
//! `evaluate_gate_sync` for a host that has none (#1389).
//!
//! **The language of the answer.** A decision has three verdicts, not two: `Dispatch`,
//! `Promote`, `Refuse`. A gate that could not evaluate is not a gate that refused, so
//! `GateOutcome::CouldNotEvaluate` exists beside `Refused`. The digest covers the verdict
//! and the findings and nothing ambient, which is the determinism claim as a value. Those
//! are invariants no other context states, and they live in [`domain`].
//!
//! The first version of this module said it owned no domain, and put all of the above in
//! `application/dto.rs` beside the request carriers. That document was wrong, and #1406
//! exists because the directory shape recorded the mistake as the standard.
//!
//! # Layers
//!
//! | layer | holds | why it is here |
//! |---|---|---|
//! | [`domain`] | `GateVerdict`, `GateOutcome`, `GateFinding`, `GateDecision`, `GateError`, `GatedStep`, the envelope binding, and `decision_digest` | the decision's own language and its invariants |
//! | [`application`] | `evaluate_gate`, `evaluate_gate_sync`, the envelope step, and the request/ports a caller supplies | the use case and its two entry points |
//! | [`infrastructure`] | `GateSetup` — ports built from `.rigorix/` for a native runtime | configuration and wiring, native only |
//!
//! There is no `interfaces` layer and that is a decision, not a gap: the gate exports no
//! protocol. Its two callers are its interfaces — the CLI's orchestrator (native) and the
//! crate's `EngineGate` (a host-supplied surface) — and a fourth directory under the
//! engine would mirror a surface the engine does not own.
//!
//! # What it deliberately does not do
//!
//! - It does not sign. The engine produces the decision and its canonical digest; the key
//!   source that signs it belongs to the runtime (ADR-013: the crate's bundle carries no
//!   verifier, and no private key).
//! - It does not read configuration in the portable slice. [`infrastructure`] builds ports
//!   from the operator's files for a native runtime; a browser supplies evidence-backed
//!   ports directly.
//! - It does not decide anything a gate did not say. Every finding is one of the services'
//!   own findings, in the order they produced them.

pub mod application;
pub mod domain;
#[cfg(not(feature = "portable"))]
pub mod infrastructure;
