//! `SealError` — the artifact set could not be sealed.
//!
//! @canonical .pi/architecture/modules/portable-session.md#error-handling
//! Implements: Contract Freeze — SealError
//! Issue: #1294 (contract freeze)

use thiserror::Error;

use crate::domain::services::ContentRule;
use crate::domain::value::PortFailure;

/// Why a session could not be sealed.
///
/// # Contract (Frozen)
/// - `ContentRejected` refuses the export; the bytes are dropped, never sanitised
///   (criterion 7)
/// - `AlreadySealed` leaves the first artifact set byte-identical (criterion 6)
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SealError {
    /// Nothing was attempted, so there is nothing to seal.
    #[error("this session has no attempt to seal")]
    NoAttempt,

    /// The session is already sealed.
    #[error("this session is already sealed")]
    AlreadySealed,

    /// The assembled set violated a content rule.
    #[error("the artifact set was rejected: {rule}")]
    ContentRejected {
        /// Which rule failed.
        rule: ContentRule,
    },

    /// A capability could not answer while sealing.
    #[error("capability {failure} could not answer while sealing")]
    PortFailure {
        /// Which capability, and how it failed.
        failure: PortFailure,
    },
}
