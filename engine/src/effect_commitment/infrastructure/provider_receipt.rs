//! HttpProviderReceiptReader — the L3 adapter stub.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#providerreceiptreader
//! Implements: ISSUE-CONTRACT-FREEZE — provider read-back adapter config
//! Issue: #1113
//!
//! The anti-corruption adapter over a foreign payment system. It maps the
//! provider's read-back onto [`ObservedOutcome`]; it never invents a verdict.
//!
//! # Status
//! ISSUE-EFFECT-COMMITMENT-12 supplies the implementation. Until then the
//! adapter fails closed: without provider evidence the context can prove
//! **not-settled** only, and pretending otherwise would be the claim ceiling
//! violation ADR-019 forbids.

use async_trait::async_trait;

use crate::effect_commitment::application::config::ProviderReceiptConfig;
use crate::effect_commitment::application::receipt::ProviderReceiptReader;
use crate::effect_commitment::domain::{CommitmentError, EffectIntent, ObservedOutcome};

/// HTTP read-back adapter for a provider's receipt API.
pub struct HttpProviderReceiptReader {
    config: ProviderReceiptConfig,
}

impl HttpProviderReceiptReader {
    /// Create an adapter from its frozen config.
    pub fn new(config: ProviderReceiptConfig) -> Self {
        Self { config }
    }

    /// The active configuration.
    pub fn config(&self) -> &ProviderReceiptConfig {
        &self.config
    }
}

#[async_trait]
impl ProviderReceiptReader for HttpProviderReceiptReader {
    async fn observe(
        &self,
        _intent: &EffectIntent,
        _receipt_id: Option<&str>,
    ) -> Result<ObservedOutcome, CommitmentError> {
        Err(CommitmentError::Unavailable {
            detail: "ISSUE-EFFECT-COMMITMENT-12: HttpProviderReceiptReader::observe is not \
                     implemented"
                .to_string(),
        })
    }
}
