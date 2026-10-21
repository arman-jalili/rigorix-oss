//! Effect Commitment event payload schemas.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#ddd-layers
//! Implements: ISSUE-CONTRACT-FREEZE — EffectEvent payload schemas
//! Issue: #1113
//!
//! Events describe commitment state changes: an intent was recorded, an effect
//! was settled with evidence, an outcome became unknown, or a resolver settled
//! an ambiguous effect. Consumers correlate them by `record_id` (and `dag_id`
//! for run-scoped views).
//!
//! # Contract (Frozen)
//! - Serialized as a JSON object with a `"type"` discriminator (snake_case)
//! - Each variant carries the full context needed by consumers
//! - No internal implementation details (storage rows, connection handles) leak

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::capacity_ledger::domain::IdempotencyKey;

use super::{EffectRecordId, EffectState, ObservedOutcome, ResolverIdentity};

/// Topic carrying effect-commitment events.
pub const EFFECT_EVENTS_TOPIC: &str = "rigorix.effect-commitment.events";

/// Events emitted by the Effect Commitment context.
///
/// Serialized with a `"type"` tag, e.g.
/// `{"type":"intent_recorded","record_id":"…","dag_id":"…",…}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EffectEvent {
    /// The write-ahead intent was durably recorded before dispatch.
    IntentRecorded {
        /// The new record identity.
        record_id: EffectRecordId,
        /// The execution that proposes the effect.
        dag_id: Uuid,
        /// The node proposing the effect.
        node_id: Uuid,
        /// Attempt counter (diagnostics only).
        attempt: u32,
        /// Intent-derived retry identity.
        idempotency_key: IdempotencyKey,
        /// The dispatched tool name.
        tool: String,
        /// State immediately after recording (always `dispatched`).
        state: EffectState,
    },

    /// The effect was settled with definite provider evidence.
    Settled {
        /// The settled record.
        record_id: EffectRecordId,
        /// Terminal state (`committed` or `not_committed`).
        state: EffectState,
        /// The provider evidence that settled it.
        outcome: ObservedOutcome,
    },

    /// The outcome became unknown (lost response, deadline, crash).
    MarkedAmbiguous {
        /// The unsettled record.
        record_id: EffectRecordId,
        /// Why the outcome is unknown.
        reason: String,
    },

    /// An `Ambiguous` effect was settled by an authorised resolver.
    Resolved {
        /// The resolved record.
        record_id: EffectRecordId,
        /// Terminal state (`committed` or `not_committed`).
        state: EffectState,
        /// The resolver that settled it.
        resolver: ResolverIdentity,
        /// The provider evidence presented.
        outcome: ObservedOutcome,
    },
}
