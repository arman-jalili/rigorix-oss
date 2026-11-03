//! `GateError` — the crate's root error type.
//!
//! @canonical .pi/architecture/modules/portable-session.md#error-handling
//! @canonical .pi/architecture/decisions/ADR-012-failure-semantics-on-wasm32.md
//! Implements: Contract Freeze — GateError
//! Issue: #1294 (contract freeze)
//!
//! Every domain error is wrapped here with `#[from]`, the way
//! `rigorix_engine::error::CoreOrchestratorError` aggregates its contexts. No
//! `anyhow`: library code keeps typed errors (engine convention).
//!
//! # Contract (Frozen)
//! - Every sub-error has a variant with `#[from]`
//! - `Display` never carries a secret, a key, an absolute path or personal data
//!   (NFR-003); redaction happens *before* construction, not in `Display`
//! - A `GateError` is always a **tier-1 logical failure** (ADR-012). A trap is not
//!   representable: it aborts the module and is the host's to report.

use thiserror::Error;

use crate::domain::errors::{AttemptError, SealError, SessionOpenError};

/// Root error type for the gate seam.
#[derive(Debug, Error)]
pub enum GateError {
    /// The session could not be opened from the supplied inputs.
    #[error(transparent)]
    SessionOpen(#[from] SessionOpenError),

    /// The run could not be performed on this session.
    #[error(transparent)]
    Attempt(#[from] AttemptError),

    /// The artifact set could not be sealed.
    #[error(transparent)]
    Seal(#[from] SealError),

    /// The request did not parse under the frozen DTO schema.
    #[error("request rejected: {field}")]
    InvalidRequest {
        /// Field that failed, never its value.
        field: &'static str,
    },
}
