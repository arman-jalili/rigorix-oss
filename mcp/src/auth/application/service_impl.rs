//! Concrete AuthService implementation — identity lifecycle orchestration.
//!
//! @canonical .pi/architecture/modules/auth.md#authservice-application
//! Implements: ISSUE-AUTH-1 — AuthService (Application)
//! Issue: #821
//! ADR-008: device flow lifecycle; ADR-012: attestation seam
//!
//! Composes the OIDC device flow ([`IdpClient`]), keychain custody
//! ([`KeychainStore`]), in-memory short-TTL access tokens
//! ([`TokenProvider`]), and the engine attestation service (ADR-012) into the
//! `AuthService` use case.
//!
//! # Behavior Contract (auth.md acceptance criteria)
//!
//! - `login` initiates the device flow (RFC 8628 §3.1–3.2) and returns
//!   `verification_uri` + `user_code` for display; it never blocks awaiting
//!   the human (AC#1: returned to caller)
//! - `poll` advances the flow (RFC 8628 §3.3–3.5); on `authorized` the
//!   refresh token is persisted to the keychain and the short-TTL access
//!   token cached in memory (AC#1). Denial/expiry are terminal, typed
//!   outcomes — never silent failures (AC#2)
//! - `status` reports authenticated/expired/unauthenticated transitions and
//!   degrades explicitly to `IdentitySource::Unverified` when no claim can be
//!   produced (AC#3, AC#8 — fail-open for local dev, ADR-008)
//! - `refresh` silently exchanges the keychain refresh token (AC#4)
//! - `logout` clears keychain + memory (AC#5)
//! - `attest` delegates token → `IdentityClaim` to the engine
//!   `IdentityAttestationService` — OSS attests, never authorizes (ADR-012)
//!
//! # Observability (SpanPrivacy)
//!
//! Domain events are recorded to the `rigorix::auth` tracing target. Event
//! payloads never contain raw token material.

use async_trait::async_trait;
use chrono::Utc;
use std::sync::{Arc, Mutex};
use tracing::info;

use rigorix_engine::identity::{AttestInput, IdentityAttestationService, IdentityClaim};

use crate::auth::domain::IdpConfig;
use crate::auth::domain::error::AuthError;
use crate::auth::domain::event::AuthEvent;
use crate::auth::domain::flow::{DeviceFlowState, DeviceFlowStatus};
use crate::auth::domain::status::TokenStatus;
use crate::auth::domain::value::ClaimSummary;
use crate::auth::infrastructure::keychain_store::{
    KeychainStore, REFRESH_TOKEN_ACCOUNT, RIGORIX_KEYCHAIN_SERVICE,
};
use crate::auth::infrastructure::token_provider::TokenProvider;
use crate::auth::infrastructure::{IdpClient, TokenPoll};
use rigorix_engine::identity::IdentitySource;

use super::dto::{
    LOGGED_OUT_STATUS, LoginInput, LoginOutput, LogoutInput, LogoutOutput, PollInput, PollOutput,
    RefreshInput, RefreshOutput, StatusInput, StatusOutput,
};
use super::service::AuthService;

/// Default OIDC auth method recorded on attested claims (device flow).
const DEVICE_FLOW_AUTH_METHOD: &str = "device_code";

/// Concrete [`AuthService`] implementation.
///
/// All four ports are injected (interface-first) so this service is
/// independent of any particular IdP, keychain backend, token store, or
/// attestation verifier. A single in-flight device flow is tracked in memory.
pub struct AuthServiceImpl {
    /// Configured IdP settings (issuer, client_id, TTL).
    config: IdpConfig,

    /// OIDC device-flow client port.
    idp: Arc<dyn IdpClient>,

    /// Refresh-token custody port (OS keychain).
    keychain: Arc<dyn KeychainStore>,

    /// Short-TTL access-token custody port (in-memory).
    tokens: Arc<dyn TokenProvider>,

    /// Engine attestation port (ADR-012 seam).
    attestation: Arc<dyn IdentityAttestationService>,

    /// The active device flow (single-session client-side module).
    pending: Mutex<Option<DeviceFlowState>>,
}

impl AuthServiceImpl {
    /// Create a new AuthServiceImpl from its injected ports.
    pub fn new(
        config: IdpConfig,
        idp: Arc<dyn IdpClient>,
        keychain: Arc<dyn KeychainStore>,
        tokens: Arc<dyn TokenProvider>,
        attestation: Arc<dyn IdentityAttestationService>,
    ) -> Self {
        Self {
            config,
            idp,
            keychain,
            tokens,
            attestation,
            pending: Mutex::new(None),
        }
    }

    /// Record a domain event to the auth tracing target (Logger consumer).
    ///
    /// Events are immutable facts; payloads are redacted by construction
    /// (never raw token material).
    fn record_event(&self, event: AuthEvent) {
        info!(target: "rigorix::auth", event = ?event);
    }

    /// Read the active device flow (clone — never holds the lock across await).
    fn pending_flow(&self) -> Option<DeviceFlowState> {
        self.pending.lock().ok().and_then(|guard| guard.clone())
    }

