//! IdpConfig — OIDC provider configuration value object.
//!
//! @canonical .pi/architecture/modules/auth.md#config
//! Implements: Contract Freeze — IdpConfig value object
//! ADR-008: OIDC device flow — dev or org supplies IdP credentials
//!
//! Static configuration for the OIDC identity provider the auth module talks
//! to. Loaded from `.rigorix/auth.toml` `[auth]` section or environment
//! (`RIGORIX_IDP_ISSUER`, `RIGORIX_IDP_CLIENT_ID`, optional
//! `RIGORIX_IDP_CLIENT_SECRET`).
//!
//! # Contract (Frozen)
//!
//! - `issuer` must be a valid HTTPS URL (validated on construction) — the
//!   IdP endpoint is always TLS (matches enterprise proxy policy)
//! - `client_id` is required and non-empty
//! - `client_secret` is stored as `Secret<String>` — never logged, always
//!   redacted; optional (public clients per RFC 6749 §2.1)
//! - `access_token_ttl_secs` defaults to 900 (5–15 min window per ADR-008)

use serde::{Deserialize, Serialize};

use super::error::AuthError;
use super::value::Secret;

/// Default access-token TTL in seconds (900 = 15 minutes, ADR-008).
pub const DEFAULT_ACCESS_TOKEN_TTL_SECS: u64 = 900;

/// OIDC provider configuration for the device flow (RFC 8628).
///
/// # Contract (Frozen)
///
/// - Immutable after construction
/// - Validated on construction — invalid values return `AuthError::Configuration`
/// - Serializes for config round-tripping; `client_secret` never leaks in
///   Debug/Display/Serialize output
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdpConfig {
    /// OIDC issuer URL (must be HTTPS).
    issuer: String,

    /// OAuth2 client id registered at the IdP.
    client_id: String,

    /// Optional client secret for confidential clients (redacted).
    client_secret: Option<Secret<String>>,

    /// Access-token TTL in seconds applied to TokenProvider custody.
    access_token_ttl_secs: u64,
}

impl IdpConfig {
    /// Create a new IdpConfig with validation.
    ///
    /// # Errors
    /// - `AuthError::Configuration` if `issuer` is not an HTTPS URL
    /// - `AuthError::Configuration` if `client_id` is empty
    pub fn new(
        issuer: String,
        client_id: String,
        client_secret: Option<String>,
        access_token_ttl_secs: Option<u64>,
    ) -> Result<Self, AuthError> {
        if client_id.trim().is_empty() {
            return Err(AuthError::Configuration(
                "IdP client_id cannot be empty".into(),
            ));
        }
        if !issuer.starts_with("https://") {
            return Err(AuthError::Configuration(format!(
                "IdP issuer must be an HTTPS URL, got: {issuer}"
            )));
        }
        Ok(Self {
            issuer,
            client_id,
            client_secret: client_secret.map(Secret::new),
            access_token_ttl_secs: access_token_ttl_secs.unwrap_or(DEFAULT_ACCESS_TOKEN_TTL_SECS),
        })
    }

    /// OIDC issuer URL.
    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    /// OAuth2 client id.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Optional client secret (redacted wrapper).
    pub fn client_secret(&self) -> Option<&Secret<String>> {
        self.client_secret.as_ref()
    }

    /// Access-token TTL in seconds for TokenProvider custody.
    pub fn access_token_ttl_secs(&self) -> u64 {
        self.access_token_ttl_secs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_https_issuer() {
        let err = IdpConfig::new(
            "http://idp.example.com".into(),
            "rigorix-cli".into(),
            None,
            None,
        )
        .unwrap_err();
        assert!(matches!(err, AuthError::Configuration(_)));
    }

    #[test]
    fn rejects_empty_client_id() {
        let err =
            IdpConfig::new("https://idp.example.com".into(), "".into(), None, None).unwrap_err();
        assert!(matches!(err, AuthError::Configuration(_)));
    }

    #[test]
    fn applies_default_ttl() {
        let cfg = IdpConfig::new(
            "https://idp.example.com".into(),
            "rigorix-cli".into(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(cfg.access_token_ttl_secs(), DEFAULT_ACCESS_TOKEN_TTL_SECS);
        assert!(cfg.client_secret().is_none());
    }

    #[test]
    fn client_secret_is_redacted_in_debug() {
        let cfg = IdpConfig::new(
            "https://idp.example.com".into(),
            "rigorix-cli".into(),
            Some("s3cret".into()),
            None,
        )
        .unwrap();
        let rendered = format!("{cfg:?}");
        assert!(!rendered.contains("s3cret"));
        assert!(rendered.contains("REDACTED"));
    }
}
