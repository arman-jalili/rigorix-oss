//! Session registry — the opaque handle behind the four exported functions.
//!
//! @canonical .pi/architecture/decisions/ADR-004-api-design.md
//! Implements: the handle mechanism (AC 15, 16)
//! Issue: #1309 (WasmContract)
//!
//! # Why a registry at all
//!
//! The host must not hold a `&mut ReplaySession`: a pointer into linear memory is a way to
//! reach in and set an outcome. So the host holds a **string**, and the crate keeps the
//! aggregate on its own side of the boundary.
//!
//! # Contract
//! - A handle is opaque; its format is not part of the contract and may change
//! - A malformed or unknown handle is the **only** error a host can provoke. Every gate
//!   decision, including a refusal, is a return value (ADR-012)
//! - State is per module instance: no persistence, no sharing between instances, and a
//!   reloaded page starts empty (ADR-002)

use std::cell::RefCell;
use std::collections::HashMap;

/// The aggregate plus the gate it runs on, held for the life of the instance.
pub(crate) struct SessionSlot {
    /// The aggregate.
    pub(crate) session: crate::domain::session::ReplaySession,
    /// The gate this session evaluates against.
    pub(crate) gate: Box<dyn crate::domain::engine_seam::GateEvaluator>,
}

thread_local! {
    /// One map per instance: the wasm target is single-threaded, so no lock is needed and
    /// none is added (a lock would be a native dependency for no gain).
    static SESSIONS: RefCell<HashMap<String, SessionSlot>> = RefCell::new(HashMap::new());
}

/// Why a handle could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HandleError {
    /// The handle did not have the expected shape.
    #[error("the session handle is malformed")]
    Malformed,
    /// The handle named a session this instance does not hold.
    #[error("this session is not open in this instance")]
    Unknown,
    /// The registry was already borrowed.
    ///
    /// Unreachable today: the surface has no callback that could re-enter. It exists as a
    /// value rather than a panic because `borrow_mut` panics on re-entry, and a panic on
    /// this path traps the module (ADR-012 tier 3).
    #[error("the session registry is busy; try again")]
    Unavailable,
}

/// The handle string for a session identity.
pub(crate) fn handle_for(id: &crate::domain::value::ReplaySessionId) -> String {
    format!("gate:{}", id.as_str())
}

/// Insert a session and return its handle.
pub(crate) fn insert(slot: SessionSlot) -> String {
    let handle = handle_for(slot.session.id());
    // A failed borrow would leave the session unregistered, which the caller sees as an
    // `Unknown` handle on the next call — a refusal, not a panic.
    let _ = SESSIONS.try_with(|sessions| {
        if let Ok(mut sessions) = sessions.try_borrow_mut() {
            sessions.insert(handle.clone(), slot);
        }
    });
    handle
}

/// Run `f` with the session behind `handle`.
pub(crate) fn with<R>(
    handle: &str,
    f: impl FnOnce(&mut SessionSlot) -> R,
) -> Result<R, HandleError> {
    if !handle.starts_with("gate:sha256:") {
        return Err(HandleError::Malformed);
    }
    SESSIONS
        .try_with(|sessions| {
            let mut sessions = sessions
                .try_borrow_mut()
                .map_err(|_| HandleError::Unavailable)?;
            let slot = sessions.get_mut(handle).ok_or(HandleError::Unknown)?;
            Ok(f(slot))
        })
        .map_err(|_| HandleError::Unavailable)?
}

/// Forget a session (used by tests; the contract exposes no way to do it).
#[allow(
    dead_code,
    reason = "test-only: the published surface must not let a host drop state"
)]
pub(crate) fn remove(handle: &str) {
    let _ = SESSIONS.try_with(|sessions| {
        if let Ok(mut sessions) = sessions.try_borrow_mut() {
            sessions.remove(handle);
        }
    });
}