    /// Replace the active device flow.
    fn set_pending_flow(&self, flow: DeviceFlowState) {
        if let Ok(mut guard) = self.pending.lock() {
            *guard = Some(flow);
        }
    }

    /// Mark the active flow terminal (kept so repeated polls echo the outcome).
    fn mark_flow_terminal(&self, status: DeviceFlowStatus) {
        if let Ok(mut guard) = self.pending.lock()
            && let Some(flow) = guard.as_mut()
        {
            flow.status = status;
        }
    }

    /// Best-effort claim summary for the current access token.
    ///
    /// Degrades explicitly: an opaque/unparseable token or unreachable
    /// attestation yields `(None, IdentitySource::Unverified)` — never an
    /// error (ADR-008 fail-open, AC#8).
    async fn summarize_current_claim(&self) -> (Option<ClaimSummary>, IdentitySource) {
        match self.attest().await {
            Ok(claim) => (Some(ClaimSummary::from(&claim)), claim.source),
            Err(_) => (None, IdentitySource::Unverified),
        }
    }

    /// Attest the given access token directly (no TokenProvider round-trip).
    async fn attest_token(&self, token: &str) -> Result<IdentityClaim, AuthError> {
        let input = AttestInput {
            token: Some(token.to_string()),
            principal: None,
            issuer: Some(self.config.issuer().to_string()),
            auth_method: Some(DEVICE_FLOW_AUTH_METHOD.to_string()),
        };
        self.attestation
            .attest(input)
            .await
            .map_err(|e| AuthError::Attestation(e.to_string()))
    }
}

#[async_trait]
impl AuthService for AuthServiceImpl {
    async fn login(&self, input: &LoginInput) -> Result<LoginOutput, AuthError> {
        // Optional client_id override (bootstrap); issuer overrides are not
        // supported against the single configured IdP client.
        let client_id = input
            .client_id
            .as_deref()
            .unwrap_or(self.config.client_id());
        if let Some(issuer) = input.issuer.as_deref()
            && issuer != self.config.issuer()
        {
            return Err(AuthError::Configuration(format!(
                "issuer override {issuer} is not the configured IdP {} \
                 (single-IdP client; configure .rigorix/auth.toml instead)",
                self.config.issuer()
            )));
        }

        // RFC 8628 §3.1 — request a device code from the IdP.
        let authorization = self.idp.device_authorization(client_id).await?;

        let session_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let flow = DeviceFlowState {
            session_id: session_id.clone(),
            device_code: authorization.device_code.clone(),
            verification_uri: authorization.verification_uri.clone(),
            user_code: authorization.user_code.clone(),
            expires_in: authorization.expires_in,
            expires_at: now + chrono::Duration::seconds(authorization.expires_in as i64),
            interval_secs: authorization.interval_secs,
            status: DeviceFlowStatus::Pending,
        };
        self.set_pending_flow(flow);

        self.record_event(AuthEvent::AuthLoginStarted {
            session_id,
            verification_uri: authorization.verification_uri.clone(),
            user_code: authorization.user_code.clone(),
            timestamp: now,
        });

        Ok(LoginOutput {
            status: DeviceFlowStatus::Pending,
            verification_uri: authorization.verification_uri,
            user_code: authorization.user_code,
            expires_in: authorization.expires_in,
        })
    }

