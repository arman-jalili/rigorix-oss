//! Authority-fact projection — attributable present-state facts from our own
//! signed history (ISSUE-AUTHORITY-FACT-PROJECTION).
//!
//! @canonical docs/spec/consequence-gating-v1.md
//! @canonical .pi/architecture/decisions/ADR-016-audit-integrity-anchor.md
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: ISSUE-AUTHORITY-FACT-PROJECTION (audit-integrity)
//!
//! Rigorix already holds signed, hash-chained records a third party can verify
//! without running the engine and without the operator's key (anchored mode,
//! ADR-016). This module exposes that as a deliberate output: a *fact
//! projection* — what authority was checked, when, with what outcome, with what
//! digests, and at what chain position.
//!
//! # The rule that matters most
//!
//! **The projection never asserts present standing.** It reports what was
//! established and how it can be verified — nothing more. `NOT_ESTABLISHED` is
//! distinct from adverse: the absence of a record is never reported as a
//! non-establishment, and is never promoted to a positive. The claim ceiling is
//! carried as data, so a consumer can read it programmatically.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::audit::domain::envelope::AuditEnvelope;

/// The one-way link from a fact to the record it is attributed to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRef {
    /// The kind of record (e.g. `"precondition_finding"`).
    pub kind: String,
    /// A stable, redaction-safe reference into the record
    /// (`"<execution-id>#<precondition-id>@<sequence>"`).
    pub reference: String,
}

impl EvidenceRef {
    /// Construct a precondition-finding evidence reference.
    pub fn precondition_finding(
        execution_id: uuid::Uuid,
        precondition_id: &str,
        sequence: u64,
    ) -> Self {
        Self {
            kind: "precondition_finding".to_string(),
            reference: format!("{execution_id}#{precondition_id}@{sequence}"),
        }
    }
}

/// The epistemic status of a fact.
///
/// `NotEstablished` means the fact is absent from the record. It is **not** a
/// refusal and SHALL NOT be rendered as one, and SHALL NOT be promoted to a
/// positive (spec §1 NC-4, terminology).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EpistemicStatus {
    /// A record establishes the fact (the check was consulted; its outcome is
    /// recorded).
    Established,
    /// No record establishes the fact. Not adverse; not a refusal.
    NotEstablished,
}

impl EpistemicStatus {
    /// The canonical wire string.
    pub fn as_str(&self) -> &'static str {
        match self {
            EpistemicStatus::Established => "ESTABLISHED",
            EpistemicStatus::NotEstablished => "NOT_ESTABLISHED",
        }
    }
}

/// One attributable present-state fact.
///
/// Every fact carries its source (`evidence_refs`), when it was established
/// (`established_at`), and where it sits in the chain (`chain_position`). It
/// carries an `outcome` and an `authority_digest` **when a record has them** —
/// never an assertion that the authority is presently valid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityFact {
    /// The subject the fact is about (e.g. `"authority:beneficiary-eligible"`).
    /// Redaction-safe: no absolute path, no username, no secret.
    pub subject: String,
    /// The operator's scope token the fact was recorded under.
    pub scope: String,
    /// When the underlying record established the fact.
    pub established_at: DateTime<Utc>,
    /// `ESTABLISHED` or `NOT_ESTABLISHED`.
    pub epistemic_status: EpistemicStatus,
    /// The recorded outcome (`"passed" | "failed" | "error"`), when established.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    /// The one-way digest binding the authority artifact, when the record has
    /// one. Absent ≠ tampered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority_digest: Option<String>,
    /// The records this fact is attributed to. Never empty for `ESTABLISHED`.
    #[serde(default)]
    pub evidence_refs: Vec<EvidenceRef>,
    /// The chain position (sequence) of the record that established the fact.
    pub chain_position: u64,
    /// The producer id of the record, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer_id: Option<String>,
}

