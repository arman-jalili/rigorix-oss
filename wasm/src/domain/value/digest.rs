//! `Digest` — a one-way `sha256:<hex>` reference to bytes.
//!
//! @canonical .pi/architecture/modules/portable-session.md#value-objects-entity-vs-vo-justification
//! Implements: Contract Freeze — Digest
//! Issue: #1294 (contract freeze)
//!
//! Digests are how the record binds artifacts without carrying them: scenario,
//! fixture transcript, outcome, envelopes. A digest is safe to export; the bytes it
//! names are not necessarily.

use serde::{Deserialize, Serialize};

/// A stable, one-way reference to some bytes.
///
/// # Contract (Frozen)
/// - Wire format is exactly `sha256:` followed by 64 lowercase hex characters
/// - Construction validates the shape; an invalid digest is a parse refusal, never
///   a default (FR-001)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Digest(String);

impl Digest {
    /// The canonical prefix.
    pub const PREFIX: &'static str = "sha256:";

    /// Parse a wire-format digest.
    ///
    /// TODO(#1294): validate prefix, length and hex alphabet.
    pub fn parse(raw: impl Into<String>) -> Result<Self, DigestError> {
        let _raw = raw.into();
        todo!("contract freeze: Digest::parse lands with domain value objects")
    }

    /// Reconstruct from a known-good value (adapter boundary only).
    pub fn from_trusted(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// The wire format.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a digest could not be accepted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DigestError {
    /// The value did not carry the `sha256:` prefix.
    #[error("digest is missing the sha256: prefix")]
    MissingPrefix,
    /// The value was not 64 lowercase hex characters.
    #[error("digest body is not 64 lowercase hex characters")]
    Malformed,
}

impl TryFrom<String> for Digest {
    type Error = DigestError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<Digest> for String {
    fn from(value: Digest) -> Self {
        value.0
    }
}
