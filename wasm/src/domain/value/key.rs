//! `KeyRef` — an opaque reference to the key that produced a record.
//!
//! @canonical .pi/architecture/decisions/ADR-005-authentication.md
//! Implements: Contract Freeze — KeyRef
//! Issue: #1294 (contract freeze)
//!
//! The artifact records **which** key signed it, never the key. An auditor needs to
//! know what to check the signature against; nobody needs the private material in a
//! record that is meant to be shared.

use serde::{Deserialize, Serialize};

/// An opaque, export-safe key reference.
///
/// # Contract (Frozen)
/// - Never contains key material, a private path or personal data (NFR-003)
/// - Opaque to the host: its shape is the key owner's business, not the contract's
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyRef(String);

impl KeyRef {
    /// Wrap a reference produced by a `KeySource` adapter.
    pub fn from_trusted(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// The opaque value, safe to export.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
