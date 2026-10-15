//! HttpLedger adapter stub.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#httpledger
//! Implements: ISSUE-CONTRACT-FREEZE — HttpLedger adapter config + contract
//! Issue: #1021
//!
//! Opt-in distributed mode. Reaches the enterprise ledger service over HTTP with
//! the same transport discipline as the precondition provider client: bounded
//! body, no redirect amplification, explicit deadline, credentials never logged.
//! Maps every ambiguity to `Unavailable` so the caller fails closed (I4).
//!
//! # Status
//! The port methods are TODO stubs that fail closed with
//! [`LedgerError::Unavailable`]; ISSUE-CAPACITY-LEDGER-10 supplies the real
//! implementation. The config type and constructor are frozen here.

use std::fmt;

use async_trait::async_trait;

use crate::capacity_ledger::domain::{
    Exposure, Grant, IdempotencyKey, LedgerError, Reservation, ResourceId, ResourceSpec,
};
use crate::capacity_ledger::interfaces::CapacityLedger;

/// Construction parameters for [`HttpLedger`].
///
/// `Debug` redacts the bearer token so configuration can be logged safely.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpLedgerConfig {
    /// Base URL of the ledger service.
    pub base_url: String,

    /// Per-request deadline in milliseconds.
    pub timeout_ms: u64,

    /// Maximum accepted response body size in bytes.
    pub max_response_body_bytes: usize,

    /// Optional bearer token; never logged.
    auth_token: Option<String>,
}

impl HttpLedgerConfig {
    /// Construct a config.
    pub fn new(
        base_url: impl Into<String>,
        timeout_ms: u64,
        max_response_body_bytes: usize,
        auth_token: Option<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            timeout_ms,
            max_response_body_bytes,
            auth_token,
        }
    }

    /// The bearer token, if configured.
    pub fn auth_token(&self) -> Option<&str> {
        self.auth_token.as_deref()
    }
}

impl fmt::Debug for HttpLedgerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpLedgerConfig")
            .field("base_url", &self.base_url)
            .field("timeout_ms", &self.timeout_ms)
            .field("max_response_body_bytes", &self.max_response_body_bytes)
            .field(
                "auth_token",
                &self.auth_token.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

/// The HTTP capacity-ledger adapter.
pub struct HttpLedger {
    config: HttpLedgerConfig,
}

impl HttpLedger {
    /// Create an adapter from its frozen config.
    pub fn new(config: HttpLedgerConfig) -> Self {
        Self { config }
    }

    /// The active configuration.
    pub fn config(&self) -> &HttpLedgerConfig {
        &self.config
    }
}

/// Fail-closed placeholder body. TODO(ISSUE-CAPACITY-LEDGER-10): replace each
/// call with the real HTTP operation.
fn not_implemented(op: &str) -> LedgerError {
    LedgerError::Unavailable {
        detail: format!("HttpLedger::{op} not implemented yet (ISSUE-CAPACITY-LEDGER-10)"),
    }
}

#[async_trait]
impl CapacityLedger for HttpLedger {
    async fn declare_resource(&self, _spec: ResourceSpec) -> Result<(), LedgerError> {
        Err(not_implemented("declare_resource"))
    }

    async fn reserve(
        &self,
        _resource: &ResourceId,
        _amount: u64,
        _key: IdempotencyKey,
    ) -> Result<Grant, LedgerError> {
        Err(not_implemented("reserve"))
    }

    async fn commit(&self, _grant: Grant) -> Result<Reservation, LedgerError> {
        Err(not_implemented("commit"))
    }

    async fn release(&self, _grant: Grant) -> Result<Reservation, LedgerError> {
        Err(not_implemented("release"))
    }

    async fn exposure(&self, _resource: &ResourceId) -> Result<Exposure, LedgerError> {
        Err(not_implemented("exposure"))
    }
}
