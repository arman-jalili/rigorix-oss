//! `IdSource` — deterministic identifier minting.
//!
//! @canonical .pi/architecture/modules/portable-session.md#ports-declared-by-the-domain
//! Implements: Contract Freeze — IdSource, IdKind
//! Issue: #1294 (contract freeze)

use crate::domain::value::PortFailure;

/// Which kind of identifier is being minted.
///
/// # Contract (Frozen)
/// - The kind selects a declared sequence; it is not a name the caller may invent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdKind {
    /// The session's single attempt.
    Attempt,
    /// A step inside the attempt, for envelope correlation.
    Step,
    /// An envelope identifier.
    Envelope,
}

/// Mints identifiers from a declared sequence.
///
/// # Contract (Frozen)
/// - Deterministic: the same sequence yields the same ids across runs
/// - A sequence that cannot answer is `PortFailure { port: IdSource, .. }`; the crate
///   never falls back to `Uuid::new_v4` (invariant 5)
pub trait IdSource {
    /// Mint the next identifier of `kind`.
    fn next_id(&mut self, kind: IdKind) -> Result<String, PortFailure>;
}
