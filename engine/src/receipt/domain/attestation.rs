//! The attestation: what the engine observed, signed (ADR-024 §3 D2, §4).
//!
//! The asymmetry with [`super::receipt`] is the whole point. A receipt is what a host says; an
//! attestation is what the engine *saw*, so its constructor lives beside the engine's own builder
//! ([`super::super::application::service::AttestationBuilder`]) and nowhere else:
//!
//! - there is no `Default`, no public `new`, and no `Deserialize` — the only way to obtain one from
//!   bytes is [`verify_attestation`], which requires a valid signature over the canonical form;
//! - every field is read from something this execution did: a per-step observation, the settled
//!   effect ids, the receipt it answers, the execution it belongs to;
//! - a value the execution did **not** observe carries `source: Caller`. There is no default that
//!   reads as success — the failure mode this module exists to prevent is an attestation that says
//!   "merge-ready" because nobody filled in the field.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::quality_gates::domain::level::QualityLevel;

use super::digest::{canonical_json, sha256_hex};

/// The quality level recorded, and who produced it (ADR-024 §2 D1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityRecord {
    /// The level.
    pub level: QualityLevel,
    /// Where it came from — and which step, when the engine produced it.
    pub source: QualitySource,
}

/// Where a recorded quality level came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QualitySource {
    /// A step in **this** execution produced it. The step id is recorded so the claim is traceable.
    Engine {
        /// The step that established the level.
        step_id: String,
    },
    /// The caller declared it. Recorded as a claim, and never as a pass.
    Caller,
}

/// A path a step wrote, and the digest of what it wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedEffect {
    /// The path.
    pub path: String,
    /// The digest of the content the engine found there afterwards.
    pub content_digest: String,
}

/// What one step did, as the engine observed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedStep {
    /// The step.
    pub step_id: String,
    /// What it wrote.
    pub observed_effects: Vec<ObservedEffect>,
    /// Whether its declared expectation was met, and why not when it was not.
    pub expectation_met: bool,
    /// The reason, when it was not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expectation_detail: Option<String>,
}

/// The budget as declared and as used (ADR-024 §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetEcho {
    /// Iterations the receipt allowed.
    pub declared_iterations: u32,
    /// Tokens the receipt allowed.
    pub declared_tokens: u64,
    /// Iterations the execution used.
    pub observed_iterations: u32,
    /// Tokens the execution used.
    pub observed_tokens: u64,
    /// Always `false` in this version: nothing here refuses a dispatch on budget. The field exists so
    /// a reader is not left to infer it — a report that omits its own limits reads as enforced.
    pub enforced: bool,
}

/// The signed observation of one execution.
///
/// Constructed only by `AttestationBuilder`; see the module docs for why that is enforced by the
/// type's surface rather than by a comment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Attestation {
    /// The schema version, first in the canonical bytes.
    schema_version: u16,
    /// The digest of the receipt this answers.
    receipt_digest: String,
    /// The execution.
    execution_id: Uuid,
    /// What each step did.
    steps: Vec<ObservedStep>,
    /// The quality record.
    quality: QualityRecord,
    /// The settled effect records.
    settled_effects: Vec<String>,
    /// The identity — redacted at construction by the caller's own helpers.
    identity: Option<String>,
    /// What the budget was and what it did.
    budget: BudgetEcho,
    /// The HMAC over the canonical body, absent when no key is configured (signing is never
    /// silently simulated).
    signature: Option<String>,
}

impl Attestation {
    /// The digest of the receipt this attestation answers.
    #[must_use]
    pub fn receipt_digest(&self) -> &str {
        &self.receipt_digest
    }

    /// The execution.
    #[must_use]
    pub fn execution_id(&self) -> Uuid {
        self.execution_id
    }

    /// What each step did.
    #[must_use]
    pub fn steps(&self) -> &[ObservedStep] {
        &self.steps
    }

    /// The quality record.
    #[must_use]
    pub fn quality(&self) -> &QualityRecord {
        &self.quality
    }

    /// The settled effect ids.
    #[must_use]
    pub fn settled_effects(&self) -> &[String] {
        &self.settled_effects
    }

    /// The budget echo.
    #[must_use]
    pub fn budget(&self) -> BudgetEcho {
        self.budget
    }

    /// Whether the attestation carries a signature.
    #[must_use]
    pub fn is_signed(&self) -> bool {
        self.signature.is_some()
    }