impl AuthorityFact {
    /// A fact explicitly recording that **no** record establishes it.
    ///
    /// This is the shape that carries "we looked and there is no record": it is
    /// `NOT_ESTABLISHED`, has no outcome, and is never rendered as a refusal.
    pub fn not_established(subject: impl Into<String>, scope: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            scope: scope.into(),
            established_at: Utc::now(),
            epistemic_status: EpistemicStatus::NotEstablished,
            outcome: None,
            authority_digest: None,
            evidence_refs: Vec::new(),
            chain_position: 0,
            producer_id: None,
        }
    }
}

/// The machine-readable claim ceiling carried by a projection (spec §6).
///
/// Carried **as data**, not prose: a consumer reads it programmatically so a
/// summarizer cannot drop it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimCeiling {
    /// The evidence layer this projection establishes at.
    pub layer: String,
    /// What the projection establishes.
    pub establishes: Vec<String>,
    /// What it explicitly does not.
    pub does_not_establish: Vec<String>,
    /// Always `false`: the projection never asserts present standing.
    pub asserts_present_standing: bool,
}

impl Default for ClaimCeiling {
    fn default() -> Self {
        Self {
            layer: "engineering_verification".to_string(),
            establishes: vec![
                "attributable present-state facts under a stated fixture and version".to_string(),
                "chain position and anchor reference of each fact".to_string(),
            ],
            does_not_establish: vec![
                "present standing of any authority".to_string(),
                "that the operator's authority artifact is externally true".to_string(),
                "compliance, certification, or fitness".to_string(),
                "that the system is safe to deploy".to_string(),
            ],
            asserts_present_standing: false,
        }
    }
}

/// The anchor reference for a projection: the chain tip that lets a third party
/// verify lineage (ADR-016).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorRef {
    /// The highest chain position included.
    pub head_sequence: u64,
    /// The `history_integrity` marker of the tip, when present
    /// (`local_unanchored` / `anchored`).
    pub history_integrity: Option<String>,
    /// The producer the chain belongs to, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer_id: Option<String>,
}

/// A versioned, offline-verifiable projection of authority facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityFactProjection {
    /// Stable identifier.
    pub spec: String,
    /// The projection's version.
    pub version: String,
    /// When the projection was generated.
    pub generated_at: DateTime<Utc>,
    /// The machine-readable claim ceiling.
    pub claim_ceiling: ClaimCeiling,
    /// The anchor reference of the chain the facts came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_ref: Option<AnchorRef>,
    /// The facts.
    pub facts: Vec<AuthorityFact>,
}

/// The stable spec identifier for this projection.
pub const AUTHORITY_FACT_PROJECTION_SPEC: &str = "rigorix-authority-fact-projection";
/// The projection version.
pub const AUTHORITY_FACT_PROJECTION_VERSION: &str = "1.0";

impl AuthorityFactProjection {
    /// Construct a projection with the default claim ceiling.
    pub fn new(
        facts: Vec<AuthorityFact>,
        anchor_ref: Option<AnchorRef>,
        generated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            spec: AUTHORITY_FACT_PROJECTION_SPEC.to_string(),
            version: AUTHORITY_FACT_PROJECTION_VERSION.to_string(),
            generated_at,
            claim_ceiling: ClaimCeiling::default(),
            anchor_ref,
            facts,
        }
    }

    /// The canonical bytes of the projection: JSON with object keys sorted
    /// recursively, so two structurally equal projections serialize byte-exact.
    ///
    /// This is the form an offline verifier checks: recompute the digest and
    /// compare. A tampered projection changes the bytes, hence the digest.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let value = serde_json::to_value(self).unwrap_or(Value::Null);
        let sorted = sort_keys(&value);
        // `to_vec` on a sorted `Value` is deterministic; a serialization failure
        // would be a programming error, and the caller gets empty bytes rather
        // than a panic (the projection is a report, not a gate).
        serde_json::to_vec(&sorted).unwrap_or_default()
    }

    /// One-way `sha256:<hex>` over [`canonical_bytes`](Self::canonical_bytes).
    pub fn digest(&self) -> String {
        use sha2::{Digest, Sha256};
        format!(
            "sha256:{}",
            hex::encode(Sha256::digest(self.canonical_bytes()))
        )
    }
}