    async fn poll(&self, input: &PollInput) -> Result<PollOutput, AuthError> {
        let _ = input; // the active flow is implicit in service state
        let flow = self.pending_flow().ok_or(AuthError::NotAuthenticated)?;

        // Terminal flows echo their outcome (idempotent).
        if flow.status.is_terminal() {
            return Ok(PollOutput {
                status: flow.status,
                retry_after_secs: None,
                reason: terminal_reason(&flow.status),
                claim_summary: None,
            });
        }

        let now = Utc::now();
        if flow.expires_at <= now {
            self.mark_flow_terminal(DeviceFlowStatus::Expired);
            self.record_event(AuthEvent::AuthLoginFailed {
                session_id: flow.session_id.clone(),
                error_type: "expired".into(),
                reason: "device code expired before authorization (RFC 8628 §3.5 expired_token)"
                    .into(),
                timestamp: now,
            });
            return Ok(PollOutput {
                status: DeviceFlowStatus::Expired,
                retry_after_secs: None,
                reason: Some("device code expired".into()),
                claim_summary: None,
            });
        }

        // RFC 8628 §3.5 — poll the token endpoint.
        let client_id = self.config.client_id();
        let outcome = match self.idp.poll_token(&flow.device_code, client_id).await {
            Ok(outcome) => outcome,
            Err(e) if e.is_retriable() => {
                // Transient fault — keep the flow pending, caller retries.
                return Ok(PollOutput {
                    status: DeviceFlowStatus::Pending,
                    retry_after_secs: Some(flow.interval_secs),
                    reason: Some(format!("transport fault: {e}")),
                    claim_summary: None,
                });
            }
            Err(e) => return Err(e),
        };

        match outcome {
            TokenPoll::Pending { retry_after_secs } => Ok(PollOutput {
                status: DeviceFlowStatus::Pending,
                retry_after_secs: Some(retry_after_secs.unwrap_or(flow.interval_secs)),
                reason: None,
                claim_summary: None,
            }),

            TokenPoll::AccessDenied { reason } => {
                self.mark_flow_terminal(DeviceFlowStatus::Denied);
                self.record_event(AuthEvent::AuthLoginFailed {
                    session_id: flow.session_id.clone(),
                    error_type: "access_denied".into(),
                    reason: reason.clone(),
                    timestamp: Utc::now(),
                });
                Ok(PollOutput {
                    status: DeviceFlowStatus::Denied,
                    retry_after_secs: None,
                    reason: Some(reason),
                    claim_summary: None,
                })
            }

            TokenPoll::Expired => {
                self.mark_flow_terminal(DeviceFlowStatus::Expired);
                self.record_event(AuthEvent::AuthLoginFailed {
                    session_id: flow.session_id.clone(),
                    error_type: "expired_token".into(),
                    reason: "device code expired before authorization".into(),
                    timestamp: Utc::now(),
                });
                Ok(PollOutput {
                    status: DeviceFlowStatus::Expired,
                    retry_after_secs: None,
                    reason: Some("device code expired".into()),
                    claim_summary: None,
                })
            }

            TokenPoll::Succeeded(response) => {
                // Custody: refresh token → keychain (crown jewel, never in
                // readable files); access token → in-memory provider.
                let refresh_token = response.refresh_token.ok_or_else(|| {
                    AuthError::InvalidTokenResponse(
                        "token response carried no refresh_token (device flow requires \
                         offline_access scope)"
                            .into(),
                    )
                })?;
                self.keychain
                    .store_refresh_token(
                        RIGORIX_KEYCHAIN_SERVICE,
                        REFRESH_TOKEN_ACCOUNT,
                        &refresh_token,
                    )
                    .await?;

                let now = Utc::now();
                let ttl_secs = if response.expires_in > 0 {
                    response.expires_in
                } else {
                    self.config.access_token_ttl_secs()
                };
                self.tokens
                    .set_access_token(
                        response.access_token,
                        now + chrono::Duration::seconds(ttl_secs as i64),
                    )
                    .await?;

                self.mark_flow_terminal(DeviceFlowStatus::Authorized);

                // Best-effort claim summary for the success event/payload.
                let token = self
                    .tokens
                    .current_access_token()
                    .await
                    .ok_or(AuthError::NotAuthenticated)?;
                let (subject, issuer, claim_summary) = match self.attest_token(token.expose()).await
                {
                    Ok(claim) => {
                        let summary = ClaimSummary::from(&claim);
                        (
                            summary.subject.clone(),
                            summary.issuer.clone(),
                            Some(summary),
                        )
                    }
                    Err(_) => (
                        "unknown".to_string(),
                        self.config.issuer().to_string(),
                        None,
                    ),
                };

                self.record_event(AuthEvent::AuthLoginSucceeded {
                    session_id: flow.session_id,
                    subject,
                    issuer,
                    token_ttl_secs: ttl_secs,
                    timestamp: now,
                });

                Ok(PollOutput {
                    status: DeviceFlowStatus::Authorized,
                    retry_after_secs: None,
                    reason: None,
                    claim_summary,
                })
            }
        }
    }

    async fn status(&self, input: &StatusInput) -> Result<StatusOutput, AuthError> {
        let _ = input;
        let now = Utc::now();
        let expires_at = self.tokens.access_token_expires_at().await;

        // Expired-token marker takes precedence (TokenProvider hides expired
        // tokens, so expiry must be read from the provider metadata).
        if let Some(expires_at) = expires_at
            && expires_at <= now
        {
            self.record_event(AuthEvent::AuthStatusChecked {
                session_id: uuid::Uuid::new_v4().to_string(),
                status: TokenStatus::Expired,
                claim_summary: None,
                timestamp: now,
            });
            return Ok(StatusOutput {
                status: TokenStatus::Expired,
                claim_summary: None,
                source: IdentitySource::Unverified,
            });
        }

        let token = self.tokens.current_access_token().await;
        let output = match token {
            Some(_) => {
                let (claim_summary, source) = self.summarize_current_claim().await;
                StatusOutput {
                    status: TokenStatus::Authenticated,
                    claim_summary,
                    source,
                }
            }
            None => StatusOutput {
                status: TokenStatus::Unauthenticated,
                claim_summary: None,
                source: IdentitySource::Unverified,
            },
        };

        self.record_event(AuthEvent::AuthStatusChecked {
            session_id: uuid::Uuid::new_v4().to_string(),
            status: output.status,
            claim_summary: output.claim_summary.clone(),
            timestamp: Utc::now(),
        });
        Ok(output)
    }

