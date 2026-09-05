//! Data Transfer Objects for the Auth module.
//!
//! @canonical .pi/architecture/modules/auth.md#dto
//! Implements: Contract Freeze — all input/output DTO schemas
//!
//! DTOs define the input/output contracts for all service operations. They
//! carry documentation and validation metadata but no behavior.
//!
//! # Contract (Frozen)
//!
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for MCP transport)
//! - Field names and types are frozen — implementation issues depend on them
//! - Outputs are always redacted — never a raw token
//! - DTO field names/types mirror `.pi/architecture/modules/auth.md`
//!   (API Endpoints table)

use serde::{Deserialize, Serialize};

use crate::auth::domain::value::ClaimSummary;
use crate::auth::domain::{DeviceFlowStatus, TokenStatus};
use rigorix_engine::identity::IdentitySource;

// ---------------------------------------------------------------------------
// Login DTOs — rigorix_auth_login
// ---------------------------------------------------------------------------

/// Input for initiating the OIDC device flow.
///
/// Empty (`{}`) uses the configured IdP; `client_id`/`issuer` are optional
/// runtime overrides (bootstrap against a different provider).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginInput {
    /// Optional client_id override (default: configured `client_id`).
    pub client_id: Option<String>,

    /// Optional issuer override (default: configured `issuer`).
    pub issuer: Option<String>,
}

/// Output from initiating the OIDC device flow.
///
/// Mirrors the frozen MCP output: `{ status, verification_uri, user_code,
/// expires_in }`. The caller displays `verification_uri` + `user_code` to the
/// human; `expires_in` is the device-code lifetime in seconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginOutput {
    /// Flow phase — `pending` immediately after initiation.
    pub status: DeviceFlowStatus,

    /// URL the human opens to authorize.
    pub verification_uri: String,

    /// Human-readable code to enter at the verification URI.
    pub user_code: String,

    /// Seconds until the device code expires (RFC 8628 §3.2).
    pub expires_in: u64,
}

/// Input for advancing an active device flow (`AuthService::poll`).
///
/// The active flow is implicit in the service's session state (single-session
/// client-side module).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollInput {}

/// Output from one poll of the token endpoint (RFC 8628 §3.3–3.5).
///
/// A non-terminal `status` (`pending`) means "poll again after
/// `retry_after_secs`". Terminal success (`authorized`) means the refresh
/// token was persisted to the keychain and the access token cached in memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollOutput {
    /// Flow phase after this poll.
    pub status: DeviceFlowStatus,

    /// Seconds to wait before the next poll (only meaningful while pending).
    pub retry_after_secs: Option<u64>,

    /// Human-readable context for `denied`/`expired` terminal states.
    pub reason: Option<String>,

    /// Redacted claim summary — present once the flow is `authorized`.
    pub claim_summary: Option<ClaimSummary>,
}

// ---------------------------------------------------------------------------
// Status DTOs — rigorix_auth_status
// ---------------------------------------------------------------------------

/// Input for a status query.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusInput {}

/// Output from a status query.
///
/// Mirrors the frozen MCP output: `{ status, claim_summary, source }`.
/// All fields are redacted — no raw token anywhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusOutput {
    /// Token lifecycle status.
    pub status: TokenStatus,

    /// Redacted claim summary when authenticated (`None` otherwise).
    pub claim_summary: Option<ClaimSummary>,

    /// Identity source marker (`idp_token`, `local_principal`, `unverified`).
    pub source: IdentitySource,
}

// ---------------------------------------------------------------------------
// Refresh DTOs — silent background refresh
// ---------------------------------------------------------------------------

/// Input for a silent refresh-token exchange.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshInput {}

/// Output from a silent refresh.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshOutput {
    /// Status after the refresh attempt.
    pub status: TokenStatus,

    /// New access-token TTL in seconds (present when refreshed).
    pub expires_in_secs: Option<u64>,
}

// ---------------------------------------------------------------------------
// Logout DTOs — rigorix_auth_logout
// ---------------------------------------------------------------------------

/// Input for logging out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogoutInput {}

/// Output from logging out.
///
/// Mirrors the frozen MCP output: `{ status: "logged_out" }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogoutOutput {
    /// Frozen literal: always `"logged_out"` on success.
    pub status: String,
}

/// The frozen `LogoutOutput.status` literal.
pub const LOGGED_OUT_STATUS: &str = "logged_out";

impl LogoutOutput {
    /// Construct the canonical logged-out output.
    pub fn logged_out() -> Self {
        Self {
            status: LOGGED_OUT_STATUS.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Claim conversion (shared-kernel dependency on engine identity)
// ---------------------------------------------------------------------------

/// Build a redacted [`ClaimSummary`] from an engine [`rigorix_engine::identity::IdentityClaim`].
///
/// The raw token and its `token_ref` are intentionally dropped — a claim
/// summary must never render them.
impl From<&rigorix_engine::identity::IdentityClaim> for ClaimSummary {
    fn from(claim: &rigorix_engine::identity::IdentityClaim) -> Self {
        Self {
            subject: claim.subject.clone(),
            issuer: claim.issuer.clone(),
            authority: claim.authority.clone(),
            expires_at: claim.expires_at,
        }
    }
}
