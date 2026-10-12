//! ProviderSpec / ProviderDetermination — an operator-configured external
//! determination service used as a precondition.
//!
//! @canonical .pi/architecture/modules/precondition.md
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: ISSUE-PRECONDITION-PROVIDERS (consequence-gating epic)
//!
//! A `provider` precondition is the second, equally operator-owned form of a
//! check: instead of an argv program the operator declares an HTTP
//! determination service. The engine calls it at the **same** enforcement point
//! as the check program, at dispatch.
//!
//! # Non-inheritance (normative)
//!
//! The provider's permitting answer is **evidence, not authority**
//! ([`docs/spec/consequence-gating-v1.md`] §2.7). It enters the same place an
//! operator check result does: a permitting provider result never satisfies the
//! step on its own — every *other* matching precondition is still evaluated, and
//! a denying provider result refuses. There is deliberately no
//! "provider says allow" short-circuit.
//!
//! # Fail-closed
//!
//! Unreachable, timeout, non-2xx, unparseable, and unknown determination
//! vocabulary all map to [`ProviderDetermination::Unknown`], which refuses. An
//! unknown vocabulary never becomes authority.
//!
//! [`docs/spec/consequence-gating-v1.md`]: ../../../../docs/spec/consequence-gating-v1.md

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::precondition::DEFAULT_TIMEOUT_MS;

const fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}

fn default_determination_field() -> String {
    "determination".to_string()
}

/// The operator's vocabulary that maps to a permit.
pub const DEFAULT_PERMIT_VALUES: &[&str] = &[
    "allow",
    "allowed",
    "permit",
    "permitted",
    "pass",
    "passed",
    "ok",
    "granted",
    "approved",
];

/// The operator's vocabulary that maps to a denial.
pub const DEFAULT_DENY_VALUES: &[&str] = &[
    "deny", "denied", "block", "blocked", "refuse", "refused", "reject", "rejected", "fail",
    "failed",
];

/// HTTP method for the provider request. `POST` (default) sends the check input
/// as the JSON body; `GET` sends it as a query parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderMethod {
    /// `GET` — the check input is carried in the request (no body).
    Get,
    /// `POST` — the check input is the JSON request body. Default.
    #[default]
    Post,
}

/// Operator-declared external determination service.
///
/// All fields are operator-owned (`.rigorix/**` is not agent-writable,
/// ADR-013 R5). `credential_ref` names a credential the operator holds; the
/// credential **value** is never read by the domain and is never logged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSpec {
    /// Stable provider id, recorded in the evidence (never the credential).
    pub id: String,
    /// Operator-declared endpoint URL. Recorded **redacted** in the evidence
    /// (scheme + host + a one-way hash of the path); never with a query string.
    pub endpoint: String,
    /// Request method. Defaults to [`ProviderMethod::Post`].
    #[serde(default)]
    pub method: ProviderMethod,
    /// Wall-clock timeout (milliseconds). Defaults to
    /// [`DEFAULT_TIMEOUT_MS`]; bounded by the precondition safety caps.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// Name of a credential the operator holds (read from the environment by
    /// the operator's process / client). Never the credential value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_ref: Option<String>,
    /// JSON field in the provider's response that carries the determination.
    /// Defaults to `"determination"`.
    #[serde(default = "default_determination_field")]
    pub determination_field: String,
    /// Additional operator values that map to a permit (additive to
    /// [`DEFAULT_PERMIT_VALUES`]).
    #[serde(default)]
    pub permit_values: Vec<String>,
    /// Additional operator values that map to a denial (additive to
    /// [`DEFAULT_DENY_VALUES`]).
    #[serde(default)]
    pub deny_values: Vec<String>,
}

impl ProviderSpec {
    /// Map a provider vocabulary token to a determination.
    ///
    /// Matching is case-insensitive and trimmed. Anything not in the permit or
    /// deny vocabulary is [`ProviderDetermination::Unknown`] — and unknown
    /// **refuses** (it must never become authority).
    pub fn map_determination(&self, raw: &str) -> ProviderDetermination {
        let token = raw.trim().to_ascii_lowercase();
        if token.is_empty() {
            return ProviderDetermination::Unknown;
        }
        let permits = DEFAULT_PERMIT_VALUES
            .iter()
            .copied()
            .chain(self.permit_values.iter().map(String::as_str));
        if permits.into_iter().any(|candidate| candidate == token) {
            return ProviderDetermination::Permit;
        }
        let denies = DEFAULT_DENY_VALUES
            .iter()
            .copied()
            .chain(self.deny_values.iter().map(String::as_str));
        if denies.into_iter().any(|candidate| candidate == token) {
            return ProviderDetermination::Deny;
        }
        ProviderDetermination::Unknown
    }

