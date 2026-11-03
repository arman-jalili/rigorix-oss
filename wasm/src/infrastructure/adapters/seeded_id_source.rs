//! `SeededIdSource` — mints identifiers from a declared sequence.
//!
//! @canonical .pi/architecture/modules/portable-session.md#ports-declared-by-the-domain
//! Implements: Contract Freeze — SeededIdSource
//! Issue: #1294 (contract freeze)

use std::collections::HashMap;

use crate::domain::ports::{IdKind, IdSource};
use crate::domain::value::PortFailure;

/// Answers from the transcript's declared id sequences.
///
/// # Contract (Frozen)
/// - Deterministic across runs and instances: the same transcript yields the same ids
/// - Never `Uuid::new_v4` (invariant 5); a sequence that cannot answer is
///   `PortFailure { IdSource, .. }`
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
        let _ = (&self.sequences, &self.cursor, kind);
        todo!("contract freeze: SeededIdSource::next_id lands with #1298")
    }
}
