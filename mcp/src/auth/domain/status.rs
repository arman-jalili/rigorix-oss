//! Status enums for the Auth bounded context.
//!
//! @canonical .pi/architecture/modules/auth.md#status
//! Implements: Contract Freeze — TokenStatus enum
//!
//! # Contract (Frozen)
//!
//! - Serialized as `snake_case` strings (`authenticated`, `expired`,
//!   `unauthenticated`) — these literals appear in MCP tool outputs and
//!   event payloads (auth.md API Endpoints table)
//! - Display matches the serde representation (stable log/event text)

use serde::{Deserialize, Serialize};

/// Token lifecycle status reported by `rigorix_auth_status` and the
/// `AuthStatusChecked` event.
///
/// # Contract (Frozen)
///
/// - `Authenticated` — a valid (non-expired) short-TTL access token is present
/// - `Expired` — an access token is present but past its TTL (silent refresh
///   should recover; failure degrades to `Unauthenticated`)
/// - `Unauthenticated` — no credential material at all (or IdP unreachable)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenStatus {
    /// Valid access token present.
    Authenticated,

    /// Access token present but expired.
    Expired,

    /// No active identity material.
    Unauthenticated,
}

impl TokenStatus {
    /// True when an access token is usable right now.
    pub fn is_authenticated(&self) -> bool {
        matches!(self, TokenStatus::Authenticated)
    }
}

impl std::fmt::Display for TokenStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Matches the frozen serde snake_case representation.
        let rendered = match self {
            TokenStatus::Authenticated => "authenticated",
            TokenStatus::Expired => "expired",
            TokenStatus::Unauthenticated => "unauthenticated",
        };
        f.write_str(rendered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_matches_serde_representation() {
        assert_eq!(TokenStatus::Authenticated.to_string(), "authenticated");
        assert_eq!(TokenStatus::Expired.to_string(), "expired");
        assert_eq!(TokenStatus::Unauthenticated.to_string(), "unauthenticated");
        let json = serde_json::to_string(&TokenStatus::Authenticated).unwrap();
        assert_eq!(json, "\"authenticated\"");
    }
}
