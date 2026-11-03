//! `SessionStore` — the domain-owned repository interface (ADR-002).
//!
//! @canonical .pi/architecture/decisions/ADR-002-data-storage.md
//! @canonical .pi/architecture/decisions/ADR-008-proportional-modeling-depth.md
//! Implements: Contract Freeze — SessionStore
//! Issue: #1294 (contract freeze)
//!
//! The trait is declared here, in `domain/`, because the aggregate — not the adapter —
//! decides what "reconstitute" means. `infrastructure/repository/mod.rs` re-exports it
//! rather than declaring a second one; there is one definition and no fork.
//!
//! There is no database behind it. The shipped adapter is in memory, the record that
//! survives is the exported `ArtifactSet`, and persistence is the host's business.

use crate::domain::errors::SealError;
use crate::domain::session::ReplaySession;
use crate::domain::value::ReplaySessionId;

/// Stores and retrieves sessions by identity.
///
/// # Contract (Frozen)
/// - Synchronous and in-memory for the shipped adapter; no file, no network, no SQL
/// - `put` is called once, when a session is sealed — an unsealed session is not a
///   record and is not persisted
/// - A missing id is `Ok(None)`, never an error: absence is a legitimate answer
pub trait SessionStore {
    /// Save a sealed session.
    fn put(&mut self, session: &ReplaySession) -> Result<(), SealError>;

    /// Retrieve a session by identity.
    fn get(&self, id: &ReplaySessionId) -> Option<&ReplaySession>;

    /// Drop a session. Not part of any production flow; present for tests.
    fn remove(&mut self, id: &ReplaySessionId) -> Option<ReplaySession>;
}
