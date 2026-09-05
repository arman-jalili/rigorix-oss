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

    async fn handle_auth_logout(&self, params: Value) -> Result<Value, AuthError> {
        let _ = params;
        let output: LogoutOutput = self.service.logout(&LogoutInput::default()).await?;
        Ok(json!({ "status": output.status }))
    }
}
