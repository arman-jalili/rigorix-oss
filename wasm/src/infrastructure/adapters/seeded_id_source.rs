//! `SeededIdSource` — mints identifiers from declared sequences.
//!
//! @canonical .pi/architecture/modules/portable-session.md#ports-declared-by-the-domain
//! Implements: `IdSource`
//! Issue: #1298 (infrastructure layer)
//!
//! # Contract
//! - Deterministic across runs and instances: the same sequences yield the same ids
//! - Never `Uuid::new_v4` (invariant 5)
//! - A kind with no declared sequence is `Missing`; a used-up sequence is `Exhausted`.
//!   The two are kept apart because they mean different things to a fixture author

use std::collections::HashMap;

use crate::domain::ports::{IdKind, IdSource};
use crate::domain::value::{PortFailure, PortFailureKind, PortId};

/// Answers from the transcript's declared id sequences.
#[derive(Debug, Default)]
pub struct SeededIdSource {
    sequences: HashMap<IdKind, Vec<String>>,
    cursor: HashMap<IdKind, usize>,
}

impl SeededIdSource {
    /// Wrap the declared sequences.
    pub fn new(sequences: HashMap<IdKind, Vec<String>>) -> Self {
        Self {
            sequences,
            cursor: HashMap::new(),
        }
    }
}

impl IdSource for SeededIdSource {
    fn next_id(&mut self, kind: IdKind) -> Result<String, PortFailure> {
        let pos = self.cursor.entry(kind).or_insert(0);
        let next = self
            .sequences
            .get(&kind)
            .and_then(|ids| ids.get(*pos))
            .cloned();
        match next {
            Some(id) => {
                *pos += 1;
                Ok(id)
            }
            None => Err(PortFailure::new(
                PortId::IdSource,
                if self.sequences.contains_key(&kind) {
                    PortFailureKind::Exhausted
                } else {
                    PortFailureKind::Missing
                },
            )),
        }
    }
}
