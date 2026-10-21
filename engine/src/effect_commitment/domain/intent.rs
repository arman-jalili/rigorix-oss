//! EffectIntent — the write-ahead identity of a proposed effect.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#effectintent
//! Implements: ISSUE-CONTRACT-FREEZE — EffectIntent aggregate identity
//! Issue: #1113
//! ADR-019: the intent-derived key replaces the attempt-derived key
//!
//! Written and made durable **before** the effect is sent. If that write fails,
//! the effect must not be sent: that is the write-ahead requirement and the one
//! that cannot be relaxed.
//!
//! # Contract (Frozen)
//! - `idempotency_key` identifies the **intent**, not the attempt. A retry of
//!   the same intent reuses it verbatim; a changed payload must derive a
//!   different key.
//! - `precondition_digest` and `authority_digest` are produced by the
//!   pre-dispatch gate and are part of the intent digest.
//! - `attempt` is recorded for diagnostics but never participates in key
//!   derivation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::capacity_ledger::domain::IdempotencyKey;

/// The identity of a proposed effect.
///
/// # Fields (Frozen)
/// - `dag_id: Uuid` — the execution/DAG that proposes the effect
/// - `node_id: Uuid` — the node proposing it
/// - `attempt: u32` — attempt counter, diagnostics only
/// - `idempotency_key: IdempotencyKey` — intent-derived retry identity
/// - `tool: String` — the dispatched tool name
/// - `args_digest: String` — digest of the tool arguments
/// - `precondition_digest: String` — digest of the gate's precondition verdict
/// - `authority_digest: String` — digest of the authority the gate accepted
/// - `created_at: DateTime<Utc>` — when the intent was recorded
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectIntent {
    /// The execution/DAG that proposes the effect.
    pub dag_id: Uuid,

    /// The node proposing the effect.
    pub node_id: Uuid,

    /// Attempt counter. Diagnostics only; never part of key derivation.
    pub attempt: u32,

    /// Intent-derived retry identity. Stable across retries of the same intent.
    pub idempotency_key: IdempotencyKey,

    /// The dispatched tool name.
    pub tool: String,

    /// Digest of the tool arguments.
    pub args_digest: String,

    /// Digest of the pre-dispatch gate's precondition verdict.
    pub precondition_digest: String,

    /// Digest of the authority the gate accepted.
    pub authority_digest: String,

    /// When the intent was recorded.
    pub created_at: DateTime<Utc>,
}

impl EffectIntent {
    /// Construct an intent with the current timestamp.
    ///
    /// Callers derive `idempotency_key` with an `IntentKeyDerivation`; it MUST
    /// be stable across retries of the same intent.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        dag_id: Uuid,
        node_id: Uuid,
        attempt: u32,
        idempotency_key: IdempotencyKey,
        tool: impl Into<String>,
        args_digest: impl Into<String>,
        precondition_digest: impl Into<String>,
        authority_digest: impl Into<String>,
    ) -> Self {
        Self {
            dag_id,
            node_id,
            attempt,
            idempotency_key,
            tool: tool.into(),
            args_digest: args_digest.into(),
            precondition_digest: precondition_digest.into(),
            authority_digest: authority_digest.into(),
            created_at: Utc::now(),
        }
    }

    /// The payload digest of the intent.
    ///
    /// `IntentKeyDerivation` hashes `(dag_id, node_id, intent_digest)`. This
    /// value deliberately excludes `attempt` and `created_at`, so a retry of the
    /// same logical effect derives the same key. Components are length-prefixed
    /// before hashing, so changing a field boundary cannot collide with another
    /// intent that happens to use the same separator characters.
    pub fn intent_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"rigorix.effect-commitment.intent-digest.v1");
        for part in [
            &self.tool,
            &self.args_digest,
            &self.precondition_digest,
            &self.authority_digest,
        ] {
            hasher.update((part.len() as u64).to_be_bytes());
            hasher.update(part.as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    /// Whether `other` expresses the same logical intent as this one.
    ///
    /// Two intents are the same when they belong to the same DAG node and have
    /// the same payload digest. The attempt counter and timestamp are ignored:
    /// a retry is the same intent, not a new one. This is the predicate behind
    /// the custody rule (same key + same payload ⇒ read the existing record).
    pub fn same_intent(&self, other: &Self) -> bool {
        self.dag_id == other.dag_id
            && self.node_id == other.node_id
            && self.intent_digest() == other.intent_digest()
    }
}
