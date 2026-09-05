//! MCP protocol handler contracts for Auth.
//!
//! @canonical .pi/architecture/modules/auth.md#mcp-handlers
//! Implements: Contract Freeze — rigorix_auth_login, rigorix_auth_status,
//! rigorix_auth_logout tool handler contracts
//! ADR-008: tool handlers bootstrap/read/clear identity
//!
//! These contracts define the MCP tool schema registrations for the auth
//! tools. Every tool returns **redacted** output — never the raw token.
//!
//! # API Endpoints
//!
//! | Tool Name | Handler | Input | Output | Auth |
//! |-----------|---------|-------|--------|------|
//! | `rigorix_auth_login` | handle_auth_login | `{}` (or `{ client_id, issuer }`) | `{ status, verification_uri, user_code, expires_in }` | None (bootstraps identity) |
//! | `rigorix_auth_status` | handle_auth_status | `{}` | `{ status, claim_summary: { subject, issuer, authority, expires_at }, source }` | None (read-only, redacted) |
//! | `rigorix_auth_logout` | handle_auth_logout | `{}` | `{ status: "logged_out" }` | None (self-service) |
//!
//! # Contract (Frozen)
//!
//! - Tool names are frozen (`rigorix_auth_login`, `rigorix_auth_status`,
//!   `rigorix_auth_logout`)
//! - Input schemas are documented but not enforced by types here
//! - Output is JSON matching the table above — all redacted
//! - Error format follows `AuthError` (typed, never raw token material)
//! - Tool names register with the MCP ToolRegistry like all other tools
//!   (mcp-server module, ADR-004)

use async_trait::async_trait;
use serde_json::Value;

use crate::auth::domain::error::AuthError;

// ---------------------------------------------------------------------------
// Tool Name Constants
// ---------------------------------------------------------------------------

/// Tool name for initiating the OIDC device flow.
pub const RIGORIX_AUTH_LOGIN: &str = "rigorix_auth_login";

/// Tool name for reporting identity status.
pub const RIGORIX_AUTH_STATUS: &str = "rigorix_auth_status";

/// Tool name for clearing identity material.
pub const RIGORIX_AUTH_LOGOUT: &str = "rigorix_auth_logout";

/// All auth tool names (registration + `tools/list`).
pub const AUTH_TOOL_NAMES: &[&str] =
    &[RIGORIX_AUTH_LOGIN, RIGORIX_AUTH_STATUS, RIGORIX_AUTH_LOGOUT];

// ---------------------------------------------------------------------------
// Tool Schema Definitions (JSON Schema — frozen)
// ---------------------------------------------------------------------------

/// JSON Schema for the `rigorix_auth_login` tool input.
pub const RIGORIX_AUTH_LOGIN_INPUT_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "client_id": {
            "type": "string",
            "description": "Optional client_id override (default: configured IdP client_id)"
        },
        "issuer": {
            "type": "string",
            "format": "uri",
            "description": "Optional issuer override (default: configured IdP issuer)"
        }
    },
    "description": "Initiate the OIDC device flow. Returns a verification_uri and user_code to display to the human."
}"#;

/// JSON Schema for the `rigorix_auth_status` tool input.
pub const RIGORIX_AUTH_STATUS_INPUT_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {},
    "description": "Report identity status (authenticated/expired/unauthenticated) with a redacted claim summary."
}"#;

/// JSON Schema for the `rigorix_auth_logout` tool input.
pub const RIGORIX_AUTH_LOGOUT_INPUT_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {},
    "description": "Clear identity material: revoke the keychain refresh token and clear the in-memory access token."
}"#;

// ---------------------------------------------------------------------------
// Tool Descriptors (MCP tools/list entries)
// ---------------------------------------------------------------------------

/// Descriptor for the `rigorix_auth_login` tool.
pub fn rigorix_auth_login_tool_descriptor() -> Value {
    serde_json::json!({
        "name": RIGORIX_AUTH_LOGIN,
        "description": "Initiate the OIDC device flow (identity attestation, ADR-008). Returns a verification_uri and user_code to display to the human; completion is silent once the human authorizes.",
        "inputSchema": serde_json::from_str::<Value>(RIGORIX_AUTH_LOGIN_INPUT_SCHEMA).unwrap_or_default()
    })
}

/// Descriptor for the `rigorix_auth_status` tool.
pub fn rigorix_auth_status_tool_descriptor() -> Value {
    serde_json::json!({
        "name": RIGORIX_AUTH_STATUS,
        "description": "Report identity status (authenticated/expired/unauthenticated) and a redacted claim summary. Read-only.",
        "inputSchema": serde_json::from_str::<Value>(RIGORIX_AUTH_STATUS_INPUT_SCHEMA).unwrap_or_default()
    })
}

/// Descriptor for the `rigorix_auth_logout` tool.
pub fn rigorix_auth_logout_tool_descriptor() -> Value {
    serde_json::json!({
        "name": RIGORIX_AUTH_LOGOUT,
        "description": "Clear identity material: revoke the keychain refresh token at the IdP and clear the in-memory access token.",
        "inputSchema": serde_json::from_str::<Value>(RIGORIX_AUTH_LOGOUT_INPUT_SCHEMA).unwrap_or_default()
    })
}

/// All auth tool descriptors (registration + `tools/list`).
pub fn auth_tool_descriptors() -> Vec<Value> {
    vec![
        rigorix_auth_login_tool_descriptor(),
        rigorix_auth_status_tool_descriptor(),
        rigorix_auth_logout_tool_descriptor(),
    ]
}

// ---------------------------------------------------------------------------
// AuthToolHandler — handler contract
// ---------------------------------------------------------------------------

/// Handler contract for the three `rigorix_auth_*` tools.
///
/// Implementations compose the application [`AuthService`](crate::auth::AuthService)
/// (and, for login completion, the poll loop) and return **redacted** JSON
/// payloads matching the frozen API Endpoints table. Formatting into the MCP
/// tool-response envelope happens in the transport/router layer.
#[async_trait]
pub trait AuthToolHandler: Send + Sync {
    /// Handle `rigorix_auth_login` — initiate the device flow.
    ///
    /// Returns `{ status, verification_uri, user_code, expires_in }`.
    ///
    /// # Errors
    /// - `AuthError::Configuration` / `Discovery` / `DeviceAuthorizationRejected`
    async fn handle_auth_login(&self, params: Value) -> Result<Value, AuthError>;

    /// Handle `rigorix_auth_status` — report identity status.
    ///
    /// Returns `{ status, claim_summary, source }` (all redacted).
    async fn handle_auth_status(&self, params: Value) -> Result<Value, AuthError>;

    /// Handle `rigorix_auth_logout` — clear identity material.
    ///
    /// Returns `{ status: "logged_out" }`.
    async fn handle_auth_logout(&self, params: Value) -> Result<Value, AuthError>;
}

// ---------------------------------------------------------------------------
// Handler implementation (ISSUE-AUTH-5)
// ---------------------------------------------------------------------------

pub mod handler_impl;

pub use handler_impl::AuthToolHandlerImpl;
