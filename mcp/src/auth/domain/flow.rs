//! DeviceFlowState — OIDC device-authorization flow state (RFC 8628).
//!
//! @canonical .pi/architecture/modules/auth.md#deviceflowstate
//! Implements: Contract Freeze — DeviceFlowState value object, DeviceFlowStatus
//!
//! Holds the state of an in-flight device authorization grant: what the human
//! must be shown (`verification_uri`, `user_code`), when the device code
//! expires, and how fast the token endpoint may be polled.
//!
//! # Contract (Frozen)
//!
//! - Immutable after construction — progress is a new state, never a mutation
//! - `device_code` is stored as `Secret<String>` — it is exchanged for tokens
//!   and must never appear in logs or serialized output
//! - `status` transitions: `Pending` → `Authorized` | `Denied` | `Expired`
//! - `expires_in` (seconds) comes from the IdP device-authorization response
//!   and is `None` when unknown

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::value::Secret;

/// Phase of an OIDC device-authorization flow (RFC 8628).
///
/// Serialized as `snake_case` literals used by the `rigorix_auth_login` and
/// poll outputs.
///
/// # Contract (Frozen)
///
/// - `Pending` — device code issued, awaiting user authorization at the IdP
/// - `Authorized` — user authorized and the token exchange succeeded
/// - `Denied` — user (or IdP policy) denied the request (`access_denied`)
/// - `Expired` — the device code expired before authorization (`expired_token`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceFlowStatus {
    /// Awaiting user authorization.
    Pending,
    /// User authorized; tokens obtained and custody persisted.
    Authorized,
    /// User or IdP policy denied the flow.
    Denied,
    /// Device code expired before the user authorized.
    Expired,
}

impl DeviceFlowStatus {
    /// True when the flow has reached a terminal state.
    pub fn is_terminal(&self) -> bool {
        !matches!(self, DeviceFlowStatus::Pending)
    }
}

/// State of an in-flight OIDC device authorization grant.
///
/// Created by `AuthService::login` (initiation), advanced by
/// `AuthService::poll` until terminal, per RFC 8628 §3.1–3.5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceFlowState {
    /// Correlation id for the login session.
    pub session_id: String,

    /// Device code exchanged for tokens (redacted everywhere).
    pub device_code: Secret<String>,

    /// URL the human opens to authorize.
    pub verification_uri: String,

    /// Human-readable code to enter at the verification URI.
    pub user_code: String,

    /// Seconds until the device code expires (RFC 8628 §3.2).
    pub expires_in: u64,

    /// When the device code expires (derived from `expires_in`).
    pub expires_at: DateTime<Utc>,

    /// Minimum polling interval in seconds (RFC 8628 §3.3 — `interval`,
    /// honoured and doubled on `slow_down`).
    pub interval_secs: u64,

    /// Current phase of the flow.
    pub status: DeviceFlowStatus,
}

impl DeviceFlowState {
    /// True while the device code is still valid and awaiting authorization.
    pub fn is_pending(&self) -> bool {
        self.status == DeviceFlowStatus::Pending && self.expires_at > Utc::now()
    }

    /// True when the flow can never complete (terminal or expired code).
    pub fn is_ended(&self) -> bool {
        self.status.is_terminal() || self.expires_at <= Utc::now()
    }
}
