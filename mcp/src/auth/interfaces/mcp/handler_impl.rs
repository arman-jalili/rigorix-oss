//! AuthToolHandlerImpl — MCP tool handlers for rigorix_auth_*.
//!
//! @canonical .pi/architecture/modules/auth.md#authhandler--sse-auth-interfaces
//! Implements: ISSUE-AUTH-5 — AuthHandler (Interfaces)
//! Issue: #825
//! ADR-008: tool handlers bootstrap/read/clear identity
//!
//! Concrete implementation of the frozen [`AuthToolHandler`] contract over
//! the application [`AuthService`]. Every output is **redacted** per the
//! auth.md API Endpoints table — never a raw token.
//!
//! # Login completion UX
//!
//! The OIDC device flow is interactive: `rigorix_auth_login` initiates and
//! returns the `verification_uri` + `user_code` immediately (never blocks).
//! `rigorix_auth_status` advances any in-flight device flow by one poll
//! before reporting, so repeated status calls surface authorization
//! completion — no extra tool surface needed for the single-session client.

use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::auth::application::dto::{
    LoginInput, LoginOutput, LogoutInput, LogoutOutput, PollInput, StatusInput, StatusOutput,
};
use crate::auth::application::service::AuthService;
use crate::auth::domain::error::AuthError;
use crate::auth::interfaces::mcp::AuthToolHandler;

/// Concrete [`AuthToolHandler`] composing the frozen tool surface with the
/// application service.
pub struct AuthToolHandlerImpl {
    /// Identity lifecycle service.
    service: Arc<dyn AuthService>,
}

impl AuthToolHandlerImpl {
    /// Create a handler over an [`AuthService`].
    pub fn new(service: Arc<dyn AuthService>) -> Self {
        Self { service }
    }
}

/// Extract an optional string field from tool params (absent/null → None).
fn optional_string(params: &Value, key: &str) -> Option<String> {
    params.get(key).and_then(Value::as_str).map(String::from)
}

#[async_trait]
impl AuthToolHandler for AuthToolHandlerImpl {
    async fn handle_auth_login(&self, params: Value) -> Result<Value, AuthError> {
        // {} or { client_id, issuer } overrides.
        let input = LoginInput {
            client_id: optional_string(&params, "client_id"),
            issuer: optional_string(&params, "issuer"),
        };
        let output: LoginOutput = self.service.login(&input).await?;
        Ok(json!({
            "status": output.status,
            "verification_uri": output.verification_uri,
            "user_code": output.user_code,
            "expires_in": output.expires_in,
        }))
    }

    async fn handle_auth_status(&self, params: Value) -> Result<Value, AuthError> {
        let _ = params;
        // Advance any in-flight device flow by one poll before reporting, so
        // rigorix_auth_status doubles as the completion driver for login.
        let _ = self.service.poll(&PollInput::default()).await;
        let output: StatusOutput = self.service.status(&StatusInput::default()).await?;
        Ok(json!({
            "status": output.status,
            "claim_summary": output.claim_summary,
            "source": output.source,
        }))
    }

    async fn current_engine_identity(&self) -> Option<rigorix_engine::identity::IdentityRef> {
        // StatusOutput.source is already the engine IdentitySource type.
        let out: StatusOutput = self.service.status(&StatusInput::default()).await.ok()?;
        // #1277 — the claim is handed to the engine even when it could not be
        // verified. Dropping it here turned "this process cannot check the
        // claim" into "there is no claim", and the L1 gate then printed
        // `unauthenticated` at a caller who had just logged in. The engine's
        // gate already distinguishes the two: `IdentitySource::Unverified`
        // refuses flagged steps with status `unverified`.
        let claim = out.claim_summary?;
        Some(rigorix_engine::identity::IdentityRef {
            subject: claim.subject,
            issuer: claim.issuer,
            source: out.source,
            authority: claim.authority,
            expires_at: claim.expires_at,
        })
    }

    async fn handle_auth_logout(&self, params: Value) -> Result<Value, AuthError> {
        let _ = params;
        let output: LogoutOutput = self.service.logout(&LogoutInput::default()).await?;
        Ok(json!({ "status": output.status }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use rigorix_engine::identity::IdentityClaim;
    use rigorix_engine::identity::IdentitySource;

    use crate::auth::application::dto::{
        LoginInput, LoginOutput, LogoutInput, LogoutOutput, StatusInput, StatusOutput,
    };
    use crate::auth::application::dto::{PollInput, PollOutput, RefreshInput, RefreshOutput};
    use crate::auth::domain::value::ClaimSummary;
    use crate::auth::domain::{DeviceFlowStatus, TokenStatus};

    /// Minimal `AuthService` that returns a scripted status.
    struct ScriptedAuth {
        output: StatusOutput,
    }

    #[async_trait]
    impl AuthService for ScriptedAuth {
        async fn login(&self, _input: &LoginInput) -> Result<LoginOutput, AuthError> {
            Err(AuthError::NotAuthenticated)
        }

        async fn poll(&self, _input: &PollInput) -> Result<PollOutput, AuthError> {
            Ok(PollOutput {
                status: DeviceFlowStatus::Pending,
                retry_after_secs: None,
                reason: None,
                claim_summary: None,
            })
        }

        async fn status(&self, _input: &StatusInput) -> Result<StatusOutput, AuthError> {
            Ok(self.output.clone())
        }

        async fn refresh(&self, _input: &RefreshInput) -> Result<RefreshOutput, AuthError> {
            Err(AuthError::NotAuthenticated)
        }

        async fn logout(&self, _input: &LogoutInput) -> Result<LogoutOutput, AuthError> {
            Ok(LogoutOutput {
                status: "logged_out".into(),
            })
        }

        async fn attest(&self) -> Result<IdentityClaim, AuthError> {
            Err(AuthError::NotAuthenticated)
        }
    }

    fn summary() -> ClaimSummary {
        ClaimSummary {
            subject: "user@corp.demo".into(),
            issuer: "https://idp.example.com".into(),
            authority: None,
            expires_at: None,
        }
    }

    /// #1277 — a session that logged in while the IdP was unreachable yields a
    /// claim tagged `unverified`; it must not be dropped on the floor.
    #[tokio::test]
    async fn unverified_claim_is_handed_to_the_engine() {
        let handler = AuthToolHandlerImpl::new(Arc::new(ScriptedAuth {
            output: StatusOutput {
                status: TokenStatus::Unverified,
                claim_summary: Some(summary()),
                source: IdentitySource::Unverified,
            },
        }));

        let identity = handler.current_engine_identity().await;
        let identity = identity.expect("an unverified claim is still a claim");
        assert_eq!(identity.source, IdentitySource::Unverified);
        assert_eq!(identity.subject, "user@corp.demo");
    }

    /// The honoured case is unchanged, and a genuinely absent claim is still `None`.
    #[tokio::test]
    async fn attested_claim_is_handed_over_and_absent_claim_is_none() {
        let attested = AuthToolHandlerImpl::new(Arc::new(ScriptedAuth {
            output: StatusOutput {
                status: TokenStatus::Authenticated,
                claim_summary: Some(summary()),
                source: IdentitySource::IdpToken,
            },
        }));
        let identity = attested.current_engine_identity().await.unwrap();
        assert_eq!(identity.source, IdentitySource::IdpToken);

        let absent = AuthToolHandlerImpl::new(Arc::new(ScriptedAuth {
            output: StatusOutput {
                status: TokenStatus::Unauthenticated,
                claim_summary: None,
                source: IdentitySource::Unverified,
            },
        }));
        assert!(absent.current_engine_identity().await.is_none());
    }
}
