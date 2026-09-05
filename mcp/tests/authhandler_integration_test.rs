//! Integration tests for AuthHandler (Interfaces) — ISSUE-AUTH-5.
//!
//! Exercises the REAL AuthToolHandlerImpl over AuthServiceImpl composed with
//! the REAL InMemoryTokenProvider (fake IdP/keychain ports) — asserting the
//! frozen MCP output JSON contracts (auth.md API Endpoints table) and the
//! login-completion UX where rigorix_auth_status advances the device flow.
//!
//! @canonical .pi/architecture/modules/auth.md#authhandler--sse-auth-interfaces
//! Implements: ISSUE-AUTH-5 — redacted tool outputs (never a raw token)

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;

use rigorix_engine::identity::IdentityAttestationServiceImpl;
use rigorix_mcp::auth::application::service::AuthService;
use rigorix_mcp::auth::application::service_impl::AuthServiceImpl;
use rigorix_mcp::auth::domain::IdpConfig;
use rigorix_mcp::auth::domain::error::AuthError;
use rigorix_mcp::auth::domain::value::Secret;
use rigorix_mcp::auth::infrastructure::keychain_store::KeychainStore;
use rigorix_mcp::auth::infrastructure::{
    DeviceAuthorization, IdpClient, IdpMetadata, InMemoryTokenProvider, TokenPoll, TokenResponse,
};
use rigorix_mcp::auth::interfaces::mcp::AuthToolHandler;

// ---------------------------------------------------------------------------
// Minimal base64url encoder (RFC 7515 §2) — mint an unsigned test JWT so the
// real engine attestation service can extract a claim from the access token.
// ---------------------------------------------------------------------------