    async fn refresh(&self, input: &RefreshInput) -> Result<RefreshOutput, AuthError> {
        let _ = input;
        let refresh_token = self
            .keychain
            .get_refresh_token(RIGORIX_KEYCHAIN_SERVICE, REFRESH_TOKEN_ACCOUNT)
            .await?
            .ok_or(AuthError::NotAuthenticated)?;

        // Silent exchange (RFC 6749 §6) — no user interaction.
        let response = self
            .idp
            .refresh_token(&refresh_token, self.config.client_id())
            .await
            .map_err(|e| match e {
                AuthError::RefreshFailed(_) | AuthError::Transport(_) => e,
                other => AuthError::RefreshFailed(other.to_string()),
            })?;

        let now = Utc::now();
        let expires_in = if response.expires_in > 0 {
            response.expires_in
        } else {
            self.config.access_token_ttl_secs()
        };
        self.tokens
            .set_access_token(
                response.access_token,
                now + chrono::Duration::seconds(expires_in as i64),
            )
            .await?;

        Ok(RefreshOutput {
            status: TokenStatus::Authenticated,
            expires_in_secs: Some(expires_in),
        })
    }

    async fn logout(&self, input: &LogoutInput) -> Result<LogoutOutput, AuthError> {
        let _ = input;
        let mut revoked = false;

        // Revoke at the IdP (RFC 7009) — best-effort: a transport fault must
        // not strand the local logout.
        if let Some(refresh_token) = self
            .keychain
            .get_refresh_token(RIGORIX_KEYCHAIN_SERVICE, REFRESH_TOKEN_ACCOUNT)
            .await?
        {
            match self
                .idp
                .revoke_token(&refresh_token, self.config.client_id())
                .await
            {
                Ok(()) => revoked = true,
                Err(AuthError::Transport(reason)) => {
                    info!(target: "rigorix::auth", reason, "refresh token revocation skipped (IdP unreachable)");
                }
                Err(e) => {
                    info!(target: "rigorix::auth", error = ?e, "refresh token revocation skipped");
                }
            }
        }

        // Clear custody: keychain credential + in-memory token.
        self.keychain
            .delete_refresh_token(RIGORIX_KEYCHAIN_SERVICE, REFRESH_TOKEN_ACCOUNT)
            .await?;
        self.tokens.clear().await;

        self.record_event(AuthEvent::AuthLoggedOut {
            session_id: uuid::Uuid::new_v4().to_string(),
            revoked,
            timestamp: Utc::now(),
        });

        Ok(LogoutOutput {
            status: LOGGED_OUT_STATUS.to_string(),
        })
    }

    async fn attest(&self) -> Result<IdentityClaim, AuthError> {
        // Expired access token → silent refresh from the keychain first
        // (AC#4), then attest the fresh token. Note: a contract-conforming
        // TokenProvider hides expired tokens (`None` once past expiry), so
        // expiry is read from provider metadata before the token itself.
        let expired = self
            .tokens
            .access_token_expires_at()
            .await
            .map(|expires_at| expires_at <= Utc::now())
            .unwrap_or(false);
        if expired {
            self.refresh(&RefreshInput::default()).await?;
        }

        let token = self
            .tokens
            .current_access_token()
            .await
            .ok_or(AuthError::NotAuthenticated)?;
        self.attest_token(token.expose()).await
    }
}

