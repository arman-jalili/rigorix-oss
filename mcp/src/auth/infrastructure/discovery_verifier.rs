//! DiscoveryBackedVerifier — attestation-time IdP discovery with a bounded retry.
//!
//! Implements: #1277 — an IdP that is unreachable at startup must not disable
//!   attestation for the life of the process.
//!
//! The composition root used to resolve the token verifier **once**, before the
//! auth service existed, and never again: a single failed discovery request in
//! the first second of the process installed `NullVerifier` forever. This
//! verifier keeps the offline default's promise (an unreachable IdP yields
//! `Unverified`, never an error — ADR-012 option c) while making the decision
//! *repeatable*: every attestation that finds no resolver may re-attempt
//! discovery, bounded by a per-process cooldown so a flapping IdP is not
//! hammered once per tool call.
//!
//! # Contract
//! - One discovery attempt in flight at a time; the first successful
//!   resolution is cached for the life of the process
//! - A failed attempt is retried no sooner than `retry_after` (default
//!   [`DEFAULT_RETRY_AFTER`]) — the bound is per process, not per caller
//! - Discovery that never succeeds keeps attestation degraded: `Unverified`
//!   with a reason naming the cause and the recovery, **never** an error and
//!   **never** a silent pass
//! - A resolved `JwksVerifier` delegates verbatim: an unknown `kid` or a
//!   tampered signature still degrades to `Unverified`

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use rigorix_engine::identity::application::service::VerificationOutcome;
use rigorix_engine::identity::domain::{IdentityClaim, IdentityError};
use rigorix_engine::identity::infrastructure::{JwksVerifier, TokenVerifier};
use tokio::sync::Mutex;

use crate::auth::infrastructure::idp_client::IdpClient;

/// How long a failed discovery is remembered before the next attempt.
///
/// Long enough that a burst of tool calls after one outage costs one request,
/// short enough that a process which outlived the outage recovers on its own.
pub const DEFAULT_RETRY_AFTER: Duration = Duration::from_secs(30);

/// Verifier that resolves the IdP JWKS lazily, at attestation time.
pub struct DiscoveryBackedVerifier {
    /// OIDC client used for discovery (it caches metadata on success).
    idp: Arc<dyn IdpClient>,
    /// Cooldown between discovery attempts after a failure.
    retry_after: Duration,
    /// Resolver state — `verifier` is `None` until discovery succeeds.
    state: Mutex<DiscoveryState>,
    /// Mirror of `DiscoveryState::last_error` that is readable without an
    /// `await`, so `rigorix_auth_status` can report *why* `unverified` without
    /// taking the resolver lock (#1282). Written only in `resolve`.
    last_failure: std::sync::Mutex<Option<String>>,
}

/// Cached outcome of the most recent discovery attempt.
#[derive(Default)]
struct DiscoveryState {
    /// Resolved verifier; `None` while the IdP has not answered yet.
    verifier: Option<Arc<dyn TokenVerifier>>,
    /// When the last attempt was made (`None` = never attempted).
    last_attempt: Option<Instant>,
    /// Why the last attempt failed (rendered into the `Unverified` reason).
    last_error: Option<String>,
}

impl DiscoveryBackedVerifier {
    /// Create a verifier over an OIDC client, using the default cooldown.
    pub fn new(idp: Arc<dyn IdpClient>) -> Self {
        Self::with_retry_after(idp, DEFAULT_RETRY_AFTER)
    }

    /// Create a verifier with an explicit retry cooldown (tests, tuning).
    pub fn with_retry_after(idp: Arc<dyn IdpClient>, retry_after: Duration) -> Self {
        Self {
            idp,
            retry_after,
            state: Mutex::new(DiscoveryState::default()),
            last_failure: std::sync::Mutex::new(None),
        }
    }

    /// True when attestation holds a resolved verifier (observability/tests).
    pub async fn is_resolved(&self) -> bool {
        self.state.lock().await.verifier.is_some()
    }