/// A projection packaged with the history it was derived from.
///
/// **Why a bundle and not just a projection.** Under the re-derivation design
/// ([`verify_authority_fact_bundle`](crate::audit::application::verify_authority_fact_bundle))
/// a third party does not have to trust a signature: they re-derive the
/// projection from the history and check that it matches. That is a stronger
/// property than "signed by a key holder", because it detects a signer asserting
/// facts the anchored chain does not support.
///
/// The cost is size and disclosure: the bundle carries the envelopes, so the
/// consumer sees the history rather than only the summary. A signed-cache
/// optimisation is documented as deferred on ISSUE-AUTHORITY-FACT-PROJECTION; if
/// it is ever added it **must** be anchored, or the guarantee degrades from
/// "consistent with the anchored chain" to "asserted by a key holder".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorityFactBundle {
    /// The projection under test.
    pub projection: AuthorityFactProjection,
    /// The signed history the projection was derived from.
    pub envelopes: Vec<AuditEnvelope>,
    /// The subjects the consumer asked about that had no record. Carried so that
    /// re-derivation is reproducible: without it the `NOT_ESTABLISHED` facts
    /// cannot be regenerated and the check would silently be incomplete.
    #[serde(default)]
    pub subjects_without_record: Vec<String>,
}

/// Recursively sort object keys so serialization is deterministic.
fn sort_keys(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            let mut out = serde_json::Map::with_capacity(map.len());
            for (key, value) in entries {
                out.insert(key.clone(), sort_keys(value));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(sort_keys).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact() -> AuthorityFact {
        AuthorityFact {
            subject: "authority:beneficiary-eligible".to_string(),
            scope: "payment".to_string(),
            established_at: Utc::now(),
            epistemic_status: EpistemicStatus::Established,
            outcome: Some("passed".to_string()),
            authority_digest: Some("sha256:abc".to_string()),
            evidence_refs: vec![EvidenceRef::precondition_finding(
                uuid::Uuid::nil(),
                "beneficiary-eligible",
                3,
            )],
            chain_position: 3,
            producer_id: Some("local".to_string()),
        }
    }

    #[test]
    fn established_and_not_established_are_distinct_and_never_conflated() {
        let established = fact();
        let absent = AuthorityFact::not_established("authority:x", "scope");
        assert_ne!(established.epistemic_status, absent.epistemic_status);
        assert_eq!(absent.epistemic_status.as_str(), "NOT_ESTABLISHED");
        // Absence is not rendered as an outcome/refusal.
        assert!(absent.outcome.is_none());
        assert!(absent.evidence_refs.is_empty());
    }

    #[test]
    fn canonical_bytes_are_deterministic_and_key_order_independent() {
        let a = AuthorityFactProjection::new(vec![fact()], None, Utc::now());
        let b = a.clone();
        assert_eq!(a.canonical_bytes(), b.canonical_bytes());
        assert_eq!(a.digest(), b.digest());
    }

    #[test]
    fn tampering_changes_the_digest() {
        let original = AuthorityFactProjection::new(vec![fact()], None, Utc::now());
        let mut tampered = original.clone();
        tampered.facts[0].outcome = Some("failed".to_string());
        assert_ne!(original.digest(), tampered.digest());
    }

    #[test]
    fn claim_ceiling_is_machine_readable_and_denies_standing() {
        let projection = AuthorityFactProjection::new(vec![], None, Utc::now());
        assert_eq!(projection.claim_ceiling.layer, "engineering_verification");
        assert!(!projection.claim_ceiling.asserts_present_standing);
        let json = serde_json::to_value(&projection).expect("serialize");
        assert_eq!(json["claim_ceiling"]["asserts_present_standing"], false);
        assert!(json["claim_ceiling"]["does_not_establish"].is_array());
    }
}