    /// The canonical bytes that are signed — the body, without the signature itself.
    ///
    /// # Errors
    /// The canonical-form error, which would be a defect.
    pub fn canonical_body(&self) -> Result<Vec<u8>, String> {
        let mut body = self.clone();
        body.signature = None;
        canonical_json(&body)
    }

    /// The attestation's own digest (over the canonical body), for a store that pins it.
    ///
    /// # Errors
    /// The canonical-form error.
    pub fn digest(&self) -> Result<String, String> {
        Ok(sha256_hex(&self.canonical_body()?))
    }
}

/// An attestation whose signature has been checked against the bytes it was given.
///
/// The newtype is the only proof: [`verify_attestation`] returns it, and nothing else can.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedAttestation(Attestation);

impl VerifiedAttestation {
    /// The verified attestation.
    #[must_use]
    pub fn attestation(&self) -> &Attestation {
        &self.0
    }
}

/// Verify an attestation's signature over the bytes it carries.
///
/// # Errors
/// The reason the bytes do not verify, as a string: malformed JSON, missing signature, or a
/// signature that does not match. All three are refusals — a byte string that cannot be verified is
/// not an attestation.
pub fn verify_attestation(bytes: &[u8], key: &[u8]) -> Result<VerifiedAttestation, String> {
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::Sha256;
    let wire: Wire =
        serde_json::from_slice(bytes).map_err(|e| format!("not an attestation: {e}"))?;
    let attestation = Attestation::from(wire);
    let signature = attestation
        .signature
        .clone()
        .ok_or_else(|| "the attestation is not signed".to_string())?;
    let body = attestation.canonical_body()?;
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key)
        .map_err(|_| "the key cannot key HMAC-SHA256".to_string())?;
    mac.update(&body);
    let expected: String = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if expected != signature {
        return Err(format!(
            "signature does not match: the attestation was altered or signed with another key \
             (recorded {signature}, computed {expected})"
        ));
    }
    Ok(VerifiedAttestation(attestation))
}

/// Engine-internal construction. Not `pub` outside the crate: `pub(crate)` methods are the only way
/// to build an `Attestation`, and the builder asserts the invariants the type documents.
impl Attestation {
    /// Build one. Crate-private on purpose (ADR-024 §4, #1527's constructor-privacy criterion).
    pub(crate) fn assemble(
        receipt_digest: String,
        execution_id: Uuid,
        steps: Vec<ObservedStep>,
        quality: QualityRecord,
        settled_effects: Vec<String>,
        identity: Option<String>,
        budget: BudgetEcho,
    ) -> Self {
        Self {
            schema_version: super::receipt::RECEIPT_SCHEMA_VERSION,
            receipt_digest,
            execution_id,
            steps,
            quality,
            settled_effects,
            identity,
            budget,
            signature: None,
        }
    }

    /// Set the signature. Crate-private: only the builder signs.
    pub(crate) fn set_signature(&mut self, signature: String) {
        self.signature = Some(signature);
    }
}

/// The wire form — the *only* deserializable shape, and it is not [`Attestation`].
///
/// A criterion of #1527 is that `grep -rn 'impl.*Deserialize.*for Attestation' engine/src` finds
/// nothing: a host that can deserialize an attestation can mint "the engine said so". So the
/// `Deserialize` impl belongs to this mirror, which lives in the same module, is only reachable
/// through [`verify_attestation`], and is converted into `Attestation` after the signature checks.
/// The serialized form is identical, which is why the mirror is worth having at all.
#[derive(Debug, Serialize, Deserialize)]
struct Wire {
    schema_version: u16,
    receipt_digest: String,
    execution_id: Uuid,
    steps: Vec<ObservedStep>,
    quality: QualityRecord,
    settled_effects: Vec<String>,
    identity: Option<String>,
    budget: BudgetEcho,
    signature: Option<String>,
}

impl From<Wire> for Attestation {
    fn from(wire: Wire) -> Self {
        Self {
            schema_version: wire.schema_version,
            receipt_digest: wire.receipt_digest,
            execution_id: wire.execution_id,
            steps: wire.steps,
            quality: wire.quality,
            settled_effects: wire.settled_effects,
            identity: wire.identity,
            budget: wire.budget,
            signature: wire.signature,
        }
    }
}
