//! Status enums for the Auth bounded context.
//!
//! @canonical .pi/architecture/modules/auth.md#status
//! Implements: Contract Freeze — TokenStatus enum
//!
//! # Contract (Frozen)
//!
//! - Serialized as `snake_case` strings (`authenticated`, `expired`,
//!   `unauthenticated`, `unverified`) — these literals appear in MCP tool
//!   outputs and event payloads (auth.md API Endpoints table)
//! - Display matches the serde representation (stable log/event text)
//! - `unverified` is the same word the engine's `IdentityRequired` refusal
//!   prints (#1277): one word for one fact, never a second spelling

use serde::{Deserialize, Serialize};

/// Token lifecycle status reported by `rigorix_auth_status` and the
/// `AuthStatusChecked` event.
///
/// # Contract (Frozen)
///
/// - `Authenticated` — a valid (non-expired) short-TTL access token is present
///   **and** its claim was checked
/// - `Expired` — an access token is present but past its TTL (silent refresh
///   should recover; failure degrades to `Unauthenticated`)
/// - `Unauthenticated` — no credential material at all. Nothing to check.
/// - `Unverified` — credential material is present and cannot be checked: the
///   IdP was unreachable when this process started (or discovered no
///   `jwks_uri`), so the claim is carried but unattested. The caller did log
///   in; the process cannot vouch for it. Never rendered as
///   `authenticated`, because a check that did not run must not read like one
///   that passed (#1277).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenStatus {
    /// Valid access token present and its claim checked.
    Authenticated,

    /// Access token present but expired.
    Expired,

    /// No active identity material at all.
    Unauthenticated,

    /// Credential material present, verification unavailable.
    Unverified,
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
            TokenStatus::Unverified => "unverified",
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
        assert_eq!(TokenStatus::Unverified.to_string(), "unverified");
        let json = serde_json::to_string(&TokenStatus::Authenticated).unwrap();
        assert_eq!(json, "\"authenticated\"");
    }

    /// #1277 — the fourth literal is distinct from `Unauthenticated` on every
    /// surface: a session that logged in and could not be checked is not a
    /// session with no credential at all.
    #[test]
    fn unverified_is_its_own_literal_and_never_authenticated() {
        assert_ne!(TokenStatus::Unverified, TokenStatus::Unauthenticated);
        assert!(!TokenStatus::Unverified.is_authenticated());
        assert_eq!(
            serde_json::to_string(&TokenStatus::Unverified).unwrap(),
            "\"unverified\""
        );
        assert_eq!(
            serde_json::from_str::<TokenStatus>("\"unverified\"").unwrap(),
            TokenStatus::Unverified
        );
    }
}
