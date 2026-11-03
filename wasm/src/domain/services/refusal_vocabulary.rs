//! `RefusalVocabulary` — engine refusal kinds to business wording.
//!
//! @canonical .pi/architecture/modules/portable-session.md#domain-services--application-services--infrastructure-services
//! @canonical .pi/architecture/decisions/ADR-007-compliance-engine.md
//! Implements: Contract Freeze — RefusalVocabulary
//! Issue: #1294 (contract freeze)
//!
//! # Contract (Frozen)
//! - The match over [`RefusalKind`] is **exhaustive**: a new kind without wording is a
//!   compile error, never a generic fallback (criterion 8)
//! - The wording is business-facing: it says what did not happen, in the operator's
//!   terms, not in Rust's
//! - `IdentityUnverified` says *"identity could not be checked"*. The words
//!   `unauthenticated` and `forged` are not available to this trait (FR-004)
//! - Every string is static and value-free: no parameter, no path, no identifier from
//!   the host's data (NFR-003)

use crate::domain::outcome::RefusalKind;
use crate::domain::value::PortFailure;

/// Maps a refusal kind to the sentence a business reader gets.
pub trait RefusalVocabulary {
    /// Business wording for a refusal kind.
    fn reason(&self, kind: RefusalKind) -> &'static str;

    /// Business wording for a port failure, which is always a refusal (invariant 2).
    fn port_reason(&self, failure: PortFailure) -> &'static str;
}