const B64URL_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn b64url(data: &[u8]) -> String {
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        out.push(B64URL_ALPHABET[(b[0] >> 2) as usize] as char);
        out.push(B64URL_ALPHABET[(((b[0] & 0x03) << 4) | (b[1] >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64URL_ALPHABET[(((b[1] & 0x0F) << 2) | (b[2] >> 6)) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(B64URL_ALPHABET[(b[2] & 0x3F) as usize] as char);
        }
    }
    out
}

fn make_jwt(subject: &str, issuer: &str) -> String {
    let header = br#"{"alg":"none","typ":"JWT"}"#;
    let now = chrono::Utc::now();
    let payload = serde_json::json!({
        "sub": subject,
        "iss": issuer,
        "iat": now.timestamp(),
        "exp": (now + chrono::Duration::seconds(900)).timestamp(),
        "roles": ["rigorix-user"],
        "auth_method": "device_code"
    });
    format!(
        "{}.{}.sig",
        b64url(header),
        b64url(&serde_json::to_vec(&payload).unwrap())
    )
}

// ---------------------------------------------------------------------------
// Fake ports
// ---------------------------------------------------------------------------

/// Fake IdP: first poll pending, second poll succeeds.
struct FakeIdp {
    polls: Mutex<usize>,
}

impl Default for FakeIdp {
    fn default() -> Self {
        Self {
            polls: Mutex::new(0),
        }
    }
}

impl FakeIdp {
    /// A real (unsigned) JWT the engine can extract claims from.
    fn minted_token(&self) -> String {
        make_jwt("user@org", "https://idp.example.com/realms/rigorix")
    }
}

#[async_trait]
impl IdpClient for FakeIdp {
    async fn discover(&self) -> Result<IdpMetadata, AuthError> {
        Ok(IdpMetadata {
            issuer: "https://idp.example.com/realms/rigorix".into(),
            device_authorization_endpoint: "https://idp.example.com/device".into(),
            token_endpoint: "https://idp.example.com/token".into(),
            revocation_endpoint: None,
            jwks_uri: None,
        })
    }

    async fn device_authorization(
        &self,
        _client_id: &str,
    ) -> Result<DeviceAuthorization, AuthError> {
        Ok(DeviceAuthorization {
            device_code: Secret::new("device-code-1".into()),
            user_code: "WXYZ-9876".into(),
            verification_uri: "https://idp.example.com/device".into(),
            expires_in: 600,
            interval_secs: 5,
            issued_at: Utc::now(),
        })
    }

    async fn poll_token(
        &self,
        _device_code: &Secret<String>,
        _client_id: &str,
    ) -> Result<TokenPoll, AuthError> {
        let mut polls = self.polls.lock().unwrap();
        *polls += 1;
        if *polls == 1 {
            Ok(TokenPoll::Pending {
                retry_after_secs: Some(5),
            })
        } else {
            Ok(TokenPoll::Succeeded(TokenResponse {
                access_token: Secret::new(self.minted_token()),
                refresh_token: Some(Secret::new("refresh-token-1".into())),
                expires_in: 900,
                token_type: "Bearer".into(),
                scope: None,
            }))
        }
    }

    async fn refresh_token(
        &self,
        _refresh_token: &Secret<String>,
        _client_id: &str,
    ) -> Result<TokenResponse, AuthError> {
        Err(AuthError::NotAuthenticated)
    }

    async fn revoke_token(
        &self,
        _refresh_token: &Secret<String>,
        _client_id: &str,
    ) -> Result<(), AuthError> {
        Ok(())
    }
}

#[derive(Default)]
struct FakeKeychain {
    entries: Mutex<HashMap<(String, String), String>>,
}

#[async_trait]
impl KeychainStore for FakeKeychain {
    async fn store_refresh_token(
        &self,
        service: &str,
        account: &str,
        token: &Secret<String>,
    ) -> Result<(), AuthError> {
        self.entries
            .lock()
            .unwrap()
            .insert((service.into(), account.into()), token.expose().clone());
        Ok(())
    }

    async fn get_refresh_token(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<Secret<String>>, AuthError> {
        Ok(self
            .entries
            .lock()
            .unwrap()
            .get(&(service.into(), account.into()))
            .cloned()
            .map(Secret::new))
    }

    async fn delete_refresh_token(&self, service: &str, account: &str) -> Result<(), AuthError> {
        self.entries
            .lock()
            .unwrap()
            .remove(&(service.into(), account.into()));
        Ok(())
    }
}

fn handler() -> impl AuthToolHandler {
    let config = IdpConfig::new(
        "https://idp.example.com/realms/rigorix".into(),
        "rigorix-cli".into(),
        None,
        None,
    )
    .unwrap();
    let service: Arc<dyn AuthService> = Arc::new(AuthServiceImpl::new(
        config,
        Arc::new(FakeIdp::default()),
        Arc::new(FakeKeychain::default()),
        Arc::new(InMemoryTokenProvider::new()),
        Arc::new(IdentityAttestationServiceImpl::new()),
    ));
    rigorix_mcp::auth::interfaces::mcp::AuthToolHandlerImpl::new(service)
}

// ---------------------------------------------------------------------------
// Frozen MCP output contract tests (auth.md API Endpoints table)
// ---------------------------------------------------------------------------

/// rigorix_auth_login → `{ status, verification_uri, user_code, expires_in }`.
#[tokio::test]
async fn login_output_matches_frozen_contract() {
    let handler = handler();
    let out = handler
        .handle_auth_login(serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(out["status"], "pending");
    assert_eq!(out["verification_uri"], "https://idp.example.com/device");
    assert_eq!(out["user_code"], "WXYZ-9876");
    assert_eq!(out["expires_in"], 600);
    // Redacted output: never a raw token/device code anywhere.
    assert!(!out.to_string().contains("device-code"));
}

/// Optional client_id/issuer overrides are accepted by the login tool.
#[tokio::test]
async fn login_accepts_override_params() {
    let handler = handler();
    let out = handler
        .handle_auth_login(serde_json::json!({
            "client_id": "alternate-cli",
            "issuer": "https://idp.example.com/realms/rigorix"
        }))
        .await
        .unwrap();
    assert_eq!(out["status"], "pending");
}

/// rigorix_auth_status advances the in-flight device flow (login completion
/// UX) then reports `{ status, claim_summary, source }`.
#[tokio::test]
async fn status_drives_login_completion_and_matches_frozen_contract() {
    let handler = handler();
    let login = handler
        .handle_auth_login(serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(login["status"], "pending");

    // Status #1: advances the flow (poll #1 → pending) → still unauthenticated.
    let status1 = handler
        .handle_auth_status(serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(status1["status"], "unauthenticated");
    assert!(status1["claim_summary"].is_null());
    assert_eq!(status1["source"], "unverified");

    // Status #2: advances the flow (poll #2 → authorized + custody) →
    // authenticated with a claim summary.
    let status2 = handler
        .handle_auth_status(serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(status2["status"], "authenticated");
    assert!(status2["claim_summary"].is_object());
    assert_eq!(status2["source"], "unverified"); // offline NullVerifier marker

    // Polling is idempotent afterwards — flow is terminal.
    let service_driven = handler
        .handle_auth_status(serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(service_driven["status"], "authenticated");
}

/// rigorix_auth_logout → `{ status: "logged_out" }`.
#[tokio::test]
async fn logout_output_matches_frozen_contract() {
    let handler = handler();
    let out = handler
        .handle_auth_logout(serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(out["status"], "logged_out");

    // After logout, status is unauthenticated.
    let status = handler
        .handle_auth_status(serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(status["status"], "unauthenticated");
}

/// Status without any prior login stays unauthenticated (poll is a no-op).
#[tokio::test]
async fn status_without_login_is_unauthenticated() {
    let handler = handler();
    let out = handler
        .handle_auth_status(serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(out["status"], "unauthenticated");
    assert_eq!(out["source"], "unverified");
}
