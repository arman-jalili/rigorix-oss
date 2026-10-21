//! ProviderReceiptReader — the L3 anti-corruption port.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#providerreceiptreader
//! Implements: ISSUE-CONTRACT-FREEZE — provider read-back port
//! Issue: #1113
//! ADR-019: commitment authority is external to the engine
//!
//! The anti-corruption boundary over a foreign payment system: read back a
//! receipt and produce an [`ObservedOutcome`]. This is the L3 boundary.
//!
//! # Contract (Frozen)
//! - Returns evidence, never a boolean guess
//! - Every transport ambiguity maps to [`CommitmentError::Unavailable`]
//! - Without an implementation the context can prove **not-settled** only
//! - Credentials are never logged or rendered by `Debug`

use async_trait::async_trait;

use crate::effect_commitment::domain::{CommitmentError, EffectIntent, ObservedOutcome};

/// Reads provider evidence for a dispatched effect.
#[async_trait]
pub trait ProviderReceiptReader: Send + Sync {
    /// Read the provider's evidence for `intent`.
    ///
    /// `receipt_id` is the provider receipt to read; `None` asks the adapter to
    /// resolve by the intent's idempotency key.
    async fn observe(
        &self,
        intent: &EffectIntent,
        receipt_id: Option<&str>,
    ) -> Result<ObservedOutcome, CommitmentError>;
}
