//! IntentHash — deterministic digest binding the approval to the exact
//! dispatch payload.
//!
//! @canonical .pi/architecture/modules/approval.md#intenthash
//! Implements: Contract Freeze — IntentHash digest wrapper
//! Issue: #786 (approval epic — contract freeze); behavior in
//!   ISSUE-INTENTHASH (#788)
//!
//! `intent_hash = HMAC-SHA256(run_key, canonical_serialize(tool ‖ intent ‖
//! declared_scope))` — computed at approval time (mid-run) so the envelope's
//! end-of-run signature covers the approval records transitively.
//!
//! # Contract (Frozen)
//! - Same tool + intent + scope → same hash (deterministic)
//! - Any byte change in the canonical intent → different hash
//! - The run key is the same key used for the envelope HMAC (ADR-011 §key)
//! - The hash is a `String` newtype — serializable and comparable
//!
//! **Implementation note:** `compute`/`verify` bodies are `todo!()` stubs;
//! the HMAC-SHA256 logic lands in ISSUE-INTENTHASH.

use serde::{Deserialize, Serialize};

use super::intent::ExecutionIntent;

/// HMAC-SHA256 digest over the canonical intent serialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentHash(pub String);

impl IntentHash {
    /// Compute the approval digest over a canonical intent with the run key.
    ///
    /// # Implementation
    /// TODO: implemented in ISSUE-INTENTHASH (#788).
    pub fn compute(_intent: &ExecutionIntent, _run_key: &[u8]) -> Self {
        todo!("ISSUE-INTENTHASH (#788): HMAC-SHA256 over canonical_bytes")
    }

    /// Verify an intent still matches this recorded digest.
    ///
    /// # Implementation
    /// TODO: implemented in ISSUE-INTENTHASH (#788).
    pub fn verify(&self, _intent: &ExecutionIntent, _run_key: &[u8]) -> bool {
        todo!("ISSUE-INTENTHASH (#788): constant-time comparison")
    }
}
