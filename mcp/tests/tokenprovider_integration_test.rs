//! Integration tests for InMemoryTokenProvider (Infrastructure) — ISSUE-AUTH-4.
//!
//! The real token provider wired into AuthServiceImpl with fake IdP/keychain
//! ports proves the downstream contract: expired tokens are hidden (not
//! served) so the service reports `Expired` and silently refreshes from the
//! keychain refresh token (auth.md AC#4).
//!
//! @canonical .pi/architecture/modules/auth.md#tokenprovider-infrastructure
//! Implements: ISSUE-AUTH-4 — short-TTL custody contract (ADR-008)

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};

use rigorix_engine::identity::IdentityAttestationServiceImpl;
use rigorix_mcp::auth::application::dto::{LoginInput, PollInput, RefreshInput, StatusInput};
use rigorix_mcp::auth::application::service::AuthService;
use rigorix_mcp::auth::application::service_impl::AuthServiceImpl;
use rigorix_mcp::auth::domain::IdpConfig;
use rigorix_mcp::auth::domain::error::AuthError;
use rigorix_mcp::auth::domain::status::TokenStatus;
use rigorix_mcp::auth::domain::value::Secret;
use rigorix_mcp::auth::infrastructure::keychain_store::KeychainStore;
use rigorix_mcp::auth::infrastructure::token_provider::TokenProvider;
use rigorix_mcp::auth::infrastructure::{
    DeviceAuthorization, IdpClient, IdpMetadata, InMemoryTokenProvider, TokenPoll, TokenResponse,
};

// ---------------------------------------------------------------------------
// Fake IdP + keychain (TokenProvider is the component under test)
// ---------------------------------------------------------------------------

struct FakeIdp {
    token: String,
    refresh_calls: Mutex<usize>,
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
            user_code: "ABCD-EFGH".into(),
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
        Ok(TokenPoll::Succeeded(self.token_response()))
    }

    async fn refresh_token(
        &self,
        _refresh_token: &Secret<String>,
        _client_id: &str,
    ) -> Result<TokenResponse, AuthError> {
        *self.refresh_calls.lock().unwrap() += 1;
        Ok(self.token_response())
    }

    async fn revoke_token(
        &self,
        _refresh_token: &Secret<String>,
        _client_id: &str,
    ) -> Result<(), AuthError> {
        Ok(())
    }
}

impl FakeIdp {
    fn token_response(&self) -> TokenResponse {
        TokenResponse {
            access_token: Secret::new(self.token.clone()),
            refresh_token: Some(Secret::new("refresh-token-1".into())),
            expires_in: 900,
            token_type: "Bearer".into(),
            scope: None,
        }
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

fn harness() -> (AuthServiceImpl, Arc<InMemoryTokenProvider>, Arc<FakeIdp>) {
    let config = IdpConfig::new(
        "https://idp.example.com/realms/rigorix".into(),
        "rigorix-cli".into(),
        None,
        None,
    )
    .unwrap();
    let idp = Arc::new(FakeIdp {
        token: "access-token-live".into(),
        refresh_calls: Mutex::new(0),
    });
    let keychain = Arc::new(FakeKeychain::default());
    let tokens = Arc::new(InMemoryTokenProvider::new());
    let attestation = Arc::new(IdentityAttestationServiceImpl::new());
    let service = AuthServiceImpl::new(
        config,
        idp.clone(),
        keychain.clone(),
        tokens.clone(),
        attestation,
    );
    (service, tokens, idp)
}

/// The real provider drives the expired → hidden → silent-refresh cycle
/// through AuthService (AC#3 + AC#4 + ADR-008 TTL custody).
#[tokio::test]
async fn real_provider_serves_short_ttl_cycle_through_auth_service() {
    let (service, tokens, idp) = harness();

    // Login + poll: provider caches the short-TTL access token.
    service.login(&LoginInput::default()).await.unwrap();
    let poll = service.poll(&PollInput::default()).await.unwrap();
    assert_eq!(
        poll.status,
        rigorix_mcp::auth::domain::flow::DeviceFlowStatus::Authorized
    );
    let cached = tokens.current_access_token().await.expect("token cached");
    assert_eq!(cached.expose(), "access-token-live");
    assert!(tokens.access_token_expires_at().await.unwrap() > Utc::now());

    // Force expiry by overwriting with a past expiry (as a real provider
    // would after the TTL elapses).
    tokens
        .set_access_token(
            Secret::new("access-token-live".into()),
            Utc::now() - ChronoDuration::seconds(30),
        )
        .await
        .unwrap();

    // The expired token is HIDDEN, never served; status reports Expired.
    assert!(tokens.current_access_token().await.is_none());
    let status = service.status(&StatusInput::default()).await.unwrap();
    assert_eq!(status.status, TokenStatus::Expired);

    // Silent refresh from the keychain refresh token restores service.
    let refreshed = service.refresh(&RefreshInput::default()).await.unwrap();
    assert_eq!(refreshed.status, TokenStatus::Authenticated);
    assert_eq!(*idp.refresh_calls.lock().unwrap(), 1);
    assert!(tokens.current_access_token().await.is_some());
    let status = service.status(&StatusInput::default()).await.unwrap();
    assert_eq!(status.status, TokenStatus::Authenticated);

    // Logout clears the provider entirely.
    service
        .logout(&rigorix_mcp::auth::application::dto::LogoutInput::default())
        .await
        .unwrap();
    assert!(tokens.current_access_token().await.is_none());
    assert!(tokens.access_token_expires_at().await.is_none());
}