    /// Validate the provider declaration (structural; fail closed).
    ///
    /// # Errors
    /// - `String` — a human-readable reason the declaration is invalid.
    pub fn validate(&self, max_timeout_ms: u64) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("provider with empty `id`".to_string());
        }
        if self.endpoint.trim().is_empty() {
            return Err(format!(
                "provider '{}': endpoint must be a non-empty URL",
                self.id
            ));
        }
        if !(self.endpoint.starts_with("http://") || self.endpoint.starts_with("https://")) {
            return Err(format!("provider '{}': endpoint must be http(s)", self.id));
        }
        if self.timeout_ms == 0 || self.timeout_ms > max_timeout_ms {
            return Err(format!(
                "provider '{}': timeout_ms {} must be in 1..={}",
                self.id, self.timeout_ms, max_timeout_ms
            ));
        }
        if self.determination_field.trim().is_empty() {
            return Err(format!(
                "provider '{}': determination_field must be non-empty",
                self.id
            ));
        }
        Ok(())
    }
}

/// The mapped determination of a provider call.
///
/// `Unknown` covers unreachable / timeout / non-2xx / unparseable / unrecognized
/// vocabulary; it **refuses**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderDetermination {
    /// The provider permits. Evidence only — never authority on its own.
    Permit,
    /// The provider denies. Refuses.
    Deny,
    /// The determination could not be established (any ambiguity). Refuses.
    Unknown,
}

impl ProviderDetermination {
    /// Canonical lowercase wire string.
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderDetermination::Permit => "permit",
            ProviderDetermination::Deny => "deny",
            ProviderDetermination::Unknown => "unknown",
        }
    }

    /// Whether this determination permits. Only [`Self::Permit`] does, and even
    /// then it is not authority (non-inheritance).
    pub fn permits(&self) -> bool {
        matches!(self, ProviderDetermination::Permit)
    }
}

/// The result of one provider invocation, with the digests that bind it.
///
/// `request_digest` / `response_digest` are one-way `sha256:<hex>` over the
/// exact bytes sent and received; `endpoint_redacted` is the endpoint with its
/// path replaced by a one-way hash. No credential, no query string, and no
/// absolute path appear here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderObservation {
    /// The mapped determination.
    pub determination: ProviderDetermination,
    /// The provider's raw vocabulary token, when one was parsed. Recorded for
    /// operability; it is not secret material (the operator's vocabulary).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_value: Option<String>,
    /// When the provider was consulted.
    pub observed_at: DateTime<Utc>,
    /// The provider's self-reported version, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_version: Option<String>,
    /// One-way hash of the exact request bytes.
    pub request_digest: String,
    /// One-way hash of the exact response bytes.
    pub response_digest: String,
    /// The endpoint, redacted (scheme + host + path hash).
    pub endpoint_redacted: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> ProviderSpec {
        ProviderSpec {
            id: "standing".to_string(),
            endpoint: "https://determine.example/v1/check".to_string(),
            method: ProviderMethod::Post,
            timeout_ms: 1000,
            credential_ref: Some("STANDING_TOKEN".to_string()),
            determination_field: "determination".to_string(),
            permit_values: vec![],
            deny_values: vec![],
        }
    }

    #[test]
    fn default_vocabulary_maps_case_insensitively() {
        let spec = spec();
        assert_eq!(
            spec.map_determination("ALLOW"),
            ProviderDetermination::Permit
        );
        assert_eq!(
            spec.map_determination(" denied "),
            ProviderDetermination::Deny
        );
        // Unknown vocabulary must not become authority.
        assert_eq!(
            spec.map_determination("maybe"),
            ProviderDetermination::Unknown
        );
        assert_eq!(spec.map_determination(""), ProviderDetermination::Unknown);
    }

    #[test]
    fn operator_vocabulary_is_additive() {
        let mut spec = spec();
        spec.permit_values = vec!["green".to_string()];
        spec.deny_values = vec!["red".to_string()];
        assert_eq!(
            spec.map_determination("green"),
            ProviderDetermination::Permit
        );
        assert_eq!(spec.map_determination("red"), ProviderDetermination::Deny);
        // Defaults still apply.
        assert_eq!(
            spec.map_determination("allow"),
            ProviderDetermination::Permit
        );
    }

    #[test]
    fn validate_rejects_bad_endpoint_and_timeout() {
        let mut bad = spec();
        bad.endpoint = "ftp://x".to_string();
        assert!(bad.validate(60_000).is_err());

        let mut slow = spec();
        slow.timeout_ms = 0;
        assert!(slow.validate(60_000).is_err());

        let mut over = spec();
        over.timeout_ms = 60_001;
        assert!(over.validate(60_000).is_err());

        assert!(spec().validate(60_000).is_ok());
    }

    #[test]
    fn only_permit_permits() {
        assert!(ProviderDetermination::Permit.permits());
        assert!(!ProviderDetermination::Deny.permits());
        assert!(!ProviderDetermination::Unknown.permits());
        assert_eq!(ProviderDetermination::Unknown.as_str(), "unknown");
    }
}
