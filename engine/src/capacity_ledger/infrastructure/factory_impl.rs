//! Concrete `CapacityLedgerFactory` — builds an adapter from a backend choice.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#enforcementseam
//! ADR-018: distributed capacity ledger
//! Implements: ISSUE-CAPACITY-LEDGER-SEAM — the missing concrete factory
//!
//! The contract freeze declared `CapacityLedgerFactory` as a trait with **no
//! implementation**, so nothing could construct an adapter and the whole context
//! was unreachable from the composition root. This is the concrete construction
//! path that closes that gap.
//!
//! Construction is the only place that knows about both adapters; the
//! application layer keeps depending on the port alone.

use crate::capacity_ledger::application::factory::{CapacityLedgerFactory, LedgerBackendConfig};
use crate::capacity_ledger::interfaces::CapacityLedger;

use super::http::{HttpLedger, HttpLedgerConfig};
use super::in_process::InProcessLedger;

/// The default factory: maps a [`LedgerBackendConfig`] to a concrete adapter.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultCapacityLedgerFactory;

impl DefaultCapacityLedgerFactory {
    /// Construct the factory.
    pub fn new() -> Self {
        Self
    }
}

impl CapacityLedgerFactory for DefaultCapacityLedgerFactory {
    fn create(&self, config: LedgerBackendConfig) -> Box<dyn CapacityLedger> {
        match config {
            LedgerBackendConfig::InProcess => Box::new(InProcessLedger::new()),
            LedgerBackendConfig::Http {
                base_url,
                timeout_ms,
                max_response_body_bytes,
                auth_token,
            } => Box::new(HttpLedger::new(HttpLedgerConfig::new(
                base_url,
                timeout_ms,
                max_response_body_bytes,
                auth_token,
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_an_in_process_adapter_for_the_default_backend() {
        let factory = DefaultCapacityLedgerFactory::new();
        // Constructing must not panic and must yield a usable port object.
        let _ledger = factory.create(LedgerBackendConfig::InProcess);
    }

    #[test]
    fn builds_an_http_adapter_without_leaking_the_token() {
        let factory = DefaultCapacityLedgerFactory::new();
        let config = LedgerBackendConfig::Http {
            base_url: "https://ledger.example.invalid".to_string(),
            timeout_ms: 1_000,
            max_response_body_bytes: 4096,
            auth_token: Some("super-secret-token".to_string()),
        };
        // Debug must never render the secret.
        let rendered = format!("{config:?}");
        assert!(!rendered.contains("super-secret-token"));
        assert!(rendered.contains("<redacted>"));
        let _ledger = factory.create(config);
    }
}
