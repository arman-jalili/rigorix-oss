//! Integration tests for AuthService (Application) — ISSUE-AUTH-1.
//!
//! Exercises the full identity lifecycle through `AuthServiceImpl` with fake
//! IdP/keychain/token ports AND the **real** engine `IdentityAttestationService`
//! (rigorix-engine, offline NullVerifier) — proving the cross-crate
//! attestation seam (ADR-012): OSS auth attests, engine builds the claim.
//!
//! @canonical .pi/architecture/modules/auth.md
//! Implements: ISSUE-AUTH-1 acceptance — device flow → custody → status →
//! refresh → logout against real claim extraction (auth.md AC#1, #3, #4, #5, #8)

#![allow(unused_imports)]

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};

use rigorix_engine::identity::IdentityAttestationServiceImpl;
use rigorix_engine::identity::IdentitySource;
use rigorix_mcp::auth::application::dto::{
    LoginInput, LogoutInput, PollInput, RefreshInput, StatusInput,
};
use rigorix_mcp::auth::application::service::AuthService;
use rigorix_mcp::auth::application::service_impl::AuthServiceImpl;
use rigorix_mcp::auth::domain::IdpConfig;
use rigorix_mcp::auth::domain::flow::DeviceFlowStatus;
use rigorix_mcp::auth::domain::status::TokenStatus;
use rigorix_mcp::auth::domain::value::Secret;
use rigorix_mcp::auth::infrastructure::keychain_store::{
    KeychainStore, REFRESH_TOKEN_ACCOUNT, RIGORIX_KEYCHAIN_SERVICE,
};
use rigorix_mcp::auth::infrastructure::token_provider::TokenProvider;
use rigorix_mcp::auth::infrastructure::{
    DeviceAuthorization, IdpClient, IdpMetadata, TokenPoll, TokenResponse,
};

// ---------------------------------------------------------------------------
// Minimal base64url encoder (RFC 7515 §2) — enough to mint a test JWT.
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

