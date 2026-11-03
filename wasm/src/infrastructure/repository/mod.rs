//! Repository adapters — re-export of the domain-owned interface (ADR-002).
//!
//! @canonical .pi/architecture/decisions/ADR-002-data-storage.md
//! Implements: Contract Freeze — InMemorySessionStore
//! Issue: #1294 (contract freeze)
//!
//! # Why there is a re-export here
//!
//! The repository **interface** is owned by `domain/ports/session_store.rs`, because the
//! aggregate — not the adapter — decides what reconstitution means (ADR-008). This
//! module exists so the adapter lives where the layer table says adapters live, without
//! a second, forked trait. There is one definition; this is the folder it is implemented
//! in.
//!
//! # Contract (Frozen)
//! - The shipped adapter is in memory: no file, no SQL, no migration (ADR-002)
//! - `put` happens once, at seal; an unsealed session is not a record

use std::collections::HashMap;

use crate::domain::errors::SealError;
use crate::domain::ports::SessionStore;
use crate::domain::session::ReplaySession;
use crate::domain::value::ReplaySessionId;

pub use crate::domain::ports::SessionStore as SessionRepository;

/// Sessions kept for the life of the host page.
///
/// # Contract (Frozen)
/// - No capacity policy, no eviction: a browser session holds one or two records and the
///   host owns how long they live
#[derive(Default)]
pub struct InMemorySessionStore {
    sessions: HashMap<ReplaySessionId, ReplaySession>,
}

impl InMemorySessionStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many sessions are held.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

impl SessionStore for InMemorySessionStore {
    fn put(&mut self, session: &ReplaySession) -> Result<(), SealError> {
        if session.sealed().is_none() {
            return Err(SealError::NoAttempt);
        }
        // ReplaySession is not Clone by contract (an aggregate is moved, not copied);
        // the implementation issue owns the reconstitution strategy.
        let _ = &self.sessions;
        todo!("contract freeze: InMemorySessionStore::put lands with #1299")
    }

    fn get(&self, id: &ReplaySessionId) -> Option<&ReplaySession> {
        self.sessions.get(id)
    }

    fn remove(&mut self, id: &ReplaySessionId) -> Option<ReplaySession> {
        self.sessions.remove(id)
    }
}