    /// Record (or clear) the reason a verification could not be performed.
    fn remember(&self, reason: Option<String>) {
        if let Ok(mut slot) = self.last_failure.lock() {
            *slot = reason;
        }
    }

    /// Attempt discovery if the cooldown allows it; returns the resolved
    /// verifier and the reason it is still absent.
    ///
    /// The lock is held across `discover()` on purpose: it serializes attempts
    /// so a cold process makes **one** discovery request even when several tool
    /// calls arrive at once. `discover()` is bounded by the client's timeout.
    async fn resolve(&self) -> (Option<Arc<dyn TokenVerifier>>, Option<String>) {
        let mut state = self.state.lock().await;

        let due = state
            .last_attempt
            .is_none_or(|at| at.elapsed() >= self.retry_after);

        if state.verifier.is_none() && due {
            state.last_attempt = Some(Instant::now());
            match self.idp.discover().await {
                Ok(meta) => match meta.jwks_uri {
                    Some(jwks_url) => {
                        tracing::info!(
                            "auth: IdP discovery succeeded — claims verify again (jwks_uri resolved)"
                        );
                        state.verifier = Some(Arc::new(JwksVerifier::new(jwks_url)));
                        state.last_error = None;
                        self.remember(None);
                    }
                    None => {
                        tracing::warn!(
                            "auth: IdP discovery returned no jwks_uri — claims degrade to unverified"
                        );
                        state.last_error =
                            Some("the IdP discovered but advertises no jwks_uri".to_string());
                        self.remember(state.last_error.clone());
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        "auth: IdP discovery failed ({e}) — claims degrade to unverified until it answers"
                    );
                    state.last_error = Some(format!("the IdP was unreachable ({e})"));
                    self.remember(state.last_error.clone());
                }
            }
        }

        (state.verifier.clone(), state.last_error.clone())
    }
}

#[async_trait]
impl TokenVerifier for DiscoveryBackedVerifier {
    /// Verify through the discovered JWKS, re-attempting discovery when the
    /// IdP had not answered yet.
    ///
    /// An unreachable IdP is not an error: it yields `Unverified` with a reason
    /// that names the cause and the recovery, so the refusal built on it can
    /// stop telling the caller to log in again (#1277).
    async fn verify(
        &self,
        token: &str,
        claim: &IdentityClaim,
    ) -> Result<VerificationOutcome, IdentityError> {
        let (verifier, last_error) = self.resolve().await;
        let outcome = match verifier {
            // The cached verifier's own `Unverified` is a verification failure
            // as much as a discovery failure: before #1285 its reason was
            // returned to the caller and recorded nowhere, so `last_failure()`
            // — and therefore `rigorix_auth_status.reason` — was empty exactly
            // when the operator needed it.
            Some(verifier) => verifier.verify(token, claim).await?,
            None => VerificationOutcome::Unverified {
                reason: format!(
                    "{} — verification is re-attempted on the next call, so this clears once the \
                     IdP answers; if it keeps failing, fix RIGORIX_IDP_ISSUER",
                    last_error
                        .unwrap_or_else(|| "no verifier is available for this claim".to_string())
                ),
            },
        };
        // One place records what this verifier reports: whatever reason the
        // returned outcome carries (discovery or JWKS), and `None` once a
        // credential verifies again (#1285).
        self.remember(match &outcome {
            VerificationOutcome::Unverified { reason } => Some(reason.clone()),
            VerificationOutcome::Verified => None,
        });
        Ok(outcome)
    }

