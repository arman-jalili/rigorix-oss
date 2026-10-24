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
//! [receipt_reader]              # optional; without it settlement stays `none`
//!                               # (durability comes from `backend`: L2 only for `http`)
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

use serde::Deserialize;

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
    /// Validation is strict and fail-closed: an unknown field, an ambiguous
    /// backend selection, or an unresolvable credential name is refused rather
    /// than guessed.
    pub fn parse(text: &str) -> Result<Self, EffectCommitmentSetupError> {
        let raw: RawConfig = toml::from_str(text).map_err(|e| {
            invalid(format!(
                ".rigorix/effect-commitment.toml could not be parsed: {e}"
            ))
        })?;

        let backend = parse_backend(raw.backend)?;
        let receipt_reader = raw.receipt_reader.map(parse_receipt_reader).transpose()?;

        Ok(Self {
            backend,
            receipt_reader,
        })
    }
}

// ---------------------------------------------------------------------------
// Raw TOML shape
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    backend: Option<RawBackend>,
    #[serde(default)]
    receipt_reader: Option<RawReceiptReader>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBackend {
    kind: String,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    max_response_body_bytes: Option<usize>,
    /// Environment variable holding the bearer token, never the token itself.
    #[serde(default)]
    auth_token_env: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawReceiptReader {
    base_url: String,
    provider_id: String,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    max_response_body_bytes: Option<usize>,
    /// Environment variable holding the bearer token, never the token itself.
    #[serde(default)]
    auth_token_env: Option<String>,
}

fn invalid(detail: impl Into<String>) -> EffectCommitmentSetupError {
    EffectCommitmentSetupError::ConfigInvalid {
        detail: detail.into(),
    }
}

fn validate_url(what: &str, url: &str) -> Result<(), EffectCommitmentSetupError> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(invalid(format!("{what} must be an absolute http(s) URL")));
    }
    Ok(())
}

fn read_token(
    env_var: Option<String>,
    what: &str,
) -> Result<Option<String>, EffectCommitmentSetupError> {
    match env_var {
        Some(var) => match std::env::var(&var) {
            Ok(value) if !value.is_empty() => Ok(Some(value)),
            _ => Err(invalid(format!(
                "{what} auth_token_env names '{var}' but the variable is missing or empty"
            ))),
        },
        None => Ok(None),
    }
}

fn parse_backend(
    raw: Option<RawBackend>,
) -> Result<EffectCommitmentBackendConfig, EffectCommitmentSetupError> {
    let Some(raw) = raw else {
        // Absent backend means the inert in-process default; the declaration
        // itself is what arms the context.
        return Ok(EffectCommitmentBackendConfig::InMemory);
    };

    match raw.kind.as_str() {
        "in_memory" => {
            if raw.base_url.is_some()
                || raw.timeout_ms.is_some()
                || raw.max_response_body_bytes.is_some()
                || raw.auth_token_env.is_some()
            {
                return Err(invalid(
                    "backend kind 'in_memory' must not declare HTTP fields (base_url, timeout_ms, max_response_body_bytes, auth_token_env)",
                ));
            }
            Ok(EffectCommitmentBackendConfig::InMemory)
        }
        "http" => {
            let base_url = raw
                .base_url
                .ok_or_else(|| invalid("backend kind 'http' requires base_url"))?;
            validate_url("backend base_url", &base_url)?;
            let timeout_ms = raw.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
            if timeout_ms == 0 {
                return Err(invalid("backend timeout_ms must be greater than zero"));
            }
            let max_response_body_bytes = raw
                .max_response_body_bytes
                .unwrap_or(DEFAULT_MAX_RESPONSE_BODY_BYTES);
            if max_response_body_bytes == 0 {
                return Err(invalid(
                    "backend max_response_body_bytes must be greater than zero",
                ));
            }
            let auth_token = read_token(raw.auth_token_env, "backend")?;
            Ok(EffectCommitmentBackendConfig::Http {
                base_url,
                timeout_ms,
                max_response_body_bytes,
                auth_token,
            })
        }
        other => Err(invalid(format!(
            "unknown backend kind '{other}' (expected 'in_memory' or 'http')"
        ))),
    }
}

fn parse_receipt_reader(
    raw: RawReceiptReader,
) -> Result<ProviderReceiptConfig, EffectCommitmentSetupError> {
    validate_url("receipt_reader base_url", &raw.base_url)?;
    if raw.provider_id.trim().is_empty() {
        return Err(invalid("receipt_reader provider_id must not be empty"));
    }

    let timeout_ms = raw.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    if timeout_ms == 0 {
        return Err(invalid(
            "receipt_reader timeout_ms must be greater than zero",
        ));
    }
    let max_response_body_bytes = raw
        .max_response_body_bytes
        .unwrap_or(DEFAULT_MAX_RESPONSE_BODY_BYTES);
    if max_response_body_bytes == 0 {
        return Err(invalid(
            "receipt_reader max_response_body_bytes must be greater than zero",
        ));
    }

    let auth_token = read_token(raw.auth_token_env, "receipt_reader")?;
    Ok(ProviderReceiptConfig::new(
        raw.base_url,
        raw.provider_id,
        timeout_ms,
        max_response_body_bytes,
        auth_token,
    ))
}
