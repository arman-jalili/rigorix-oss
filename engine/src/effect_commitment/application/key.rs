//! IntentKeyDerivation — the intent-derived idempotency key.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#intentkeyderivation
//! Implements: ISSUE-CONTRACT-FREEZE — key derivation port
//! Issue: #1113
//! ADR-019: `IdempotencyKey = H(dag_id, node_id, intent_digest)`
//! ADR-014: effect identity
//!
//! The attempt-derived key at `dispatch.rs:795` makes every retry a new intent
//! and therefore makes the retry fence unenforceable. This key identifies the
//! **intent**:
//!
//! - same key + same payload ⇒ read the existing record (custody)
//! - same key + different payload ⇒ refuse (preserves #1052)
//!
//! # Contract (Frozen)
//! - Deterministic: the same intent always derives the same key
//! - Stable across retries: `attempt` and `created_at` never participate
//! - Changes when any intent digest component changes

use crate::capacity_ledger::domain::IdempotencyKey;
use crate::effect_commitment::domain::EffectIntent;

/// Derives the retry identity of a proposed effect.
pub trait IntentKeyDerivation: Send + Sync {
    /// Derive the intent-derived key for `intent`.
    ///
    /// Implementations MUST hash `(dag_id, node_id, intent_digest)` and MUST NOT
    /// include the attempt counter.
    fn derive_key(&self, intent: &EffectIntent) -> IdempotencyKey;
}
