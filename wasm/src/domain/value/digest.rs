//! `Digest` — a one-way `sha256:<hex>` reference to bytes.
//!
//! @canonical .pi/architecture/modules/portable-session.md#value-objects-entity-vs-vo-justification
//! Implements: Contract Freeze — Digest; AC 2 (derived session identity)
//! Issue: #1295
//!
//! Digests are how the record binds artifacts without carrying them: scenario,
//! fixture transcript, outcome, envelopes. A digest is safe to export; the bytes it
//! names are not necessarily.
//!
//! The hash is `sha2`, the same crate the engine uses. That is a byte-identity
//! requirement rather than a taste: a second implementation would produce different
//! digests for the same bytes and break conformance with the CLI (ADR-009, ADR-010).

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

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

    /// Number of hex characters in the body.
    const BODY_LEN: usize = 64;

    /// Hash bytes into their canonical reference.
    ///
    /// Pure: no clock, no entropy, no allocation beyond the string. Same bytes in, same
    /// digest out, in any process and on any target.
    pub fn of(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let out = hasher.finalize();

        let mut hex = String::with_capacity(Self::BODY_LEN);
        for byte in out {
            use std::fmt::Write as _;
            // Writing into a String cannot fail; the result is ignored deliberately.
            let _ = write!(hex, "{byte:02x}");
        }
        Self(format!("{}{hex}", Self::PREFIX))
    }

    /// Parse a wire-format digest.
    ///
    /// # Contract (Frozen)
    /// - Prefix and alphabet are checked; a malformed digest is a refusal, never a
    ///   value the record then carries (FR-001)
    pub fn parse(raw: impl Into<String>) -> Result<Self, DigestError> {
        let raw = raw.into();
        let body = raw
            .strip_prefix(Self::PREFIX)
            .ok_or(DigestError::MissingPrefix)?;

        if body.len() != Self::BODY_LEN {
            return Err(DigestError::Malformed);
        }
        if !body
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            // Uppercase hex is rejected on purpose: two spellings of one hash would
            // make two artifacts look different for the same bytes.
            return Err(DigestError::Malformed);
        }
        Ok(Self(raw))
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
