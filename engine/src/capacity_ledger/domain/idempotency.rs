//! IdempotencyKey domain type.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#idempotencykey
//! Implements: ISSUE-CONTRACT-FREEZE — IdempotencyKey retry identity
//! Issue: #1021
//!
//! Retry identity. Part of the reserve request so that a retried reservation
//! cannot create a second one *by construction*. In a distributed system,
//! network retries otherwise become the over-spend.
//!
//! # Contract (Frozen)
//! - Equality is exact and case-sensitive (no normalization)
//! - Two reserves with the same key MUST resolve to one reservation (I2)

use std::fmt;

use serde::{Deserialize, Serialize};

/// Retry identity for a reserve operation.
///
/// Callers are responsible for generating a fresh key per logical reservation
/// and reusing it verbatim across retries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    /// Construct a key from an opaque token.
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// Derive a key from a UUID (canonical hyphenated form).
    pub fn from_uuid(id: uuid::Uuid) -> Self {
        Self(id.to_string())
    }

    /// Borrow the underlying token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IdempotencyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
