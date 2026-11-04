//! `evaluate_gate_sync` — the bridge for a runtime with no executor.
//!
//! @canonical .pi/architecture/modules/gate.md
//! Issue: #1389 (Phase C.1b)
//!
//! # Why the façade is here and not in the adapter
//!
//! The wasm carrier is synchronous by frozen contract (`wasm32` has no executor), while the
//! composition is async because `PreconditionService::evaluate` is an `async fn`. Something
//! must bridge, and the only question that matters is **where the property is stated**.
//!
//! A `block_on` inside the adapter would put the requirement ("nothing on the slice's gate
//! path awaits I/O") in the layer that cannot test it. Here it is a single poll with a
//! no-op waker: a composition that is ready on its first poll answers; one that is not
//! returns [`GateError::NotReady`] — a value, not a hang and not a panic (ADR-012 forbids
//! the panic, and a retry loop on `wasm32` is a busy-wait with nothing behind it).
//!
//! The test that keeps this honest polls every port the slice can construct and requires
//! `Ready` on the first poll, so a port that starts awaiting real I/O fails there — at the
//! place that defines the property — rather than in a browser.

use std::task::{Context, Poll, Waker};

use super::compose::evaluate_gate;
use super::request::GateRequest;
use crate::gate::domain::{GateDecision, GateError};

/// Evaluate one run, requiring the composition to complete on its first poll.
///
/// Equivalent to awaiting [`evaluate_gate`] for every request whose ports answer without
/// awaiting I/O, which is every port the portable slice can construct.
///
/// # Errors
/// - [`GateError::NotReady`] if the composition pended
/// - every error [`evaluate_gate`] can return
pub fn evaluate_gate_sync(request: GateRequest<'_>) -> Result<GateDecision, GateError> {
    let mut future = Box::pin(evaluate_gate(request));
    let waker = Waker::noop();
    let mut cx = Context::from_waker(waker);
    match future.as_mut().poll(&mut cx) {
        Poll::Ready(result) => result,
        // Not an error in the composition — an unmet precondition of the *runtime*. The
        // caller is told which, so it can supply a port that resolves.
        Poll::Pending => Err(GateError::NotReady),
    }
}
