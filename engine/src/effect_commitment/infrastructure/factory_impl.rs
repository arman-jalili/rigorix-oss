//! DefaultEffectCommitmentFactory — concrete construction from a backend choice.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#defaulteffectcommitmentfactory
//! Implements: ISSUE-CONTRACT-FREEZE — the concrete factory
//! Issue: #1113
//!
//! The capacity ledger's contract freeze declared a factory with **no
//! implementation** and the whole context was unreachable; this exists so that
//! failure cannot repeat here. Construction is the only place that knows about
//! both adapters; the application layer keeps depending on the port alone.

use crate::effect_commitment::application::{
    EffectCommitmentBackendConfig, EffectCommitmentFactory, ProviderReceiptConfig,
    ProviderReceiptReader,
};
use crate::effect_commitment::interfaces::EffectCommitmentLedger;

use super::http::{HttpEffectLedger, HttpEffectLedgerConfig};
use super::in_memory::InMemoryEffectLedger;
use super::provider_receipt::HttpProviderReceiptReader;

/// The default factory: maps a backend selection to a concrete adapter.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultEffectCommitmentFactory;

impl DefaultEffectCommitmentFactory {
    /// Construct the factory.
    pub fn new() -> Self {
        Self
    }
}

impl EffectCommitmentFactory for DefaultEffectCommitmentFactory {
    fn create(&self, config: EffectCommitmentBackendConfig) -> Box<dyn EffectCommitmentLedger> {
        match config {
            EffectCommitmentBackendConfig::InMemory => Box::new(InMemoryEffectLedger::new()),
            EffectCommitmentBackendConfig::Http {
                base_url,
                timeout_ms,
                max_response_body_bytes,
                auth_token,
            } => Box::new(HttpEffectLedger::new(HttpEffectLedgerConfig::new(
                base_url,
                timeout_ms,
                max_response_body_bytes,
                auth_token,
            ))),
        }
    }

    fn create_receipt_reader(
        &self,
        config: ProviderReceiptConfig,
    ) -> Box<dyn ProviderReceiptReader> {
        Box::new(HttpProviderReceiptReader::new(config))
    }
}
