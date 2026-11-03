//! Identities of the aggregate and its single entity.
//!
//! @canonical .pi/architecture/modules/portable-session.md#aggregates
//! Implements: Contract Freeze — ReplaySessionId, GateAttemptId
//! Issue: #1294 (contract freeze)
//!
//! Both identities are **derived, never random** (invariant 5): a fresh UUID would
//! make two runs of the same session differ and conformance with the CLI untestable.
//! `ReplaySessionId` is a digest over (scenario, fixtures, seed); `GateAttemptId`
//! comes from the `IdSource` port.

use serde::{Deserialize, Serialize};

use super::Digest;

/// Identity of a replay session: a digest over its pinned inputs.
///
/// # Contract (Frozen)
/// - Equal inputs ⇒ equal id; a different seed ⇒ a different id (criterion 2)
/// - Constructed by `ReplaySession::open`, never by a caller
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReplaySessionId(Digest);

impl ReplaySessionId {
    /// Wrap the derived digest.
    pub fn from_digest(digest: Digest) -> Self {
        Self(digest)
    }

    /// The underlying digest.
    pub fn as_digest(&self) -> &Digest {
        &self.0
    }

    /// Canonical string form, safe to export.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Identity of the one attempt a session may hold, minted by the `IdSource` port.
///
/// # Contract (Frozen)
/// - Opaque to the host; the value is stable for the life of the attempt
/// - Never derived from `Uuid::new_v4`
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GateAttemptId(String);

impl GateAttemptId {
    /// Wrap an id obtained from the `IdSource` port.
    pub fn from_trusted(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// The opaque value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
