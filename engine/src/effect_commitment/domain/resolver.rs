//! ResolverIdentity — who may settle an `Ambiguous` effect, on what evidence.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#resolveridentity
//! Implements: ISSUE-CONTRACT-FREEZE — resolver authority value object
//! Issue: #1113
//! ADR-019: F3/F4 — only provider evidence, never the party that lost the response
//! ADR-013: resolver evidence must live outside the agent-writable workspace
//!
//! # Contract (Frozen)
//! - The dispatching process and the agent are representable so the policy can
//!   reject them explicitly, not by convention
//! - `is_authorized` is the single predicate the reconciliation policy uses
//! - `evidence_digest` is recorded on the effect so the signed record shows
//!   *who* resolved *on what evidence*

use serde::{Deserialize, Serialize};

/// The kind of authority presenting settlement evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolverKind {
    /// The provider itself, presenting a receipt or read-back.
    Provider,
    /// An authenticated external operator presenting provider evidence.
    ExternalOperator,
    /// The process that dispatched the effect and lost the response. Rejected
    /// by policy (F4): it provably cannot know.
    DispatchingProcess,
    /// The agent that proposed the effect. Rejected by policy (F4).
    Agent,
}

/// The authenticated identity permitted to settle an effect.
///
/// # Fields (Frozen)
/// - `resolver_id: String` — stable identifier of the resolver
/// - `kind: ResolverKind` — the authority class
/// - `evidence_digest: String` — digest of the evidence presented
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolverIdentity {
    /// Stable identifier of the resolver.
    pub resolver_id: String,

    /// The authority class.
    pub kind: ResolverKind,

    /// Digest of the evidence presented.
    pub evidence_digest: String,
}

impl ResolverIdentity {
    /// Construct a resolver identity.
    pub fn new(
        resolver_id: impl Into<String>,
        kind: ResolverKind,
        evidence_digest: impl Into<String>,
    ) -> Self {
        Self {
            resolver_id: resolver_id.into(),
            kind,
            evidence_digest: evidence_digest.into(),
        }
    }

    /// Whether this resolver is permitted to leave `Ambiguous` (F4).
    ///
    /// Only `Provider` and `ExternalOperator` are authorised. The dispatching
    /// process and the agent are rejected because neither can possess evidence
    /// the engine does not already have.
    pub fn is_authorized(&self) -> bool {
        matches!(
            self.kind,
            ResolverKind::Provider | ResolverKind::ExternalOperator
        )
    }
}
