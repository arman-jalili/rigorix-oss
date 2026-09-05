//! HttpIdpClient — OIDC device-flow client over HTTPS (RFC 8628).
//!
//! @canonical .pi/architecture/modules/auth.md#idpclient-infrastructure
//! Implements: ISSUE-AUTH-2 — IdpClient (Infrastructure)
//! Issue: #822
//! ADR-008: OIDC device flow (RFC 8628) over HTTPS
//!
//! Concrete implementation of the frozen [`IdpClient`] port. Talks to any
//! OIDC provider the dev or org configures (Keycloak, Entra ID, Okta, …)
//! over reqwest:
//!
//! - `discover` — OIDC discovery (`.well-known/openid-configuration`, RFC 8414)
//! - `device_authorization` — RFC 8628 §3.1
//! - `poll_token` — RFC 8628 §3.5 (RFC in-progress codes → [`TokenPoll`])
//! - `refresh_token` — RFC 6749 §6
//! - `revoke_token` — RFC 7009
//!
//! # Security Contract
//!
//! - Production issuers MUST be HTTPS; TLS verification is always on
//! - Plain-HTTP is accepted **only** for loopback addresses
//!   (`http://127.0.0.1`, `http://localhost`, `http://[::1]`) — local mock
//!   IdPs for development and tests. Any other plain-HTTP issuer/endpoint is
//!   rejected at construction/discovery (matches enterprise proxy policy)
//! - Discovered `issuer` must match the configured issuer (confused-deputy
//!   guard)
//! - Secrets (device codes, tokens) stay wrapped in `Secret<T>`; never logged

use chrono::Utc;
use serde_json::Value;
use std::sync::Mutex;

use crate::auth::domain::error::AuthError;
use crate::auth::domain::value::Secret;
use crate::auth::infrastructure::idp_client::{
    DEFAULT_POLL_INTERVAL_SECS, DeviceAuthorization, IdpClient, IdpMetadata, TokenPoll,
    TokenResponse,
};

/// Default request/response timeout for IdP calls (seconds).
const DEFAULT_TIMEOUT_SECS: u64 = 15;

/// Scope requested on device authorization — `openid` plus `offline_access`
/// so the IdP returns a refresh token for keychain custody (ADR-008).
const DEVICE_AUTHORIZATION_SCOPE: &str = "openid offline_access";

/// OAuth2 grant type for the device flow token exchange (RFC 8628 §3.4).
const DEVICE_CODE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

/// OAuth2 grant type for refresh (RFC 6749 §6).
const REFRESH_TOKEN_GRANT_TYPE: &str = "refresh_token";

// ---------------------------------------------------------------------------
// URL policy helpers
// ---------------------------------------------------------------------------

/// True when `url` is HTTPS (production policy).
fn is_https(url: &str) -> bool {
    url.starts_with("https://")
}

/// True when `url` is plain HTTP bound to a loopback address (local mocks).
fn is_loopback_http(url: &str) -> bool {
    url.starts_with("http://127.0.0.1")
        || url.starts_with("http://localhost")
        || url.starts_with("http://[::1]")
}

/// Validate an issuer/endpoint URL against the transport security policy.
///
/// # Errors
/// - `AuthError::Configuration` — plain HTTP to a non-loopback address
fn validate_endpoint_url(kind: &str, url: &str) -> Result<(), AuthError> {
    if is_https(url) || is_loopback_http(url) {
        Ok(())
    } else {
        Err(AuthError::Configuration(format!(
            "{kind} {url} must be HTTPS (plain HTTP is only accepted for \
             loopback addresses — local mock IdPs)"
        )))
    }
}

// ---------------------------------------------------------------------------
// Pure response parsers (unit-testable without a server)
// ---------------------------------------------------------------------------

