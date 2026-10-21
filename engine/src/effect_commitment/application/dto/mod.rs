//! Data Transfer Objects for Effect Commitment.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#effectcommitmentservice
//! Implements: ISSUE-CONTRACT-FREEZE — begin/settle/mark/resolve/query DTO schemas
//! Issue: #1113
//!
//! DTOs define the input/output contracts for the application service and the
//! HTTP adapter. They carry documented field names/types but no behavior.
//!
//! # Contract (Frozen)
//! - Every operation has a dedicated input and output DTO
//! - DTOs are JSON-serializable (`snake_case` fields as written)
//! - Outputs always return the full [`EffectRecord`] snapshot; no partial views
//! - Query uses [`EffectQuery`], never an ad-hoc pair of optional ids

use serde::{Deserialize, Serialize};

use crate::effect_commitment::domain::{
    EffectIntent, EffectQuery, EffectRecord, EffectRecordId, ObservedOutcome, ResolverIdentity,
};

// ---------------------------------------------------------------------------
// Begin
// ---------------------------------------------------------------------------

/// Input for recording a write-ahead intent.
///
/// # Fields (Frozen)
/// - `intent: EffectIntent` — the fully-derived intent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeginEffectInput {
    /// The intent to record durably before dispatch.
    pub intent: EffectIntent,
}

/// Output from `begin`.
///
/// # Fields (Frozen)
/// - `record: EffectRecord` — the newly recorded aggregate in `dispatched`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeginEffectOutput {
    /// The newly recorded aggregate.
    pub record: EffectRecord,
}

// ---------------------------------------------------------------------------
// Settle definite
// ---------------------------------------------------------------------------

/// Input for settling a `Dispatched` effect with definite evidence.
///
/// # Fields (Frozen)
/// - `record_id: EffectRecordId` — the effect to settle
/// - `outcome: ObservedOutcome` — the provider evidence
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettleDefiniteInput {
    /// The effect to settle.
    pub record_id: EffectRecordId,

    /// The provider evidence.
    pub outcome: ObservedOutcome,
}

/// Output from `settle_definite`.
///
/// # Fields (Frozen)
/// - `record: EffectRecord` — the terminal snapshot
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettleDefiniteOutput {
    /// The terminal snapshot.
    pub record: EffectRecord,
}

// ---------------------------------------------------------------------------
// Mark ambiguous
// ---------------------------------------------------------------------------

/// Input for marking a dispatched effect ambiguous.
///
/// # Fields (Frozen)
/// - `record_id: EffectRecordId` — the unsettled effect
/// - `reason: String` — why the outcome is unknown
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkAmbiguousInput {
    /// The unsettled effect.
    pub record_id: EffectRecordId,

    /// Why the outcome is unknown.
    pub reason: String,
}

/// Output from `mark_ambiguous`.
///
/// # Fields (Frozen)
/// - `record: EffectRecord` — the `ambiguous` snapshot
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkAmbiguousOutput {
    /// The `ambiguous` snapshot.
    pub record: EffectRecord,
}

// ---------------------------------------------------------------------------
// Resolve
// ---------------------------------------------------------------------------

/// Input for resolving an `Ambiguous` effect.
///
/// # Fields (Frozen)
/// - `record_id: EffectRecordId` — the effect to resolve
/// - `outcome: ObservedOutcome` — provider evidence (never a local guess)
/// - `resolver: ResolverIdentity` — who presents the evidence
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveEffectInput {
    /// The effect to resolve.
    pub record_id: EffectRecordId,

    /// The provider evidence.
    pub outcome: ObservedOutcome,

    /// The resolver presenting it.
    pub resolver: ResolverIdentity,
}

/// Output from `resolve`.
///
/// # Fields (Frozen)
/// - `record: EffectRecord` — the terminal snapshot, carrying evidence + resolver
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveEffectOutput {
    /// The terminal snapshot.
    pub record: EffectRecord,
}

// ---------------------------------------------------------------------------
// Query
// ---------------------------------------------------------------------------

/// Input for querying commitment records.
///
/// # Fields (Frozen)
/// - `query: EffectQuery` — by record id, by DAG, or ambiguous effects
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryEffectInput {
    /// The read request.
    pub query: EffectQuery,
}

/// Output from `query`.
///
/// # Fields (Frozen)
/// - `records: Vec<EffectRecord>` — matching snapshots (empty when none)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryEffectOutput {
    /// Matching snapshots.
    pub records: Vec<EffectRecord>,
}
