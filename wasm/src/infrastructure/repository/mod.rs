//! Repository adapters — the domain-owned interface, implemented in memory (ADR-002).
//!
//! @canonical .pi/architecture/decisions/ADR-002-data-storage.md
//! Implements: `InMemorySessionStore` (AC: no persistence, no migrations)
//! Issue: #1298 (infrastructure layer)
//!
//! # Why the trait lives in `domain/`
//!
//! The aggregate — not the adapter — decides what reconstitution means, so the interface
//! is owned by `domain/ports/session_store.rs` and this module implements it. There is one
//! definition and no fork; the folder exists because adapters live with adapters.
//!
//! # What is stored
//!
//! The **sealed `ArtifactSet`**, not the session. A session is a live object with a
//! fixture transcript and an unscored outcome; the record is the artifact set. Storing the
//! session would let a host resume one and append a second run to it, which is exactly
//! what invariant 4 forbids (ADR-002).
//!
//! # Contract
//! - No file, no SQL, no migration, no eviction policy
//! - An unsealed set cannot be stored: there is nothing to record yet
//! - A missing id is `None`, never an error — absence is a legitimate answer

use std::collections::HashMap;

use crate::domain::artifact::ArtifactSet;
use crate::domain::errors::SealError;
use crate::domain::ports::SessionStore;
use crate::domain::value::ReplaySessionId;

pub use crate::domain::ports::SessionStore as SessionRepository;

/// Sealed records kept for the life of the host page.
#[derive(Debug, Default)]
pub struct InMemorySessionStore {
    records: HashMap<ReplaySessionId, ArtifactSet>,
}

impl InMemorySessionStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many records are held.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl SessionStore for InMemorySessionStore {
    fn put(&mut self, artifact: &ArtifactSet) -> Result<(), SealError> {
        // Overwriting would silently replace a record with a different one under the same
        // identity; the identity is derived, so a second put for one id is a defect.
        if self.records.contains_key(&artifact.session_id) {
            return Err(SealError::AlreadySealed);
        }
        self.records
            .insert(artifact.session_id.clone(), artifact.clone());
        Ok(())
    }

    fn get(&self, id: &ReplaySessionId) -> Option<&ArtifactSet> {
        self.records.get(id)
    }

    fn remove(&mut self, id: &ReplaySessionId) -> Option<ArtifactSet> {
        self.records.remove(id)
    }
}