/// Mint an unsigned JWT (`alg: none`) with the claim fields the engine's
/// `extract_claims` reads: sub, iss, exp, iat, roles, auth_method.
fn make_jwt(subject: &str, issuer: &str) -> String {
    let header = br#"{"alg":"none","typ":"JWT"}"#;
    let now = Utc::now();
    let exp = now + ChronoDuration::seconds(900);
    let payload = serde_json::json!({
        "sub": subject,
        "iss": issuer,
        "iat": now.timestamp(),
        "exp": exp.timestamp(),
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
// Fake ports (the real IdpClient/KeychainStore/TokenProvider implementations
// land in their own issues — #822–#824)
// ---------------------------------------------------------------------------

struct FakeIdp {
    /// Access token minted by this fake IdP (real JWT for attestation).
    jwt: String,
    refresh_calls: Mutex<usize>,
    revoked: Mutex<bool>,
}

impl FakeIdp {
    fn new(jwt: String) -> Self {
        Self {
            jwt,
            refresh_calls: Mutex::new(0),
            revoked: Mutex::new(false),
        }
    }

    fn token_response(&self) -> TokenResponse {
        TokenResponse {
            access_token: Secret::new(self.jwt.clone()),
            refresh_token: Some(Secret::new("refresh-token-integration".into())),
            expires_in: 900,
            token_type: "Bearer".into(),
            scope: Some("openid".into()),
        }
    }
}

#[async_trait]
impl IdpClient for FakeIdp {
    async fn discover(&self) -> Result<IdpMetadata, rigorix_mcp::auth::domain::error::AuthError> {
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
    ) -> Result<DeviceAuthorization, rigorix_mcp::auth::domain::error::AuthError> {
        Ok(DeviceAuthorization {
            device_code: Secret::new("device-code-integration".into()),
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
    ) -> Result<TokenPoll, rigorix_mcp::auth::domain::error::AuthError> {
        Ok(TokenPoll::Succeeded(self.token_response()))
    }

    async fn refresh_token(
        &self,
        _refresh_token: &Secret<String>,
        _client_id: &str,
    ) -> Result<TokenResponse, rigorix_mcp::auth::domain::error::AuthError> {
        *self.refresh_calls.lock().unwrap() += 1;
        Ok(self.token_response())
    }

    async fn revoke_token(
        &self,
        _refresh_token: &Secret<String>,
        _client_id: &str,
    ) -> Result<(), rigorix_mcp::auth::domain::error::AuthError> {
        *self.revoked.lock().unwrap() = true;
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
    ) -> Result<(), rigorix_mcp::auth::domain::error::AuthError> {
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
    ) -> Result<Option<Secret<String>>, rigorix_mcp::auth::domain::error::AuthError> {
        Ok(self
            .entries
            .lock()
            .unwrap()
            .get(&(service.into(), account.into()))
            .cloned()
            .map(Secret::new))
    }

    async fn delete_refresh_token(
        &self,
        service: &str,
        account: &str,
    ) -> Result<(), rigorix_mcp::auth::domain::error::AuthError> {
        self.entries
            .lock()
            .unwrap()
            .remove(&(service.into(), account.into()));
        Ok(())
    }
}

#[derive(Default)]
struct FakeTokenProvider {
    state: Mutex<Option<(String, chrono::DateTime<Utc>)>>,
}

#[async_trait]
impl TokenProvider for FakeTokenProvider {
    async fn current_access_token(&self) -> Option<Secret<String>> {
        let guard = self.state.lock().unwrap();
        let (token, expires_at) = guard.as_ref()?;
        if *expires_at <= Utc::now() {
            return None;
        }
        Some(Secret::new(token.clone()))
    }

    async fn set_access_token(
        &self,
        token: Secret<String>,
        expires_at: chrono::DateTime<Utc>,
    ) -> Result<(), rigorix_mcp::auth::domain::error::AuthError> {
        *self.state.lock().unwrap() = Some((token.expose().clone(), expires_at));
        Ok(())
    }

    async fn access_token_expires_at(&self) -> Option<chrono::DateTime<Utc>> {
        self.state.lock().unwrap().as_ref().map(|(_, at)| *at)
    }

    async fn clear(&self) {
        *self.state.lock().unwrap() = None;
    }
}

// ---------------------------------------------------------------------------
// Lifecycle integration tests
// ---------------------------------------------------------------------------

const ISSUER: &str = "https://idp.example.com/realms/rigorix";

fn harness() -> (
    AuthServiceImpl,
    Arc<FakeIdp>,
    Arc<FakeKeychain>,
    Arc<FakeTokenProvider>,
) {
    let config = IdpConfig::new(ISSUER.into(), "rigorix-cli".into(), None, None).unwrap();
    let idp = Arc::new(FakeIdp::new(make_jwt("user@org", ISSUER)));
    let keychain = Arc::new(FakeKeychain::default());
    let tokens = Arc::new(FakeTokenProvider::default());
    // Real engine attestation service (offline NullVerifier) — cross-crate.
    let attestation = Arc::new(IdentityAttestationServiceImpl::new());
    let service = AuthServiceImpl::new(
        config,
        idp.clone(),
        keychain.clone(),
        tokens.clone(),
        attestation,
    );
    (service, idp, keychain, tokens)
}

/// Full lifecycle: login → poll → authorized → status → refresh → logout.
#[tokio::test]
async fn device_flow_to_attested_identity_lifecycle() {
    let (service, idp, keychain, tokens) = harness();

    // AC#1 — login returns verification info without blocking.
    let login = service.login(&LoginInput::default()).await.unwrap();
    assert_eq!(login.status, DeviceFlowStatus::Pending);
    assert_eq!(login.user_code, "WXYZ-9876");
    assert_eq!(login.expires_in, 600);

    // AC#1 — polling succeeds; custody is persisted (keychain + provider).
    let poll = service.poll(&PollInput::default()).await.unwrap();
    assert_eq!(poll.status, DeviceFlowStatus::Authorized);
    assert!(poll.claim_summary.is_some());
    let stored = keychain
        .get_refresh_token(RIGORIX_KEYCHAIN_SERVICE, REFRESH_TOKEN_ACCOUNT)
        .await
        .unwrap()
        .expect("refresh token stored in keychain");
    assert_eq!(stored.expose(), "refresh-token-integration");
    let cached = tokens
        .current_access_token()
        .await
        .expect("access token cached");
    assert!(!cached.expose().is_empty());

    // AC#3 — status is authenticated with a REAL engine-extracted claim.
    // Offline NullVerifier degrades verification explicitly (AC#8) — the
    // claim content (sub/iss/lifetime) is real, the source marker honest.
    let status = service.status(&StatusInput::default()).await.unwrap();
    assert_eq!(status.status, TokenStatus::Authenticated);
    let summary = status.claim_summary.expect("claim summary present");
    assert_eq!(summary.subject, "user@org");
    assert_eq!(summary.issuer, ISSUER);
    assert!(summary.expires_at.is_some());
    assert_eq!(status.source, IdentitySource::Unverified);

    // attest() produces the shared IdentityClaim (ADR-012 seam).
    let claim = service.attest().await.unwrap();
    assert_eq!(claim.subject, "user@org");
    assert_eq!(claim.issuer, ISSUER);
    assert_eq!(claim.auth_method.as_deref(), Some("device_code"));

    // AC#4 — expired access token silently refreshed via the keychain.
    tokens
        .set_access_token(
            Secret::new(make_jwt("user@org", ISSUER)),
            Utc::now() - ChronoDuration::seconds(30), // past expiry
        )
        .await
        .unwrap();
    let status = service.status(&StatusInput::default()).await.unwrap();
    assert_eq!(status.status, TokenStatus::Expired);
    let refreshed = service.refresh(&RefreshInput::default()).await.unwrap();
    assert_eq!(refreshed.status, TokenStatus::Authenticated);
    assert_eq!(*idp.refresh_calls.lock().unwrap(), 1);
    let status = service.status(&StatusInput::default()).await.unwrap();
    assert_eq!(status.status, TokenStatus::Authenticated);

    // AC#5 — logout clears keychain + memory and revokes at the IdP.
    let logout = service.logout(&LogoutInput::default()).await.unwrap();
    assert_eq!(logout.status, "logged_out");
    assert!(
        keychain
            .get_refresh_token(RIGORIX_KEYCHAIN_SERVICE, REFRESH_TOKEN_ACCOUNT)
            .await
            .unwrap()
            .is_none()
    );
    assert!(tokens.current_access_token().await.is_none());
    assert!(*idp.revoked.lock().unwrap());

    // Post-logout everything reports unauthenticated.
    let status = service.status(&StatusInput::default()).await.unwrap();
    assert_eq!(status.status, TokenStatus::Unauthenticated);
    assert_eq!(status.source, IdentitySource::Unverified);
}
