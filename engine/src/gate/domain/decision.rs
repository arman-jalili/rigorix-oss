//! The decision: the verdict, its findings, and the digest that binds them.
//!
//! @canonical .pi/architecture/modules/gate.md

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::binding::{GateEnvelopeBinding, GateEnvelopeEvidence};
use super::finding::GateFinding;
use super::verdict::GateVerdict;

/// What the gate answers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateDecision {
    /// May the step dispatch.
    pub verdict: GateVerdict,
    /// Every finding, ordered exactly as the gates were evaluated.
    pub findings: Vec<GateFinding>,
    /// The identity a signer binds.
    pub envelope: GateEnvelopeBinding,
    /// The signed record, when a [`RunContext`] was supplied.
    ///
    /// Assembled here rather than by each runtime, so the CLI and the crate cannot
    /// produce matching digests over differing signed bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envelope_evidence: Option<GateEnvelopeEvidence>,
}

impl PartialEq for GateDecision {
    /// Two decisions are the same decision when the verdict, the findings and — if a record
    /// was built — the canonical bytes and hash agree. Comparing the envelope's fields one
    /// by one would make equality a property of the factory's internals rather than of what
    /// a verifier sees.
    fn eq(&self, other: &Self) -> bool {
        let evidence_eq = match (&self.envelope_evidence, &other.envelope_evidence) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                a.canonical_bytes == b.canonical_bytes && a.envelope_hash == b.envelope_hash
            }
            _ => false,
        };
        self.verdict == other.verdict
            && self.findings == other.findings
            && self.envelope == other.envelope
            && evidence_eq
    }
}

impl GateDecision {
    /// Whether the step may dispatch.
    #[must_use]
    pub fn permits_dispatch(&self) -> bool {
        self.verdict.permits_dispatch()
    }
}

/// SHA-256 (hex) of the decision's canonical bytes.
///
/// The bytes are `[verdict, findings]` in serde declaration order. Nothing
/// ambient — no `Utc::now()`, no hash-map iteration — enters them, so two calls with
/// the same request produce the same digest. That is what `determinism_test` asserts,
/// and what makes the digest worth signing.
#[must_use]
pub fn decision_digest(verdict: &GateVerdict, findings: &[GateFinding]) -> String {
    let canonical = serde_json::to_vec(&(verdict, findings)).unwrap_or_else(|_| Vec::new());
    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
