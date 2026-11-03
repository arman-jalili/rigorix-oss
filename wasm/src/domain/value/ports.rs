//! Port identity and typed port failure.
//!
//! @canonical .pi/architecture/modules/portable-session.md#ports-declared-by-the-domain
//! @canonical .pi/architecture/decisions/ADR-012-failure-semantics-on-wasm32.md
//! Implements: Contract Freeze — PortId, PortFailureKind, PortFailure
//! Issue: #1294 (contract freeze)
//!
//! A port failure is a **value**, not an error path: it is recorded as evidence of
//! why the gate refused. That is the difference between "we could not check" and "the
//! check said no", and it must survive into the record.

use serde::{Deserialize, Serialize};

/// Which capability failed to answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortId {
    /// Wall-clock time.
    Clock,
    /// Deterministic identifier minting.
    IdSource,
    /// Entropy (nonces).
    Randomness,
    /// The key that signs envelopes.
    KeySource,
    /// Operator check transport (fixture-driven; never the network).
    ProviderTransport,
    /// The business sink a step would have written to.
    BusinessSink,
}

impl PortId {
    /// The stable wire name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clock => "clock",
            Self::IdSource => "id_source",
            Self::Randomness => "randomness",
            Self::KeySource => "key_source",
            Self::ProviderTransport => "provider_transport",
            Self::BusinessSink => "business_sink",
        }
    }
}

/// Why a port could not answer.
///
/// # Contract (Frozen)
/// - Four kinds only; each maps to an engine-aligned refusal reason
/// - `Failed` is a real answer of "no"; `Missing` is "we never asked successfully";
///   the two are never collapsed (that collapse is what makes an unavailable check
///   look like a passed one)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortFailureKind {
    /// The transcript has no answer for this call.
    Missing,
    /// The transcript's answers for this port are used up.
    Exhausted,
    /// The transcript had an answer that does not parse.
    Malformed,
    /// The port answered with an error.
    Failed,
}

/// A typed port failure: which capability, and how it failed.
///
/// # Contract (Frozen)
/// - Projects to a refusal reason; **never** to an allow (invariant 2)
/// - For `PortId::KeySource` the engine's truthful status is `unverified`; no
///   message may render it `unauthenticated` (criterion 5, FR-004)
/// - Carries no payload, so it cannot leak host data into a refusal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PortFailure {
    /// The capability that failed.
    pub port: PortId,
    /// How it failed.
    pub kind: PortFailureKind,
}

impl PortFailure {
    /// Build a failure value.
    pub const fn new(port: PortId, kind: PortFailureKind) -> Self {
        Self { port, kind }
    }
}

impl std::fmt::Display for PortFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.port.as_str(), self.kind.as_str())
    }
}

impl PortFailureKind {
    /// The stable wire name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Exhausted => "exhausted",
            Self::Malformed => "malformed",
            Self::Failed => "failed",
        }
    }
}
