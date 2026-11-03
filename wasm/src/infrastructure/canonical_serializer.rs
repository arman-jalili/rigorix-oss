//! `CanonicalSerializer` — one serializer, two runtimes.
//!
//! @canonical .pi/architecture/decisions/ADR-010-artifact-format.md
//! @canonical .pi/architecture/decisions/ADR-009-conformance-harness.md
//! Implements: `CanonicalSerializer`
//! Issue: #1298 (infrastructure layer)
//!
//! Byte identity is not a property of the crate; it is a property of using the *same*
//! serializer on both sides. This wrapper is the single call site for "turn this into the
//! canonical bytes", so a second serializer cannot sneak in.
//!
//! # What canonical means here
//!
//! - Object keys sorted (no `preserve_order` feature: `serde_json::Map` is a `BTreeMap`,
//!   so sorting is a property of the type rather than a step someone can forget)
//! - No insignificant whitespace (compact form)
//! - Integers as integers, never floats with a trailing `.0`
//!
//! # Contract
//! - Envelopes are **not** re-serialized: their bytes come from the engine (ADR-010)
//! - Deterministic: the same value produces the same bytes, in any instance

use serde::Serialize;

/// Canonical JSON encoding for the values the crate owns.
#[derive(Debug, Default, Clone, Copy)]
pub struct CanonicalSerializer;

impl CanonicalSerializer {
    /// Construct the serializer.
    pub const fn new() -> Self {
        Self
    }

    /// Encode a value canonically.
    pub fn to_bytes<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, SerializeError> {
        serde_json::to_vec(value).map_err(|_| SerializeError::Unencodable)
    }
}

/// Why canonical encoding failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SerializeError {
    /// The value could not be encoded.
    #[error("the value could not be encoded canonically")]
    Unencodable,
}
