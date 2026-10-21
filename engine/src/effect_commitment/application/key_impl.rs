//! Sha256IntentKeyDerivation — the concrete intent-derived key.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#intentkeyderivation
//! Implements: ISSUE-EFFECT-COMMITMENT-8 — concrete key derivation
//! Issue: #1121
//! ADR-019: `IdempotencyKey = H(dag_id, node_id, intent_digest)`
//! ADR-014: effect identity
//!
//! The attempt-derived key at `dispatch.rs:795` makes every retry a new intent
//! and therefore makes the retry fence unenforceable. This derivation is stable
//! across retries of the same intent and changes when the payload changes.
//!
//! # Contract (Frozen)
//! - Domain-separated SHA-256 over `(dag_id, node_id, intent_digest)`
//! - The `attempt` counter and `created_at` never participate
//! - The output is lowercase hex, fixed length (64 characters)

use sha2::{Digest, Sha256};

use crate::capacity_ledger::domain::IdempotencyKey;
use crate::effect_commitment::domain::EffectIntent;

use super::key::IntentKeyDerivation;

/// The default derivation: SHA-256 over the intent identity.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Sha256IntentKeyDerivation;

impl Sha256IntentKeyDerivation {
    /// Construct the derivation.
    pub const fn new() -> Self {
        Self
    }
}

impl IntentKeyDerivation for Sha256IntentKeyDerivation {
    fn derive_key(&self, intent: &EffectIntent) -> IdempotencyKey {
        let mut hasher = Sha256::new();
        hasher.update(b"rigorix.effect-commitment.intent-key.v1");
        // UUIDs are fixed-length, so only the variable payload needs framing.
        hasher.update(intent.dag_id.as_bytes());
        hasher.update(intent.node_id.as_bytes());
        hasher.update(intent.intent_digest().as_bytes());
        IdempotencyKey::new(hex::encode(hasher.finalize()))
    }
}