/// Parse an OIDC discovery document (RFC 8414) and validate the issuer.
fn parse_discovery(body: &str, expected_issuer: &str) -> Result<IdpMetadata, AuthError> {
    let value: Value = serde_json::from_str(body).map_err(|e| AuthError::Discovery {
        issuer: expected_issuer.to_string(),
        reason: format!("malformed discovery JSON: {e}"),
    })?;

    let issuer = value
        .get("issuer")
        .and_then(Value::as_str)
        .ok_or_else(|| AuthError::Discovery {
            issuer: expected_issuer.to_string(),
            reason: "discovery document missing 'issuer'".into(),
        })?
        .to_string();

    if issuer != expected_issuer {
        return Err(AuthError::Discovery {
            issuer: expected_issuer.to_string(),
            reason: format!("discovery issuer {issuer} does not match configured issuer"),
        });
    }

    let get_url = |key: &str| -> Result<String, AuthError> {
        let raw = value
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| AuthError::Discovery {
                issuer: expected_issuer.to_string(),
                reason: format!("discovery document missing '{key}'"),
            })?;
        validate_endpoint_url(key, raw).map_err(|_| AuthError::Discovery {
            issuer: expected_issuer.to_string(),
            reason: format!(
                "discovery '{key}' {raw} violates transport security \
                             policy (HTTPS or loopback only)"
            ),
        })?;
        Ok(raw.to_string())
    };

    Ok(IdpMetadata {
        issuer: issuer.clone(),
        device_authorization_endpoint: get_url("device_authorization_endpoint")?,
        token_endpoint: get_url("token_endpoint")?,
        revocation_endpoint: value
            .get("revocation_endpoint")
            .and_then(Value::as_str)
            .map(str::to_string),
        jwks_uri: value
            .get("jwks_uri")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

/// Parse a device-authorization response (RFC 8628 §3.2).
fn parse_device_authorization(body: &str) -> Result<DeviceAuthorization, AuthError> {
    let value: Value = serde_json::from_str(body).map_err(|e| {
        AuthError::InvalidTokenResponse(format!("malformed device authorization JSON: {e}"))
    })?;

    let device_code = value
        .get("device_code")
        .and_then(Value::as_str)
        .ok_or_else(|| AuthError::DeviceAuthorizationRejected("missing 'device_code'".into()))?;
    let user_code = value
        .get("user_code")
        .and_then(Value::as_str)
        .ok_or_else(|| AuthError::DeviceAuthorizationRejected("missing 'user_code'".into()))?;
    let verification_uri = value
        .get("verification_uri")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            AuthError::DeviceAuthorizationRejected("missing 'verification_uri'".into())
        })?;
    let expires_in = value
        .get("expires_in")
        .and_then(Value::as_u64)
        .ok_or_else(|| AuthError::DeviceAuthorizationRejected("missing 'expires_in'".into()))?;
    let interval_secs = value
        .get("interval")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_POLL_INTERVAL_SECS);

    Ok(DeviceAuthorization {
        device_code: Secret::new(device_code.to_string()),
        user_code: user_code.to_string(),
        verification_uri: verification_uri.to_string(),
        expires_in,
        interval_secs,
        issued_at: Utc::now(),
    })
}