    /// #1282 — the operator-facing reason, readable without an `await`.
    fn last_failure(&self) -> Option<String> {
        self.last_failure.lock().ok().and_then(|slot| slot.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use chrono::Utc;

    use crate::auth::domain::{AuthError, Secret};
    use crate::auth::infrastructure::idp_client::{
        DeviceAuthorization, IdpMetadata, TokenPoll, TokenResponse,
    };

    /// IdP that fails discovery until `failures` attempts have been made.
    struct FlakyIdp {
        failures_left: AtomicUsize,
        attempts: AtomicUsize,
        jwks_uri: Option<String>,
    }

    impl FlakyIdp {
        fn new(failures: usize, jwks_uri: Option<&str>) -> Self {
            Self {
                failures_left: AtomicUsize::new(failures),
                attempts: AtomicUsize::new(0),
                jwks_uri: jwks_uri.map(str::to_string),
            }
        }

        fn attempts(&self) -> usize {
            self.attempts.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl IdpClient for FlakyIdp {
        async fn discover(&self) -> Result<IdpMetadata, AuthError> {
            self.attempts.fetch_add(1, Ordering::SeqCst);
            if self
                .failures_left
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                .is_ok()
            {
                return Err(AuthError::Discovery {
                    issuer: "https://idp.example.com".into(),
                    reason: "connection refused".into(),
                });
            }
            Ok(IdpMetadata {
                issuer: "https://idp.example.com".into(),
                device_authorization_endpoint: "https://idp.example.com/device".into(),
                token_endpoint: "https://idp.example.com/token".into(),
                revocation_endpoint: None,
                jwks_uri: self.jwks_uri.clone(),
            })
        }

        async fn device_authorization(
            &self,
            _client_id: &str,
        ) -> Result<DeviceAuthorization, AuthError> {
            Err(AuthError::NotAuthenticated)
        }

        async fn poll_token(
            &self,
            _device_code: &Secret<String>,
            _client_id: &str,
        ) -> Result<TokenPoll, AuthError> {
            Err(AuthError::NotAuthenticated)
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

    fn claim() -> IdentityClaim {
        IdentityClaim {
            subject: "user@corp.demo".into(),
            issuer: "https://idp.example.com".into(),
            authority: None,
            source: rigorix_engine::identity::IdentitySource::IdpToken,
            auth_method: Some("device_code".into()),
            issued_at: Utc::now(),
            expires_at: None,
            token_ref: None,
        }
    }

    fn unverified_reason(outcome: &VerificationOutcome) -> &str {
        match outcome {
            VerificationOutcome::Unverified { reason } => reason,
            VerificationOutcome::Verified => panic!("expected Unverified, got Verified"),
        }
    }

    /// #1277 — a process that outlived the outage recovers without a restart:
    /// the second attestation, past the cooldown, re-attempts discovery.
    #[tokio::test]
    async fn discovery_is_re_attempted_after_the_cooldown() {
        let idp = Arc::new(FlakyIdp::new(1, Some("https://idp.example.com/jwks")));
        let verifier = DiscoveryBackedVerifier::with_retry_after(idp.clone(), Duration::ZERO);

        // Cold: discovery fails → degraded, not an error.
        let cold = verifier.verify("a.b.c", &claim()).await.unwrap();
        let reason = unverified_reason(&cold);
        assert!(
            reason.contains("unreachable"),
            "the reason must name the cause: {reason}"
        );
        assert!(
            !reason.contains("restart"),
            "a restart is not required — the next attestation re-attempts discovery: {reason}"
        );
        // #1282 — the recorded failure is readable without triggering a refusal,
        // which is what `rigorix_auth_status` reports.
        let recorded = verifier.last_failure().expect("a failure was recorded");
        assert!(
            recorded.contains("unreachable") && recorded.contains("connection refused"),
            "the recorded failure must name the underlying error: {recorded}"
        );
        assert_eq!(idp.attempts(), 1);
        assert!(!verifier.is_resolved().await);

        // Warm: the IdP answers now → discovery is re-attempted and resolves.
        // The delegate then rejects the non-JWT probe offline (header parse
        // fails before any JWKS fetch), so this stays a pure unit test;
        // resolution — not the signature check — is the property under test.
        let _ = verifier.verify("a.b.c", &claim()).await;
        assert_eq!(idp.attempts(), 2);
        assert!(
            verifier.is_resolved().await,
            "a recovered IdP must resolve the verifier without a restart"
        );
        // #1282 — a recovered process reports no stale reason.
        assert!(
            verifier.last_failure().is_none(),
            "the recorded failure must clear when discovery succeeds"
        );
    }

    /// #1285 — the case that happens most often: discovery succeeded and the
    /// verifier was cached, so it is the **JWKS fetch** that fails when the IdP
    /// goes away. Before this the reason was returned to the caller and
    /// recorded nowhere, so `rigorix_auth_status.reason` was `null` exactly
    /// when the operator needed it, and the refusal blamed a discovery attempt
    /// that had not failed.
    #[tokio::test]
    async fn a_jwks_failure_after_a_successful_discovery_is_recorded() {
        // Discovery answers, and points the cached verifier at a port with no
        // listener: the credential cannot be checked, and discovery is not
        // what failed.
        let idp = Arc::new(FlakyIdp::new(
            0,
            Some("http://127.0.0.1:1/.well-known/jwks.json"),
        ));
        let verifier = DiscoveryBackedVerifier::with_retry_after(idp.clone(), Duration::ZERO);

        // A parseable RS256 header is enough to reach the JWKS fetch.
        let token = "eyJhbGciOiJSUzI1NiIsImtpZCI6Im1vY2staWRwLWtleS0xIn0.payload.signature";
        let out = verifier.verify(token, &claim()).await.unwrap();
        let outcome_reason = unverified_reason(&out);

        assert!(
            verifier.is_resolved().await,
            "discovery succeeded — the verifier is cached, not rediscovered"
        );
        assert_eq!(idp.attempts(), 1, "one discovery attempt, not one per call");

        let recorded = verifier
            .last_failure()
            .expect("the JWKS failure must be recorded, not dropped");
        assert!(
            !recorded.contains("discovery"),
            "discovery did not fail; the recorded cause must not blame it: {recorded}"
        );
        assert!(
            recorded.contains("http://127.0.0.1:1") || recorded.contains("connection refused"),
            "the recorded cause must name the failing fetch: {recorded}"
        );
        assert_eq!(
            recorded, outcome_reason,
            "the surface and the outcome must report the same cause"
        );

        // End of the chain: the refusal the operator reads (#1285) — the
        // engine quotes the cause and names no stage, because this process is
        // the only one that knows which stage failed.
        let refusal = rigorix_engine::orchestrator::domain::OrchestratorError::IdentityRequired {
            step: "payout".to_string(),
            status: "unverified".to_string(),
            reason: Some(recorded.clone()),
        }
        .to_string();
        assert!(
            refusal.contains(&recorded),
            "the refusal must quote the cause the verifier recorded: {refusal}"
        );
        assert!(
            !refusal.contains("discovery"),
            "the refusal must not blame a stage: {refusal}"
        );
        assert!(
            !refusal.contains("restart"),
            "a restart is not what recovers: {refusal}"
        );
    }

    /// The bound: inside the cooldown a degraded process makes no new request.
    #[tokio::test]
    async fn discovery_is_not_hammered_inside_the_cooldown() {
        let idp = Arc::new(FlakyIdp::new(3, Some("https://idp.example.com/jwks")));
        let verifier =
            DiscoveryBackedVerifier::with_retry_after(idp.clone(), Duration::from_secs(3600));

        for _ in 0..5 {
            verifier.verify("a.b.c", &claim()).await.unwrap();
        }
        assert_eq!(idp.attempts(), 1, "one attempt per cooldown window");
    }

    /// A discovered IdP without a `jwks_uri` stays degraded and says why.
    #[tokio::test]
    async fn missing_jwks_uri_degrades_with_a_named_cause() {
        let idp = Arc::new(FlakyIdp::new(0, None));
        let verifier = DiscoveryBackedVerifier::with_retry_after(idp.clone(), Duration::ZERO);

        let out = verifier.verify("a.b.c", &claim()).await.unwrap();
        let reason = unverified_reason(&out);
        assert!(
            reason.contains("no jwks_uri"),
            "unexpected reason: {reason}"
        );
        assert!(!verifier.is_resolved().await);
        assert!(
            verifier
                .last_failure()
                .is_some_and(|r| r.contains("no jwks_uri")),
            "the recorded failure must name the missing jwks_uri"
        );
    }
}
