//! EffectCommitmentFactory — the adapter construction contract.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#defaulteffectcommitmentfactory
//! Implements: ISSUE-CONTRACT-FREEZE — backend selection + factory interface
//! Issue: #1113
//!
//! The application layer declares **which** adapters may be built without
//! importing the infrastructure crate. Concrete construction is owned by
//! `infrastructure::DefaultEffectCommitmentFactory`.
//!
//! # Contract (Frozen)
//! - Returns the `EffectCommitmentLedger` port, never a concrete adapter type
//! - Credentials in [`EffectCommitmentBackendConfig`] are never rendered by
//!   `Debug`
//! - A backend selection that cannot be constructed fails closed at composition
//!   time

use std::fmt;

use crate::effect_commitment::interfaces::EffectCommitmentLedger;

/// Backend selection for the commitment ledger.
#[derive(Clone, PartialEq, Eq)]
pub enum EffectCommitmentBackendConfig {
    /// The single-process adapter (L1).
    InMemory,

    /// The opt-in HTTP adapter against a contested store (L2).
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

impl fmt::Debug for EffectCommitmentBackendConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InMemory => f.write_str("EffectCommitmentBackendConfig::InMemory"),
            Self::Http {
                base_url,
                timeout_ms,
                max_response_body_bytes,
                auth_token,
            } => f
                .debug_struct("EffectCommitmentBackendConfig::Http")
                .field("base_url", base_url)
                .field("timeout_ms", timeout_ms)
                .field("max_response_body_bytes", max_response_body_bytes)
                .field("auth_token", &auth_token.as_ref().map(|_| "<redacted>"))
                .finish(),
        }
    }
}

/// Builds an [`EffectCommitmentLedger`] adapter from a backend selection.
pub trait EffectCommitmentFactory: Send + Sync {
    /// Construct the adapter described by `config`.
    fn create(&self, config: EffectCommitmentBackendConfig) -> Box<dyn EffectCommitmentLedger>;
}
