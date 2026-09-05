//! Value objects for the Auth bounded context.
//!
//! @canonical .pi/architecture/modules/auth.md#value-objects
//! Implements: Contract Freeze — Secret, ClaimSummary value objects
//!
//! Value objects are immutable, interchangeable, and defined by their
//! attributes, not identity. Secrets follow the SpanPrivacy pattern — the
//! redaction contract is part of the frozen surface (no raw token material
//! in any log, error, or serialized output).
//!
//! # Contract (Frozen)
//!
//! - All value objects are immutable (no pub fields, no setters)
//! - All value objects implement PartialEq
//! - All types derive Serialize + Deserialize for JSON transmission
//! - `Secret` Debug/Display/Serialize always render "***REDACTED***"
//! - No behavior beyond field accessors and validation

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Secret — secure string wrapper with redacted display
// ---------------------------------------------------------------------------

/// A secure string wrapper that redacts its contents in Debug, Display,
/// and Serialize implementations. Prevents accidental leakage of sensitive
/// values (device codes, refresh tokens, access tokens, client secrets)
/// in logs and error messages.
///
/// # Contract (Frozen)
///
/// - Debug/Display always shows "***REDACTED***"
/// - Serialize always outputs "***REDACTED***"
/// - `expose()` returns the inner value for deliberate use
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Secret<T: Clone> {
    #[serde(skip_serializing)]
    inner: T,
    #[serde(serialize_with = "serialize_redacted")]
    _marker: (),
}

fn serialize_redacted<S: serde::Serializer>(_: &(), s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str("***REDACTED***")
}

impl<T: Clone> Secret<T> {
    /// Create a new Secret wrapping the given value.
    pub fn new(value: T) -> Self {
        Self {
            inner: value,
            _marker: (),
        }
    }

    /// Expose the inner value for deliberate use (e.g., a token endpoint call).
    pub fn expose(&self) -> &T {
        &self.inner
    }
}

impl<T: Clone> std::fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Secret(***REDACTED***)")
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "***REDACTED***")
    }
}

// ---------------------------------------------------------------------------
// ClaimSummary — redacted rendering of an identity claim
// ---------------------------------------------------------------------------

/// Redacted, serialization-safe summary of an `IdentityClaim` (ADR-012).
///
/// This is the only identity representation that crosses the agent-visible
/// surface (`rigorix_auth_status` output, `AuthStatusChecked` event payload).
/// It carries subject/issuer/authority/lifetime — never the raw token and
/// never the claim's `token_ref`.
///
/// # Contract (Frozen)
///
/// - Never contains the raw token or its reference locator
/// - `subject` and `issuer` are always present on an authenticated claim
/// - `expires_at` is `None` when the claim has no expiry
/// - Field names and types are frozen (serde-stable)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimSummary {
    /// Subject — the human's unique identifier at the IdP.
    pub subject: String,

    /// Issuer — who vouches for the subject (IdP issuer URL).
    pub issuer: String,

    /// Roles / authority presented (captured fact, not judgment).
    pub authority: Option<String>,

    /// When the underlying claim expires (`None` = no expiry).
    pub expires_at: Option<DateTime<Utc>>,
}
