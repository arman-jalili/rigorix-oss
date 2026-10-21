//! Operator declaration surface for Effect Commitment.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#effectcommitmentconfig
//! Implements: ISSUE-CONTRACT-FREEZE — `.rigorix/effect-commitment.toml` shape
//! Issue: #1113
//!
//! The context is **opt-in**, exactly like capacity: absent declaration means
//! no adapter, no network, no shared state, and the previous code path is
//! unchanged. A malformed declaration is a hard error at composition time.
//!
//! # Declaration shape (Frozen)
//!
//! ```toml
//! [backend]
//! kind = "in_memory"            # or "http"
//! # base_url = "https://…"     # required for kind = "http"
//! # timeout_ms = 5000
//! # max_response_body_bytes = 65536
//! # auth_token_env = "EFFECT_LEDGER_TOKEN"
//!
//! [receipt_reader]              # optional; without it the ceiling is L2
//! base_url = "https://payments.example"
//! provider_id = "stripe"
//! # timeout_ms = 5000
//! # max_response_body_bytes = 65536
//! # auth_token_env = "PAYMENTS_TOKEN"
//! ```
//!
//! # Contract (Frozen)
//! - `backend` selects the ledger adapter; default when absent is `in_memory`
//! - `receipt_reader` is optional: `None` means the context can prove
//!   **not-settled** only (L3 is unavailable)
//! - Credentials are named by environment variable and never stored in the file
//! - Parsing is fail-closed: an ambiguous declaration is refused, never guessed

use std::fmt;

use crate::effect_commitment::application::factory::EffectCommitmentBackendConfig;

use super::setup::EffectCommitmentSetupError;

/// Default per-request deadline for HTTP adapters.
pub const DEFAULT_TIMEOUT_MS: u64 = 5_000;

/// Default response body cap for HTTP adapters (64 KiB).
pub const DEFAULT_MAX_RESPONSE_BODY_BYTES: usize = 64 * 1024;

/// Configuration for the optional provider receipt reader (L3).
#[derive(Clone, PartialEq, Eq)]
pub struct ProviderReceiptConfig {
    /// Base URL of the provider read-back surface.
    pub base_url: String,
    /// Provider identity recorded on produced evidence.
    pub provider_id: String,
    /// Per-request deadline in milliseconds.
    pub timeout_ms: u64,
    /// Maximum accepted response body size in bytes.
    pub max_response_body_bytes: usize,
    /// Optional bearer token; never logged.
    auth_token: Option<String>,
}

impl ProviderReceiptConfig {
    /// Construct a receipt-reader config.
    pub fn new(
        base_url: impl Into<String>,
        provider_id: impl Into<String>,
        timeout_ms: u64,
        max_response_body_bytes: usize,
        auth_token: Option<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            provider_id: provider_id.into(),
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

impl fmt::Debug for ProviderReceiptConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderReceiptConfig")
            .field("base_url", &self.base_url)
            .field("provider_id", &self.provider_id)
            .field("timeout_ms", &self.timeout_ms)
            .field("max_response_body_bytes", &self.max_response_body_bytes)
            .field(
                "auth_token",
                &self.auth_token.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

/// Parsed and validated operator configuration.
///
/// # Fields (Frozen)
/// - `backend: EffectCommitmentBackendConfig` — ledger adapter selection
/// - `receipt_reader: Option<ProviderReceiptConfig>` — optional L3 read-back
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectCommitmentConfig {
    /// Ledger adapter selection.
    pub backend: EffectCommitmentBackendConfig,

    /// Optional provider receipt reader. `None` caps the claim at not-settled.
    pub receipt_reader: Option<ProviderReceiptConfig>,
}

impl EffectCommitmentConfig {
    /// Construct a configuration.
    pub fn new(
        backend: EffectCommitmentBackendConfig,
        receipt_reader: Option<ProviderReceiptConfig>,
    ) -> Self {
        Self {
            backend,
            receipt_reader,
        }
    }

    /// Whether provider evidence is available (L3).
    pub fn can_prove_commitment(&self) -> bool {
        self.receipt_reader.is_some()
    }

    /// Parse and validate `.rigorix/effect-commitment.toml`.
    ///
    /// The signature is frozen; ISSUE-EFFECT-COMMITMENT-13 supplies the strict
    /// parser. Until then parsing fails closed rather than silently ignoring a
    /// declaration.
    pub fn parse(_text: &str) -> Result<Self, EffectCommitmentSetupError> {
        Err(EffectCommitmentSetupError::Unavailable {
            detail:
                "ISSUE-EFFECT-COMMITMENT-13: effect-commitment config parsing is not implemented"
                    .to_string(),
        })
    }
}
