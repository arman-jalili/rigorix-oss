//! Gate composition — the one place a gate decision is ordered.
//!
//! @canonical .pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md#2
//! @canonical .pi/architecture/modules/gate.md
//! Implements: Phase C.1 (#1381) — one entry point both runtimes call
//! Issue: #1381 (Phase C.1); consumed by the CLI (#1382) and the wasm crate (#1383)
//!
//! # Why this module owns no domain
//!
//! The two halves of a gate decision are engine code: the precondition service
//! (`precondition`) decides whether the step may dispatch at all, and the sequence
//! policy's requirements (`sequence_policy`) decide which operator obligations the
//! step leaves unmet. What was **not** engine code was the order they run in — that
//! lived in each caller, which meant "the browser decides the same as the engine"
//! could only ever be true by imitation.
//!
//! So this module has no domain of its own and no invariant of its own. It holds
//! one function that composes the two services in a fixed, fail-closed order, and
//! two DTOs: what the caller asks, and what the caller is told. A second runtime
//! calls the same function with its own ports, and the ordering cannot drift,
//! because there is nowhere for it to drift to.
//!
//! # What it deliberately does not do
//!
//! - It does not sign. The engine produces the decision and its canonical digest;
//!   the key source that signs it belongs to the runtime (ADR-013: the crate's
//!   bundle carries no verifier, and no private key).
//! - It does not read configuration. `setup` builds the ports from the operator's
//!   files for a native runtime; a browser supplies evidence-backed ports directly.
//! - It does not decide anything. Every finding is one of the services' own
//!   findings, in the order they produced them.

pub mod application;
