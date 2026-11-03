//! `CanonicalSerializer` — one serializer, two runtimes.
//!
//! @canonical .pi/architecture/decisions/ADR-010-artifact-format.md
//! @canonical .pi/architecture/decisions/ADR-009-conformance-harness.md
//! Implements: Contract Freeze — CanonicalSerializer
//! Issue: #1294 (contract freeze)
//!
//! Byte identity is not a property of the crate; it is a property of using the *same*
//! serializer on both sides. This wrapper exists so there is exactly one call site for
//! "turn this into the canonical bytes", and so a second serializer cannot sneak in.
//!
//! # Contract (Frozen)
//! - Canonical form: fixed field order, sorted object keys, no insignificant whitespace
//! - Envelopes are **not** re-serialized: their bytes come from the engine (ADR-010)
//! - Deterministic: the same value always produces the same bytes, in any instance

/// Canonical JSON encoding for the values the crate owns.
#[derive(Debug, Default, Clone, Copy)]
pub struct CanonicalSerializer;

impl CanonicalSerializer {
    /// Construct the serializer.
    pub const fn new() -> Self {
        Self
    }

    /// Encode a value canonically.
    ///
    /// TODO(#1297): delegate to the engine's serializer so both runtimes agree.
    pub fn to_bytes<T: serde::Serialize>(&self, value: &T) -> Result<Vec<u8>, SerializeError> {
        let _ = value;
        todo!("contract freeze: CanonicalSerializer::to_bytes lands with #1297")
    }
}

/// Why canonical encoding failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SerializeError {
    /// The value could not be encoded.
    #[error("the value could not be encoded canonically")]
    Unencodable,
}
