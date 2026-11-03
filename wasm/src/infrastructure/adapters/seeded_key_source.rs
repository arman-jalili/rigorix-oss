//! `SeededKeySource` — an opaque key reference, never key material.
//!
//! @canonical .pi/architecture/decisions/ADR-005-authentication.md
//! Implements: Contract Freeze — SeededKeySource
//! Issue: #1294 (contract freeze)

use crate::domain::ports::KeySource;
use crate::domain::value::{KeyRef, PortFailure, PortFailureKind, PortId};

/// Returns a fixed, opaque `KeyRef`.
///
/// # Contract (Frozen)
/// - Holds a reference only; the demo's key custody is the host's (ADR-005)
/// - A missing reference is `PortFailure { KeySource, .. }`, which the engine reports
///   as `unverified` — never `unauthenticated` (FR-004)
pub struct SeededKeySource {
    key_ref: Option<KeyRef>,
}

impl SeededKeySource {
    /// Wrap an existing reference.
    pub const fn new(key_ref: KeyRef) -> Self {
        Self {
            key_ref: Some(key_ref),
        }
    }

    /// A source that cannot resolve a key, for the fault matrix.
    pub const fn unavailable() -> Self {
        Self { key_ref: None }
    }
}

impl KeySource for SeededKeySource {
    fn key_ref(&self) -> Result<KeyRef, PortFailure> {
        self.key_ref.clone().ok_or(PortFailure::new(
            PortId::KeySource,
            PortFailureKind::Missing,
        ))
    }
}
