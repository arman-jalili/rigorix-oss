//! Domain events for the Auth bounded context.
//!
//! @canonical .pi/architecture/modules/auth.md#events
//! Implements: Contract Freeze — auth event payload schemas
//! ADR-008: device flow lifecycle events
//!
//! These events are emitted throughout the identity lifecycle. Consumers
//! (logger, TUI, metrics, alerts) subscribe to these event types.
//!
//! # Event Catalog
//!
//! | Event | Description | Payload | Consumers |
//! |-------|-------------|---------|-----------|
//! | AuthLoginStarted | Device flow initiated | `{ session_id, verification_uri, user_code }` | Logger, TUI |
//! | AuthLoginSucceeded | Token exchange completed | `{ session_id, subject, issuer, token_ttl_secs }` | Logger, Metrics |
//! | AuthLoginFailed | Device flow failed (denied/expired/error) | `{ session_id, error_type, reason }` | Logger, Alerts |
//! | AuthStatusChecked | Identity status queried | `{ session_id, status, claim_summary }` | Logger |
//! | AuthLoggedOut | Tokens cleared | `{ session_id, revoked }` | Logger |
//!
//! # Contract (Frozen)
//!
//! - Every event carries `session_id` and `timestamp` for correlation
//! - Serialized as a tagged union with `#[serde(tag = "type")]`
//! - Events are facts — immutable and append-only
//! - No behavior — pure data
//! - Never carries raw token material (redacted surfaces only)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::status::TokenStatus;
use super::value::ClaimSummary;

/// All domain events emitted by the Auth bounded context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthEvent {
    /// OIDC device flow initiated — the human must open `verification_uri`
    /// and enter `user_code`.
    AuthLoginStarted {
        /// Correlation id for the login session.
        session_id: String,

        /// URL the human opens to authorize.
        verification_uri: String,

        /// Human-readable code to enter at the verification URI.
        user_code: String,

        /// When the flow was initiated.
        timestamp: DateTime<Utc>,
    },

    /// Token exchange completed — refresh token in keychain, access token in
    /// memory.
    AuthLoginSucceeded {
        /// Correlation id for the login session.
        session_id: String,

        /// Subject of the attested identity.
        subject: String,

        /// Issuer that vouches for the subject.
        issuer: String,

        /// TTL of the short-TTL access token (seconds).
        token_ttl_secs: u64,

        /// When the exchange completed.
        timestamp: DateTime<Utc>,
    },

    /// Device flow failed — user denied, code expired, or transport error.
    AuthLoginFailed {
        /// Correlation id for the login session.
        session_id: String,

        /// Typed failure marker (`access_denied`, `expired`, `transport`, …).
        error_type: String,

        /// Human-readable reason.
        reason: String,

        /// When the failure occurred.
        timestamp: DateTime<Utc>,
    },

    /// Identity status was queried (`rigorix_auth_status`).
    AuthStatusChecked {
        /// Correlation id for the status check session.
        session_id: String,

        /// Token lifecycle status.
        status: TokenStatus,

        /// Redacted claim summary when authenticated (`None` otherwise).
        claim_summary: Option<ClaimSummary>,

        /// When the status was read.
        timestamp: DateTime<Utc>,
    },

    /// Logout completed — tokens cleared from keychain and memory.
    AuthLoggedOut {
        /// Correlation id for the logout session.
        session_id: String,

        /// True when the refresh token was revoked at the IdP.
        revoked: bool,

        /// When the logout occurred.
        timestamp: DateTime<Utc>,
    },
}
