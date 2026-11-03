//! `EvidenceRef` — a stable reference to the facts a record carries.
//!
//! @canonical .pi/architecture/modules/portable-session.md#value-objects-entity-vs-vo-justification
//! Implements: Contract Freeze — EvidenceRef, EvidenceKind
//! Issue: #1294 (contract freeze)
//!
//! Evidence is *borrowed* from the engine: the crate never mints a determination, it
//! only points at one. A reference is a digest, so exporting it cannot leak the facts
//! themselves.

use serde::{Deserialize, Serialize};

use crate::domain::value::Digest;

/// What kind of fact a reference points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    /// A precondition determination.
    Precondition,
    /// A sequence-policy finding.
    SequencePolicy,
    /// A step-requirement finding.
    Requirement,
    /// An approval binding.
    Approval,
    /// An identity attestation.
    Identity,
    /// A capability port's failure.
    PortFailure,
}

/// A reference to evidence the engine produced.
///
/// # Contract (Frozen)
/// - Carries a digest and a kind; never the evidence body
/// - Never constructed by the crate from its own observation, except for
///   `EvidenceKind::PortFailure`, which the crate did observe
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRef {
    /// Digest of the evidence as the engine produced it.
    pub digest: Digest,
    /// What kind of fact this is.
    pub kind: EvidenceKind,
}