/// Human-readable reason text for terminal flow statuses.
fn terminal_reason(status: &DeviceFlowStatus) -> Option<String> {
    match status {
        DeviceFlowStatus::Denied => Some("device flow denied by the user".into()),
        DeviceFlowStatus::Expired => Some("device code expired".into()),
        DeviceFlowStatus::Authorized => Some("device flow completed".into()),
        DeviceFlowStatus::Pending => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::{DateTime, Duration as ChronoDuration};
    use rigorix_engine::identity::{
        AttestInput, IdentityClaim, IdentityError, IdentitySource, VerificationOutcome,
    };
    use std::collections::{HashMap, VecDeque};
    use std::sync::Mutex;

    use crate::auth::domain::value::Secret;
    use crate::auth::infrastructure::{
        DeviceAuthorization, IdpClient, IdpMetadata, TokenPoll, TokenResponse,
    };

    // ---------------------------------------------------------------------
    // Test doubles — implement the frozen ports
    // ---------------------------------------------------------------------

    /// Scripted poll step for the fake IdP (RFC 8628 §3.5 outcomes).
    #[derive(Debug, Clone)]
    enum PollStep {
        Pending,
        Succeed,
        Denied,
        Expired,
        TransportFault,
    }

    /// Fake OIDC device-flow client — scripted RFC 8628 behavior.
    struct FakeIdp {
        /// Poll steps consumed in order (last step repeats).
        polls: Mutex<VecDeque<PollStep>>,
        /// Token response returned on success/refresh.
        response: TokenResponse,
        /// Revocation flag.
        revoked: Mutex<bool>,
        /// Refresh-token exchange counter.
        refresh_calls: Mutex<usize>,
        /// Whether device_authorization errors with a transport fault.
        auth_transport_fault: Mutex<bool>,
        /// Whether refresh errors with a transport fault.
        refresh_transport_fault: Mutex<bool>,
    }

    impl FakeIdp {
        fn new() -> Self {
            Self {
                polls: Mutex::new(VecDeque::from([PollStep::Pending, PollStep::Succeed])),
                response: TokenResponse {
                    access_token: Secret::new("access-token-1".into()),
                    refresh_token: Some(Secret::new("refresh-token-1".into())),
                    expires_in: 900,
                    token_type: "Bearer".into(),
                    scope: Some("openid".into()),
                },
                revoked: Mutex::new(false),
                refresh_calls: Mutex::new(0),
                auth_transport_fault: Mutex::new(false),
                refresh_transport_fault: Mutex::new(false),
            }
        }

        fn with_poll_steps(mut self, steps: Vec<PollStep>) -> Self {
            self.polls = Mutex::new(VecDeque::from(steps));
            self
        }

        fn with_refresh_transport_fault(mut self) -> Self {
            self.refresh_transport_fault = Mutex::new(true);
            self
        }

        fn next_poll(&self) -> PollStep {
            let mut guard = self.polls.lock().unwrap();
            if guard.len() <= 1 {
                return guard.front().cloned().unwrap_or(PollStep::Pending);
            }
            guard.pop_front().unwrap()
        }
    }

    #[async_trait]
    impl IdpClient for FakeIdp {
        async fn discover(&self) -> Result<IdpMetadata, AuthError> {
            Ok(IdpMetadata {
                issuer: "https://idp.example.com/realms/rigorix".into(),
                device_authorization_endpoint: "https://idp.example.com/device".into(),
                token_endpoint: "https://idp.example.com/token".into(),
                revocation_endpoint: Some("https://idp.example.com/revoke".into()),
                jwks_uri: None,
            })
        }

        async fn device_authorization(
            &self,
            _client_id: &str,
        ) -> Result<DeviceAuthorization, AuthError> {
            if *self.auth_transport_fault.lock().unwrap() {
                return Err(AuthError::Transport("IdP unreachable".into()));
            }
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
            match self.next_poll() {
                PollStep::Pending => Ok(TokenPoll::Pending {
                    retry_after_secs: Some(5),
                }),
                PollStep::Succeed => Ok(TokenPoll::Succeeded(self.response.clone())),
                PollStep::Denied => Ok(TokenPoll::AccessDenied {
                    reason: "user declined the request".into(),
                }),
                PollStep::Expired => Ok(TokenPoll::Expired),
                PollStep::TransportFault => Err(AuthError::Transport("IdP unreachable".into())),
            }
        }

        async fn refresh_token(
            &self,
            _refresh_token: &Secret<String>,
            _client_id: &str,
        ) -> Result<TokenResponse, AuthError> {
            *self.refresh_calls.lock().unwrap() += 1;
            if *self.refresh_transport_fault.lock().unwrap() {
                return Err(AuthError::Transport("IdP unreachable".into()));
            }
            Ok(self.response.clone())
        }

        async fn revoke_token(
            &self,
            _refresh_token: &Secret<String>,
            _client_id: &str,
        ) -> Result<(), AuthError> {
            *self.revoked.lock().unwrap() = true;
            Ok(())
        }
    }

    /// Fake keychain — in-memory credential store (contract-conforming).
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

        async fn delete_refresh_token(
            &self,
            service: &str,
            account: &str,
        ) -> Result<(), AuthError> {
            self.entries
                .lock()
                .unwrap()
                .remove(&(service.into(), account.into()));
            Ok(())
        }
    }

    /// Fake in-memory token provider (contract-conforming: hides expired
    /// tokens behind the expiry metadata).
    #[derive(Default)]
    struct FakeTokenProvider {
        state: Mutex<Option<(String, DateTime<Utc>)>>,
    }

    #[async_trait]
    impl TokenProvider for FakeTokenProvider {
        async fn current_access_token(&self) -> Option<Secret<String>> {
            let guard = self.state.lock().unwrap();
            let (token, expires_at) = guard.as_ref()?;
            if *expires_at <= Utc::now() {
                return None; // expired — hidden (TokenProvider contract)
            }
            Some(Secret::new(token.clone()))
        }

        async fn set_access_token(
            &self,
            token: Secret<String>,
            expires_at: DateTime<Utc>,
        ) -> Result<(), AuthError> {
            *self.state.lock().unwrap() = Some((token.expose().clone(), expires_at));
            Ok(())
        }

        async fn access_token_expires_at(&self) -> Option<DateTime<Utc>> {
            self.state.lock().unwrap().as_ref().map(|(_, at)| *at)
        }

        async fn clear(&self) {
            *self.state.lock().unwrap() = None;
        }
    }

    impl FakeTokenProvider {
        /// Seed an expired access token directly (past expiry).
        fn seed_expired(&self, token: &str) {
            let past = Utc::now() - ChronoDuration::seconds(60);
            *self.state.lock().unwrap() = Some((token.into(), past));
        }
    }

    /// Fake engine attestation service — deterministic claims from tokens.
    #[derive(Default)]
    struct FakeAttestation {
        failures: Mutex<usize>,
    }

    fn claim_from_token(token: &str, issuer: Option<&str>, method: &str) -> IdentityClaim {
        let now = Utc::now();
        IdentityClaim {
            subject: format!("user-from-{token}"),
            issuer: issuer.unwrap_or("https://idp.example.com").to_string(),
            authority: Some("rigorix-user".into()),
            source: IdentitySource::IdpToken,
            auth_method: Some(method.to_string()),
            issued_at: now,
            expires_at: Some(now + ChronoDuration::seconds(900)),
            token_ref: Some("redacted-locator".into()),
        }
    }

    #[async_trait]
    impl IdentityAttestationService for FakeAttestation {
        async fn attest(&self, input: AttestInput) -> Result<IdentityClaim, IdentityError> {
            if *self.failures.lock().unwrap() > 0 {
                *self.failures.lock().unwrap() -= 1;
                return Err(IdentityError::Internal("forced failure".into()));
            }
            match input.token {
                Some(token) => Ok(claim_from_token(
                    &token,
                    input.issuer.as_deref(),
                    input.auth_method.as_deref().unwrap_or("device_code"),
                )),
                None => Err(IdentityError::MissingClaim(
                    "presented identity: token or principal".into(),
                )),
            }
        }

        fn extract_claims(&self, token: &str) -> Result<IdentityClaim, IdentityError> {
            Ok(claim_from_token(token, None, "device_code"))
        }

        async fn verify(
            &self,
            _claim: &IdentityClaim,
            _token: &str,
        ) -> Result<VerificationOutcome, IdentityError> {
            Ok(VerificationOutcome::Verified)
        }
    }

    // ---------------------------------------------------------------------
    // Test harness
    // ---------------------------------------------------------------------

    struct Harness {
        idp: Arc<FakeIdp>,
        keychain: Arc<FakeKeychain>,
        tokens: Arc<FakeTokenProvider>,
        attestation: Arc<FakeAttestation>,
        service: AuthServiceImpl,
    }

    fn harness() -> Harness {
        harness_with(FakeIdp::new())
    }

    fn harness_with(idp: FakeIdp) -> Harness {
        let config = IdpConfig::new(
            "https://idp.example.com/realms/rigorix".into(),
            "rigorix-cli".into(),
            None,
            None,
        )
        .unwrap();
        let idp = Arc::new(idp);
        let keychain = Arc::new(FakeKeychain::default());
        let tokens = Arc::new(FakeTokenProvider::default());
        let attestation = Arc::new(FakeAttestation::default());
        let service = AuthServiceImpl::new(
            config,
            idp.clone(),
            keychain.clone(),
            tokens.clone(),
            attestation.clone(),
        );
        Harness {
            idp,
            keychain,
            tokens,
            attestation,
            service,
        }
    }

    // ---------------------------------------------------------------------
    // AC#1 — device flow: verification info returned; polling succeeds
    // ---------------------------------------------------------------------

    #[tokio::test]
    async fn login_returns_verification_info_and_starts_pending_flow() {
        let h = harness();
        let out = h.service.login(&LoginInput::default()).await.unwrap();
        assert_eq!(out.status, DeviceFlowStatus::Pending);
        assert_eq!(out.verification_uri, "https://idp.example.com/device");
        assert_eq!(out.user_code, "ABCD-EFGH");
        assert_eq!(out.expires_in, 600);
    }

    #[tokio::test]
    async fn poll_echoes_pending_then_succeeds_and_persists_custody() {
        let h = harness();
        h.service.login(&LoginInput::default()).await.unwrap();

        let pending = h.service.poll(&PollInput::default()).await.unwrap();
        assert_eq!(pending.status, DeviceFlowStatus::Pending);
        assert_eq!(pending.retry_after_secs, Some(5));

        let authorized = h.service.poll(&PollInput::default()).await.unwrap();
        assert_eq!(authorized.status, DeviceFlowStatus::Authorized);
        assert!(authorized.claim_summary.is_some());
        assert_eq!(
            authorized
                .claim_summary
                .as_ref()
                .unwrap()
                .authority
                .as_deref(),
            Some("rigorix-user")
        );

        // Custody: refresh token in the keychain, access token in memory.
        let stored = h
            .keychain
            .get_refresh_token(RIGORIX_KEYCHAIN_SERVICE, REFRESH_TOKEN_ACCOUNT)
            .await
            .unwrap();
        assert_eq!(stored.unwrap().expose(), "refresh-token-1");
        let token = h.tokens.current_access_token().await.unwrap();
        assert_eq!(token.expose(), "access-token-1");
        assert!(h.tokens.access_token_expires_at().await.unwrap() > Utc::now());
    }

    #[tokio::test]
    async fn token_response_without_refresh_token_fails_custody() {
        let mut idp = FakeIdp::new();
        idp.response.refresh_token = None; // missing offline_access scope
        let h = harness_with(idp);
        h.service.login(&LoginInput::default()).await.unwrap();
        let _pending = h.service.poll(&PollInput::default()).await.unwrap();
        let err = h.service.poll(&PollInput::default()).await.unwrap_err();
        assert!(matches!(err, AuthError::InvalidTokenResponse(_)));
    }

    // ---------------------------------------------------------------------
    // AC#2 — login denied/expired → typed terminal outcomes
    // ---------------------------------------------------------------------

    #[tokio::test]
    async fn poll_denied_is_terminal_with_typed_reason() {
        let idp = FakeIdp::new().with_poll_steps(vec![PollStep::Denied]);
        let h = harness_with(idp);
        h.service.login(&LoginInput::default()).await.unwrap();

        let denied = h.service.poll(&PollInput::default()).await.unwrap();
        assert_eq!(denied.status, DeviceFlowStatus::Denied);
        assert_eq!(denied.reason.as_deref(), Some("user declined the request"));

        // Terminal flows echo their outcome idempotently.
        let again = h.service.poll(&PollInput::default()).await.unwrap();
        assert_eq!(again.status, DeviceFlowStatus::Denied);
    }

    #[tokio::test]
    async fn poll_expired_is_terminal_with_typed_reason() {
        let idp = FakeIdp::new().with_poll_steps(vec![PollStep::Expired]);
        let h = harness_with(idp);
        h.service.login(&LoginInput::default()).await.unwrap();

        let expired = h.service.poll(&PollInput::default()).await.unwrap();
        assert_eq!(expired.status, DeviceFlowStatus::Expired);

        let again = h.service.poll(&PollInput::default()).await.unwrap();
        assert_eq!(again.status, DeviceFlowStatus::Expired);
    }

    #[tokio::test]
    async fn poll_transport_fault_keeps_flow_pending() {
        let idp = FakeIdp::new().with_poll_steps(vec![PollStep::TransportFault]);
        let h = harness_with(idp);
        h.service.login(&LoginInput::default()).await.unwrap();

        let out = h.service.poll(&PollInput::default()).await.unwrap();
        assert_eq!(out.status, DeviceFlowStatus::Pending);
        assert!(out.reason.as_deref().unwrap().contains("transport"));
    }

    #[tokio::test]
    async fn poll_without_active_flow_is_not_authenticated() {
        let h = harness();
        let err = h.service.poll(&PollInput::default()).await.unwrap_err();
        assert!(matches!(err, AuthError::NotAuthenticated));
    }

    // ---------------------------------------------------------------------
    // AC#3 — status transitions authenticated/expired/unauthenticated
    // ---------------------------------------------------------------------

    #[tokio::test]
    async fn status_unauthenticated_when_no_identity_material() {
        let h = harness();
        let out = h.service.status(&StatusInput::default()).await.unwrap();
        assert_eq!(out.status, TokenStatus::Unauthenticated);
        assert!(out.claim_summary.is_none());
        assert_eq!(out.source, IdentitySource::Unverified);
    }

    #[tokio::test]
    async fn status_authenticated_after_login_with_claim_summary() {
        let h = harness();
        h.service.login(&LoginInput::default()).await.unwrap();
        h.service.poll(&PollInput::default()).await.unwrap();
        h.service.poll(&PollInput::default()).await.unwrap();

        let out = h.service.status(&StatusInput::default()).await.unwrap();
        assert_eq!(out.status, TokenStatus::Authenticated);
        let summary = out.claim_summary.unwrap();
        assert!(summary.subject.starts_with("user-from-access-token-1"));
        assert_eq!(out.source, IdentitySource::IdpToken);
    }

    #[tokio::test]
    async fn status_expired_when_access_token_past_ttl() {
        let h = harness();
        h.tokens.seed_expired("expired-access-token");

        let out = h.service.status(&StatusInput::default()).await.unwrap();
        assert_eq!(out.status, TokenStatus::Expired);
        assert!(out.claim_summary.is_none());
    }

    #[tokio::test]
    async fn status_degrades_to_unverified_when_attestation_fails() {
        let h = harness();
        h.service.login(&LoginInput::default()).await.unwrap();
        h.service.poll(&PollInput::default()).await.unwrap();
        h.service.poll(&PollInput::default()).await.unwrap();
        // Force the next attestation to fail → explicit degrade, no error.
        *h.attestation.failures.lock().unwrap() = 1;

        let out = h.service.status(&StatusInput::default()).await.unwrap();
        assert_eq!(out.status, TokenStatus::Authenticated);
        assert!(out.claim_summary.is_none());
        assert_eq!(out.source, IdentitySource::Unverified);
    }

    // ---------------------------------------------------------------------
    // AC#4 — expired access token silently refreshed from the keychain
    // ---------------------------------------------------------------------

    #[tokio::test]
    async fn refresh_silently_exchanges_keychain_refresh_token() {
        let h = harness();
        h.keychain
            .store_refresh_token(
                RIGORIX_KEYCHAIN_SERVICE,
                REFRESH_TOKEN_ACCOUNT,
                &Secret::new("refresh-token-1".into()),
            )
            .await
            .unwrap();
        h.tokens.seed_expired("stale-access-token");

        let out = h.service.refresh(&RefreshInput::default()).await.unwrap();
        assert_eq!(out.status, TokenStatus::Authenticated);
        assert_eq!(out.expires_in_secs, Some(900));
        let token = h.tokens.current_access_token().await.unwrap();
        assert_eq!(token.expose(), "access-token-1");
        assert_eq!(*h.idp.refresh_calls.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn refresh_without_keychain_credential_is_not_authenticated() {
        let h = harness();
        let err = h
            .service
            .refresh(&RefreshInput::default())
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::NotAuthenticated));
    }

    #[tokio::test]
    async fn attest_refreshes_expired_token_then_produces_claim() {
        let h = harness();
        h.keychain
            .store_refresh_token(
                RIGORIX_KEYCHAIN_SERVICE,
                REFRESH_TOKEN_ACCOUNT,
                &Secret::new("refresh-token-1".into()),
            )
            .await
            .unwrap();
        h.tokens.seed_expired("stale-access-token");

        let claim = h.service.attest().await.unwrap();
        assert!(claim.subject.starts_with("user-from-access-token-1"));
        assert_eq!(claim.source, IdentitySource::IdpToken);
        assert_eq!(
            claim.auth_method.as_deref(),
            Some("device_code"),
            "auth method records the device flow (engine claim contract)"
        );
    }

    #[tokio::test]
    async fn attest_without_token_is_not_authenticated() {
        let h = harness();
        let err = h.service.attest().await.unwrap_err();
        assert!(matches!(err, AuthError::NotAuthenticated));
    }

    #[tokio::test]
    async fn attest_refresh_transport_fault_propagates() {
        let idp = FakeIdp::new().with_refresh_transport_fault();
        let h = harness_with(idp);
        h.keychain
            .store_refresh_token(
                RIGORIX_KEYCHAIN_SERVICE,
                REFRESH_TOKEN_ACCOUNT,
                &Secret::new("refresh-token-1".into()),
            )
            .await
            .unwrap();
        h.tokens.seed_expired("stale-access-token");

        let err = h.service.attest().await.unwrap_err();
        assert!(matches!(err, AuthError::Transport(_)));
    }

    // ---------------------------------------------------------------------
    // AC#5 — logout clears keychain + memory
    // ---------------------------------------------------------------------

    #[tokio::test]
    async fn logout_clears_keychain_and_memory_and_revokes_at_idp() {
        let h = harness();
        h.service.login(&LoginInput::default()).await.unwrap();
        h.service.poll(&PollInput::default()).await.unwrap();
        h.service.poll(&PollInput::default()).await.unwrap();

        let out = h.service.logout(&LogoutInput::default()).await.unwrap();
        assert_eq!(out.status, LOGGED_OUT_STATUS);

        assert!(
            h.keychain
                .get_refresh_token(RIGORIX_KEYCHAIN_SERVICE, REFRESH_TOKEN_ACCOUNT)
                .await
                .unwrap()
                .is_none()
        );
        assert!(h.tokens.current_access_token().await.is_none());
        assert!(*h.idp.revoked.lock().unwrap());
    }

    #[tokio::test]
    async fn logout_without_identity_is_still_ok() {
        let h = harness();
        let out = h.service.logout(&LogoutInput::default()).await.unwrap();
        assert_eq!(out.status, LOGGED_OUT_STATUS);
    }

    // ---------------------------------------------------------------------
    // login() input handling
    // ---------------------------------------------------------------------

    #[tokio::test]
    async fn login_client_id_override_is_forwarded() {
        let h = harness();
        let out = h
            .service
            .login(&LoginInput {
                client_id: Some("alternate-cli".into()),
                issuer: None,
            })
            .await
            .unwrap();
        assert_eq!(out.status, DeviceFlowStatus::Pending);
    }

    #[tokio::test]
    async fn login_issuer_override_mismatch_is_configuration_error() {
        let h = harness();
        let err = h
            .service
            .login(&LoginInput {
                client_id: None,
                issuer: Some("https://other-idp.example.com".into()),
            })
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::Configuration(_)));
    }

    #[tokio::test]
    async fn login_propagates_idp_transport_fault() {
        let idp = FakeIdp::new();
        *idp.auth_transport_fault.lock().unwrap() = true;
        let h = harness_with(idp);
        let err = h.service.login(&LoginInput::default()).await.unwrap_err();
        assert!(matches!(err, AuthError::Transport(_)));
    }

    // ---------------------------------------------------------------------
    // Device flow state helpers
    // ---------------------------------------------------------------------

    #[test]
    fn secret_never_renders_raw_material() {
        let secret = Secret::new("crown-jewel");
        assert!(!format!("{secret:?}").contains("crown-jewel"));
        assert!(format!("{secret:?}").contains("REDACTED"));
    }

    #[test]
    fn uuid_session_ids_are_unique() {
        let a = uuid::Uuid::new_v4().to_string();
        let b = uuid::Uuid::new_v4().to_string();
        assert_ne!(a, b);
    }
}
