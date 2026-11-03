//! `KeySource` — the key that signs, referenced but never disclosed.
//!
//! @canonical .pi/architecture/decisions/ADR-005-authentication.md
//! Implements: Contract Freeze — KeySource
//! Issue: #1294 (contract freeze)

use crate::domain::value::{KeyRef, PortFailure};

/// Provides the opaque reference of the key that will sign the envelopes.
///
/// # Contract (Frozen)
/// - Returns a `KeyRef`, never key bytes, never a filesystem path
/// - A key that cannot be resolved is `PortFailure { port: KeySource, .. }` and the
///   run is refused; the engine's status for this case is `unverified`, never
///   `unauthenticated` (FR-004)
/// - `&self`: the reference is stable for the session, so it is not a consuming call
pub trait KeySource {
    /// The reference of the key that will produce the record.
    fn key_ref(&self) -> Result<KeyRef, PortFailure>;
}
