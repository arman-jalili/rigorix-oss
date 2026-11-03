//! Error projection — typed domain errors to `RefusalDto`.
//!
//! @canonical .pi/architecture/modules/portable-session.md#error-handling
//! @canonical .pi/architecture/decisions/ADR-012-failure-semantics-on-wasm32.md
//! Implements: Contract Freeze — the projection mapping
//! Issue: #1294 (contract freeze)
//!
//! # Contract (Frozen)
//! - One arm per domain error variant: adding a variant without a mapping is a compile
//!   error, so no error can escape unmapped into a host message
//! - The mapping is where redaction is *checked*: the message comes from
//!   [`RefusalVocabulary`](crate::domain::services::RefusalVocabulary), not from
//!   `Display` of the error

pub use super::dto::RefusalDto;

/// Maps a domain error to the wire refusal.
///
/// TODO(#1300): implement with an exhaustive match and the vocabulary.
pub fn project_session_open(_error: &crate::domain::errors::SessionOpenError) -> RefusalDto {
    todo!("contract freeze: error projection lands with #1300")
}

/// Maps an attempt error to the wire refusal.
pub fn project_attempt(_error: &crate::domain::errors::AttemptError) -> RefusalDto {
    todo!("contract freeze: error projection lands with #1300")
}

/// Maps a seal error to the wire refusal.
pub fn project_seal(_error: &crate::domain::errors::SealError) -> RefusalDto {
    todo!("contract freeze: error projection lands with #1300")
}
