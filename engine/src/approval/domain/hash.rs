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
//! Implemented in ISSUE-INTENTHASH (#788).

use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use super::intent::ExecutionIntent;

type HmacSha256 = Hmac<Sha256>;

/// HMAC-SHA256 digest over the canonical intent serialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentHash(pub String);

impl IntentHash {
    /// Compute the approval digest over a canonical intent with the run key.
    ///
    /// ```text
    /// intent_hash = hex(HMAC-SHA256(run_key, canonical_bytes(intent)))
    /// ```
    ///
    /// Deterministic by construction: identical canonical intent bytes and key
    /// produce the identical digest; any byte change alters it. The run key is
    /// the same key used for the envelope HMAC (ADR-011 §key).
    pub fn compute(intent: &ExecutionIntent, run_key: &[u8]) -> Self {
        let mut mac = HmacSha256::new_from_slice(run_key).expect("HMAC accepts any key length");
        mac.update(&intent.canonical_bytes());
        let digest = mac.finalize().into_bytes();
        IntentHash(hex::encode(digest))
    }

    /// Verify an intent still matches this recorded digest.
    ///
    /// Recomputes the digest over the supplied intent and compares in
    /// constant time. Returns `false` for a tampered intent, a different run
    /// key, or a malformed stored digest.
    pub fn verify(&self, intent: &ExecutionIntent, run_key: &[u8]) -> bool {
        let Ok(expected) = hex::decode(&self.0) else {
            return false;
        };
        let mut mac = match HmacSha256::new_from_slice(run_key) {
            Ok(mac) => mac,
            Err(_) => return false,
        };
        mac.update(&intent.canonical_bytes());
        mac.verify_slice(&expected).is_ok()
    }
}