/// Parse a successful token response (RFC 6749 §5.1).
fn parse_token_response(body: &str) -> Result<TokenResponse, AuthError> {
    let value: Value = serde_json::from_str(body).map_err(|e| {
        AuthError::InvalidTokenResponse(format!("malformed token response JSON: {e}"))
    })?;

    let access_token = value
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| AuthError::InvalidTokenResponse("missing 'access_token'".into()))?;
    let expires_in = value
        .get("expires_in")
        .and_then(Value::as_u64)
        .ok_or_else(|| AuthError::InvalidTokenResponse("missing 'expires_in'".into()))?;

    Ok(TokenResponse {
        access_token: Secret::new(access_token.to_string()),
        refresh_token: value
            .get("refresh_token")
            .and_then(Value::as_str)
            .map(|s| Secret::new(s.to_string())),
        expires_in,
        token_type: value
            .get("token_type")
            .and_then(Value::as_str)
            .unwrap_or("Bearer")
            .to_string(),
        scope: value
            .get("scope")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

/// Parse an RFC 8628 token-endpoint error body into a typed outcome.
fn parse_token_error(body: &str) -> TokenPoll {
    let value: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let error = value.get("error").and_then(Value::as_str).unwrap_or("");
    let description = value
        .get("error_description")
        .and_then(Value::as_str)
        .unwrap_or("");
    match error {
        "authorization_pending" => TokenPoll::Pending {
            retry_after_secs: None,
        },
        "slow_down" => TokenPoll::Pending {
            retry_after_secs: Some(DEFAULT_POLL_INTERVAL_SECS),
        },
        "access_denied" => TokenPoll::AccessDenied {
            reason: if description.is_empty() {
                "user denied the device authorization request".into()
            } else {
                description.to_string()
            },
        },
        "expired_token" => TokenPoll::Expired,
        other => TokenPoll::AccessDenied {
            reason: format!("IdP token error '{other}': {description}")
                .trim()
                .to_string(),
        },
    }
}

// ---------------------------------------------------------------------------
// HttpIdpClient
// ---------------------------------------------------------------------------

/// Default [`IdpClient`] implementation backed by reqwest.
///
/// Discovery results are cached for the client lifetime; the token and
/// revocation endpoints come from the discovery document (never from
/// configuration, preventing endpoint smuggling).
pub struct HttpIdpClient {
    /// Configured OIDC issuer (base URL for discovery).
    issuer: String,

    /// Reusable HTTPS-capable HTTP client.
    http: reqwest::Client,

    /// Cached discovery metadata.
    metadata: Mutex<Option<IdpMetadata>>,
}

impl HttpIdpClient {
    /// Create a client for the given OIDC issuer.
    ///
    /// # Errors
    /// - `AuthError::Configuration` — issuer is not HTTPS (and not a
    ///   loopback address)
    /// - `AuthError::Configuration` — reqwest client construction failed
    pub fn new(issuer: impl Into<String>) -> Result<Self, AuthError> {
        let issuer = issuer.into();
        validate_endpoint_url("issuer", &issuer)?;
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .map_err(|e| AuthError::Configuration(format!("HTTP client: {e}")))?;
        Ok(Self {
            issuer,
            http,
            metadata: Mutex::new(None),
        })
    }

    /// Build a client over an existing reqwest client (composition/testing).
    ///
    /// Callers own the transport configuration; issuer/endpoint policy still
    /// applies (HTTPS or loopback plain HTTP).
    pub fn with_http_client(
        issuer: impl Into<String>,
        http: reqwest::Client,
    ) -> Result<Self, AuthError> {
        let issuer = issuer.into();
        validate_endpoint_url("issuer", &issuer)?;
        Ok(Self {
            issuer,
            http,
            metadata: Mutex::new(None),
        })
    }

    /// Cached discovery metadata (fetches on first use).
    async fn metadata(&self) -> Result<IdpMetadata, AuthError> {
        if let Some(cached) = self.metadata.lock().ok().and_then(|g| g.clone()) {
            return Ok(cached);
        }
        let meta = self.fetch_discovery().await?;
        if let Ok(mut guard) = self.metadata.lock() {
            *guard = Some(meta.clone());
        }
        Ok(meta)
    }

    async fn fetch_discovery(&self) -> Result<IdpMetadata, AuthError> {
        let well_known = format!("{}/.well-known/openid-configuration", self.issuer);
        let response =
            self.http
                .get(&well_known)
                .send()
                .await
                .map_err(|e| AuthError::Discovery {
                    issuer: self.issuer.clone(),
                    reason: format!("request failed: {e}"),
                })?;
        let status = response.status();
        let body = response.text().await.map_err(|e| AuthError::Discovery {
            issuer: self.issuer.clone(),
            reason: format!("read failed: {e}"),
        })?;
        if !status.is_success() {
            return Err(AuthError::Discovery {
                issuer: self.issuer.clone(),
                reason: format!("HTTP {status}: {}", truncate(&body)),
            });
        }
        parse_discovery(&body, &self.issuer)
    }

    /// Shared token/device endpoint POST used by all grants.
    async fn post_form(
        &self,
        endpoint: &str,
        form: &[(&str, String)],
    ) -> Result<(reqwest::StatusCode, String), AuthError> {
        let response = self
            .http
            .post(endpoint)
            .form(form)
            .send()
            .await
            .map_err(|e| AuthError::Transport(format!("{endpoint}: {e}")))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| AuthError::Transport(format!("{endpoint}: {e}")))?;
        Ok((status, body))
    }
}

#[async_trait::async_trait]
impl IdpClient for HttpIdpClient {
    async fn discover(&self) -> Result<IdpMetadata, AuthError> {
        self.metadata().await
    }

    async fn device_authorization(
        &self,
        client_id: &str,
    ) -> Result<DeviceAuthorization, AuthError> {
        let meta = self.metadata().await?;
        let form = [
            ("client_id", client_id.to_string()),
            ("scope", DEVICE_AUTHORIZATION_SCOPE.to_string()),
        ];
        let (status, body) = self
            .post_form(&meta.device_authorization_endpoint, &form)
            .await?;
        if status.is_success() {
            parse_device_authorization(&body)
        } else if let Some(reason) = rfc_error_reason(&body) {
            Err(AuthError::DeviceAuthorizationRejected(reason))
        } else {
            Err(AuthError::Transport(format!(
                "device authorization HTTP {status}: {}",
                truncate(&body)
            )))
        }
    }

    async fn poll_token(
        &self,
        device_code: &Secret<String>,
        client_id: &str,
    ) -> Result<TokenPoll, AuthError> {
        let meta = self.metadata().await?;
        let form = [
            ("grant_type", DEVICE_CODE_GRANT_TYPE.to_string()),
            ("device_code", device_code.expose().clone()),
            ("client_id", client_id.to_string()),
        ];
        let (status, body) = self.post_form(&meta.token_endpoint, &form).await?;
        if status.is_success() {
            Ok(TokenPoll::Succeeded(parse_token_response(&body)?))
        } else {
            // RFC 8628 §3.5 in-progress codes are expected states.
            Ok(parse_token_error(&body))
        }
    }

    async fn refresh_token(
        &self,
        refresh_token: &Secret<String>,
        client_id: &str,
    ) -> Result<TokenResponse, AuthError> {
        let meta = self.metadata().await?;
        let form = [
            ("grant_type", REFRESH_TOKEN_GRANT_TYPE.to_string()),
            ("refresh_token", refresh_token.expose().clone()),
            ("client_id", client_id.to_string()),
        ];
        let (status, body) = self.post_form(&meta.token_endpoint, &form).await?;
        if status.is_success() {
            parse_token_response(&body)
        } else {
            let reason = rfc_error_reason(&body).unwrap_or_else(|| truncate(&body));
            Err(AuthError::RefreshFailed(reason))
        }
    }

    async fn revoke_token(
        &self,
        refresh_token: &Secret<String>,
        _client_id: &str,
    ) -> Result<(), AuthError> {
        let meta = self.metadata().await?;
        // RFC 7009 endpoint is optional — nothing to revoke against.
        let Some(endpoint) = meta.revocation_endpoint.as_deref() else {
            return Ok(());
        };
        let form = [
            ("token", refresh_token.expose().clone()),
            ("token_type_hint", "refresh_token".to_string()),
        ];
        let (status, body) = self.post_form(endpoint, &form).await?;
        if status.is_success() {
            Ok(())
        } else {
            Err(AuthError::Transport(format!(
                "revocation HTTP {status}: {}",
                truncate(&body)
            )))
        }
    }
}

/// Extract an RFC 6749 `error` (with optional `error_description`) from an
/// error response body.
fn rfc_error_reason(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let error = value.get("error")?.as_str()?;
    let description = value.get("error_description").and_then(Value::as_str);
    Some(match description {
        Some(desc) if !desc.is_empty() => format!("{error}: {desc}"),
        _ => error.to_string(),
    })
}

/// Keep error surfaces bounded (SpanPrivacy: never echo a full response that
/// could carry token material).
fn truncate(body: &str) -> String {
    const MAX: usize = 200;
    if body.len() <= MAX {
        body.to_string()
    } else {
        format!("{}…", &body[..MAX])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------
    // Transport security policy
    // ---------------------------------------------------------------------

    #[test]
    fn https_issuer_is_accepted() {
        assert!(is_https("https://idp.example.com/realms/rigorix"));
        assert!(validate_endpoint_url("issuer", "https://idp.example.com").is_ok());
    }

    #[test]
    fn loopback_http_is_accepted_for_local_mocks() {
        assert!(is_loopback_http("http://127.0.0.1:8080/realms/rigorix"));
        assert!(is_loopback_http("http://localhost:8080"));
        assert!(validate_endpoint_url("issuer", "http://127.0.0.1:8080").is_ok());
    }

    #[test]
    fn plain_http_non_loopback_is_rejected() {
        assert!(!is_loopback_http("http://idp.example.com"));
        let err = validate_endpoint_url("issuer", "http://idp.example.com").unwrap_err();
        assert!(matches!(err, AuthError::Configuration(_)));
    }

    #[test]
    fn client_constructor_enforces_policy() {
        assert!(HttpIdpClient::new("https://idp.example.com").is_ok());
        assert!(HttpIdpClient::new("http://127.0.0.1:8080").is_ok());
        let err = match HttpIdpClient::new("http://idp.example.com") {
            Ok(_) => panic!("plain-HTTP issuer must be rejected"),
            Err(e) => e,
        };
        assert!(matches!(err, AuthError::Configuration(_)));
    }

    // ---------------------------------------------------------------------
    // Discovery parsing
    // ---------------------------------------------------------------------

    #[test]
    fn parses_discovery_document() {
        let body = r#"{
            "issuer": "https://idp.example.com",
            "device_authorization_endpoint": "https://idp.example.com/device_authorization",
            "token_endpoint": "https://idp.example.com/token",
            "revocation_endpoint": "https://idp.example.com/revoke",
            "jwks_uri": "https://idp.example.com/jwks"
        }"#;
        let meta = parse_discovery(body, "https://idp.example.com").unwrap();
        assert_eq!(meta.issuer, "https://idp.example.com");
        assert_eq!(
            meta.device_authorization_endpoint,
            "https://idp.example.com/device_authorization"
        );
        assert!(meta.revocation_endpoint.is_some());
        assert!(meta.jwks_uri.is_some());
    }

    #[test]
    fn discovery_issuer_mismatch_is_rejected() {
        let body = r#"{
            "issuer": "https://evil.example.com",
            "device_authorization_endpoint": "https://evil.example.com/device",
            "token_endpoint": "https://evil.example.com/token"
        }"#;
        let err = parse_discovery(body, "https://idp.example.com").unwrap_err();
        assert!(matches!(err, AuthError::Discovery { .. }));
        // No confusion: the configured issuer is never silently overridden.
        assert!(format!("{err}").contains("idp.example.com"));
    }

    #[test]
    fn discovery_rejects_http_endpoints_off_loopback() {
        let body = r#"{
            "issuer": "https://idp.example.com",
            "device_authorization_endpoint": "http://idp.example.com/device",
            "token_endpoint": "https://idp.example.com/token"
        }"#;
        let err = parse_discovery(body, "https://idp.example.com").unwrap_err();
        assert!(matches!(err, AuthError::Discovery { .. }));
    }

    #[test]
    fn discovery_malformed_json_is_discovery_error() {
        let err = parse_discovery("{nope", "https://idp.example.com").unwrap_err();
        assert!(matches!(err, AuthError::Discovery { .. }));
    }

    // ---------------------------------------------------------------------
    // Device authorization parsing
    // ---------------------------------------------------------------------

    #[test]
    fn parses_device_authorization_with_default_interval() {
        let body = r#"{
            "device_code": "dc-123",
            "user_code": "ABCD-EFGH",
            "verification_uri": "https://idp.example.com/device",
            "expires_in": 600,
            "interval": 5
        }"#;
        let auth = parse_device_authorization(body).unwrap();
        assert_eq!(auth.device_code.expose(), "dc-123");
        assert_eq!(auth.user_code, "ABCD-EFGH");
        assert_eq!(auth.expires_in, 600);
        assert_eq!(auth.interval_secs, 5);
        // Device code never renders.
        assert!(!format!("{auth:?}").contains("dc-123"));
    }

    #[test]
    fn device_authorization_missing_fields_are_rejected() {
        let err = parse_device_authorization(r#"{"device_code": "x"}"#).unwrap_err();
        assert!(matches!(err, AuthError::DeviceAuthorizationRejected(_)));
    }

    // ---------------------------------------------------------------------
    // Token response parsing
    // ---------------------------------------------------------------------

    #[test]
    fn parses_successful_token_response() {
        let body = r#"{
            "access_token": "at-1",
            "refresh_token": "rt-1",
            "expires_in": 900,
            "token_type": "Bearer",
            "scope": "openid offline_access"
        }"#;
        let resp = parse_token_response(body).unwrap();
        assert_eq!(resp.access_token.expose(), "at-1");
        assert_eq!(
            resp.refresh_token.as_ref().unwrap().expose(),
            "rt-1",
            "refresh token preserved"
        );
        assert_eq!(resp.expires_in, 900);
        assert_eq!(resp.token_type, "Bearer");
        // No token material ever renders.
        assert!(!format!("{resp:?}").contains("rt-1"));
        assert!(!format!("{resp:?}").contains("at-1"));
    }

    #[test]
    fn token_response_without_refresh_token_is_ok_for_other_grants() {
        let body = r#"{"access_token": "at-1", "expires_in": 900}"#;
        let resp = parse_token_response(body).unwrap();
        assert!(resp.refresh_token.is_none());
    }

    // ---------------------------------------------------------------------
    // RFC 8628 token-endpoint error mapping (§3.5)
    // ---------------------------------------------------------------------

    #[test]
    fn rfc_in_progress_codes_map_to_typed_outcomes() {
        match parse_token_error(r#"{"error":"authorization_pending"}"#) {
            TokenPoll::Pending {
                retry_after_secs: None,
            } => {}
            other => panic!("expected pending, got {other:?}"),
        }
        match parse_token_error(r#"{"error":"slow_down"}"#) {
            TokenPoll::Pending {
                retry_after_secs: Some(backoff),
            } => assert_eq!(backoff, DEFAULT_POLL_INTERVAL_SECS),
            other => panic!("expected slow_down pending, got {other:?}"),
        }
        match parse_token_error(r#"{"error":"access_denied","error_description":"user declined"}"#)
        {
            TokenPoll::AccessDenied { reason } => assert_eq!(reason, "user declined"),
            other => panic!("expected denied, got {other:?}"),
        }
        match parse_token_error(r#"{"error":"expired_token"}"#) {
            TokenPoll::Expired => {}
            other => panic!("expected expired, got {other:?}"),
        }
        // Unknown codes degrade to a typed, explicit denial — never a panic.
        match parse_token_error(r#"{"error":"invalid_grant"}"#) {
            TokenPoll::AccessDenied { reason } => assert!(reason.contains("invalid_grant")),
            other => panic!("expected denied fallback, got {other:?}"),
        }
    }

    #[test]
    fn rfc_error_reason_extracts_error_and_description() {
        assert_eq!(
            rfc_error_reason(r#"{"error":"invalid_client","error_description":"bad creds"}"#)
                .as_deref(),
            Some("invalid_client: bad creds")
        );
        assert_eq!(
            rfc_error_reason(r#"{"error":"invalid_client"}"#).as_deref(),
            Some("invalid_client")
        );
        assert!(rfc_error_reason("not json").is_none());
    }
}
