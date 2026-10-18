//! CapacityLedgerFactory contract.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#ddd-layers
//! Implements: ISSUE-CONTRACT-FREEZE — adapter factory interface
//! Issue: #1021
//!
//! The application layer declares *which* adapters may be built without
//! importing the infrastructure crate. Concrete construction (and the mapping
//! from [`LedgerBackendConfig`] to an adapter-internal config) is owned by
//! `infrastructure`.
//!
//! # Contract (Frozen)
//! - Returns the `CapacityLedger` port, never a concrete adapter type
//! - Credentials in [`LedgerBackendConfig`] are never rendered by `Debug`

use std::fmt;

use crate::capacity_ledger::interfaces::CapacityLedger;

/// Backend selection passed to a [`CapacityLedgerFactory`].
///
/// `Debug` redacts the bearer token so config can be logged safely.
#[derive(Clone, PartialEq, Eq)]
pub enum LedgerBackendConfig {
    /// The standalone in-process adapter (single-process atomicity).
    InProcess,

    /// The opt-in HTTP adapter against an enterprise ledger service.
    Http {
        /// Base URL of the ledger service (no trailing slash required).
        base_url: String,
        /// Per-request deadline in milliseconds.
        timeout_ms: u64,
        /// Maximum accepted response body size in bytes.
        max_response_body_bytes: usize,
        /// Optional bearer token; never logged.
        auth_token: Option<String>,
    },
}

impl fmt::Debug for LedgerBackendConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InProcess => f.write_str("LedgerBackendConfig::InProcess"),
            Self::Http {
                base_url,
                timeout_ms,
                max_response_body_bytes,
                auth_token,
            } => f
                .debug_struct("LedgerBackendConfig::Http")
                .field("base_url", base_url)
                .field("timeout_ms", timeout_ms)
                .field("max_response_body_bytes", max_response_body_bytes)
                .field("auth_token", &auth_token.as_ref().map(|_| "<redacted>"))
                .finish(),
        }
    }
}

/// Builds a [`CapacityLedger`] adapter from a backend selection.
pub trait CapacityLedgerFactory: Send + Sync {
    /// Construct the adapter described by `config`.
    fn create(&self, config: LedgerBackendConfig) -> Box<dyn CapacityLedger>;
}
