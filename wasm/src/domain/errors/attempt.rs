//! `AttemptError` — the run could not be performed.
//!
//! @canonical .pi/architecture/modules/portable-session.md#error-handling
//! Implements: Contract Freeze — AttemptError
//! Issue: #1294 (contract freeze)

use thiserror::Error;

use crate::domain::engine_seam::EngineBindingError;

/// Why an attempt could not be performed.
///
/// # Contract (Frozen)
/// - `AlreadyAttempted` is the one-attempt invariant (criterion 3); the first outcome
///   is untouched by the second call
/// - A port failure inside a run is **not** this error: it is a `RunOutcome` with
///   status `Refused` (invariant 2). This type is for calls that cannot produce a
///   decision at all
/// - `EngineFailure` refuses; it never allows
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AttemptError {
    /// This session already ran its one attempt.
    #[error("this session has already run its one attempt")]
    AlreadyAttempted,

    /// The session was sealed; nothing may be appended.
    #[error("this session is sealed and cannot run another attempt")]
    AlreadySealed,

    /// The engine binding could not evaluate the run.
    #[error("the gate could not evaluate the run: {source}")]
    EngineFailure {
        /// Why the binding failed.
        #[source]
        source: EngineBindingError,
    },
}
