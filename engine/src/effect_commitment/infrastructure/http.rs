//! HttpEffectLedger — the L2 adapter stub.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#httpeffectledger
//! Implements: ISSUE-CONTRACT-FREEZE — HTTP adapter config + contract
//! Issue: #1113
//!
//! Opt-in distributed mode. Reaches a contested store that owns atomicity, with
//! the same transport discipline as the capacity ledger's HTTP adapter: bounded
//! body, no redirect amplification, explicit deadline, credentials never logged.
//! Maps every ambiguity to `Unavailable` so the caller fails closed.
//!
//! # Status
//! ISSUE-EFFECT-COMMITMENT-11 supplies the implementation. The config type and
//! constructor are frozen here.

use std::fmt;

use async_trait::async_trait;

use crate::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectQuery, EffectRecord, EffectRecordId, ObservedOutcome,
    ResolverIdentity,
};
use crate::effect_commitment::interfaces::EffectCommitmentLedger;

/// Construction parameters for [`HttpEffectLedger`].
///
/// `Debug` redacts the bearer token so configuration can be logged safely.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpEffectLedgerConfig {
    /// Base URL of the commitment service.
    pub base_url: String,

    /// Per-request deadline in milliseconds.
    pub timeout_ms: u64,

    /// Maximum accepted response body size in bytes.
    pub max_response_body_bytes: usize,

    /// Optional bearer token; never logged.
    auth_token: Option<String>,
}

impl HttpEffectLedgerConfig {
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

impl fmt::Debug for HttpEffectLedgerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpEffectLedgerConfig")
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

/// The HTTP commitment-ledger adapter.
pub struct HttpEffectLedger {
    config: HttpEffectLedgerConfig,
}

impl HttpEffectLedger {
    /// Create an adapter from its frozen config.
    pub fn new(config: HttpEffectLedgerConfig) -> Self {
        Self { config }
    }

    /// The active configuration.
    pub fn config(&self) -> &HttpEffectLedgerConfig {
        &self.config
    }

    fn not_implemented(operation: &str) -> CommitmentError {
        CommitmentError::Unavailable {
            detail: format!(
                "ISSUE-EFFECT-COMMITMENT-11: HttpEffectLedger::{operation} is not implemented"
            ),
        }
    }
}

#[async_trait]
impl EffectCommitmentLedger for HttpEffectLedger {
    async fn begin(&self, _intent: EffectIntent) -> Result<EffectRecord, CommitmentError> {
        Err(Self::not_implemented("begin"))
    }

    async fn settle_definite(
        &self,
        _record_id: EffectRecordId,
        _outcome: ObservedOutcome,
    ) -> Result<EffectRecord, CommitmentError> {
        Err(Self::not_implemented("settle_definite"))
    }

    async fn mark_ambiguous(
        &self,
        _record_id: EffectRecordId,
        _reason: String,
    ) -> Result<EffectRecord, CommitmentError> {
        Err(Self::not_implemented("mark_ambiguous"))
    }

    async fn resolve(
        &self,
        _record_id: EffectRecordId,
        _outcome: ObservedOutcome,
        _resolver: ResolverIdentity,
    ) -> Result<EffectRecord, CommitmentError> {
        Err(Self::not_implemented("resolve"))
    }

    async fn query(&self, _query: EffectQuery) -> Result<Vec<EffectRecord>, CommitmentError> {
        Err(Self::not_implemented("query"))
    }
}
