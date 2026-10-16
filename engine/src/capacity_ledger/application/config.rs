//! Operator declaration surface for capacity-ledger governance (ADR-018).
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#enforcementseam
//! ADR-018: distributed capacity ledger
//! Implements: ISSUE-CAPACITY-LEDGER-SEAM — the opt-in declaration surface
//!
//! A resource is ledger-governed **only if declared**. No declaration means the
//! engine takes exactly its previous code path (no adapter, no reserve, no
//! network, no shared state). That is what keeps "OSS runs standalone with no
//! database" true while still allowing a distributed guarantee when declared.
//!
//! The declaration is intentionally **domain-free**: the engine never learns
//! what a resource *means*, only that it is limited and how much a matching step
//! claims. All domain semantics stay in the operator's config and in the
//! preconditions that enforce them.
//!
//! # Loading semantics (mirrors the repository boundary)
//! - **missing file** -> no governance (fail-open-absent, status quo)
//! - **valid file** -> declared resources + step/claim rules (armed)
//! - **malformed / over cap** -> refuse to arm (fail closed, never a downgrade)

use serde::{Deserialize, Serialize};

use crate::capacity_ledger::application::factory::LedgerBackendConfig;
use crate::capacity_ledger::domain::{ResourceId, ResourceSpec, Window};

use super::setup::CapacitySetupError;

/// Safety caps. A declaration beyond these is refused rather than truncated:
/// silently dropping operator intent would change what is governed.
pub const MAX_RESOURCES: usize = 256;
/// Maximum number of step/claim rules.
pub const MAX_GRANTS: usize = 1024;
/// Maximum lease a rule may request (24h).
pub const MAX_LEASE_SECONDS: u64 = 86_400;
/// Default per-request deadline for the HTTP adapter.
pub const DEFAULT_TIMEOUT_MS: u64 = 5_000;
/// Default response body cap for the HTTP adapter (64 KiB).
pub const DEFAULT_MAX_RESPONSE_BODY_BYTES: usize = 64 * 1024;

/// A step/claim rule: which step claims how much of which resource.
///
/// Matching is on the step name, optionally narrowed by tool. A rule is the
/// *only* way capacity is claimed; there is no implicit or inferred claiming.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapacityGrantRule {
    /// Step name this rule applies to (exact match).
    pub step: String,

    /// Optional tool narrowing; `None` matches any tool.
    #[serde(default)]
    pub tool: Option<String>,

    /// The declared resource to claim from.
    pub resource: String,

    /// Units claimed by one dispatch of this step. Must be greater than zero.
    pub amount: u64,

    /// Lease for the hold, in seconds. Bounded by [`MAX_LEASE_SECONDS`].
    pub lease_seconds: u64,
}

impl CapacityGrantRule {
    /// Whether this rule governs the given dispatch.
    ///
    /// Takes plain strings rather than another context's DTO so the capacity
    /// context does not depend on a sibling context.
    pub fn matches(&self, step_name: &str, tool: &str) -> bool {
        if self.step != step_name {
            return false;
        }
        match &self.tool {
            Some(expected) => expected == tool,
            None => true,
        }
    }
}

/// Parsed and validated operator configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapacityLedgerConfig {
    /// Which adapter to build.
    pub backend: LedgerBackendConfig,
    /// Declared resources (limits).
    pub resources: Vec<ResourceSpec>,
    /// Step/claim rules.
    pub grants: Vec<CapacityGrantRule>,
}

impl CapacityLedgerConfig {
    /// The rule governing a dispatch, if any.
    pub fn rule_for(&self, step_name: &str, tool: &str) -> Option<&CapacityGrantRule> {
        self.grants.iter().find(|r| r.matches(step_name, tool))
    }

    /// Whether any rule exists (an armed seam).
    pub fn is_armed(&self) -> bool {
        !self.grants.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Raw TOML shape
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    backend: Option<RawBackend>,
    #[serde(default)]
    resources: Vec<RawResource>,
    #[serde(default)]
    grants: Vec<RawGrant>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBackend {
    kind: String,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    max_response_body_bytes: Option<usize>,
    /// Environment variable holding the bearer token, never the token itself.
    #[serde(default)]
    auth_token_env: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawResource {
    id: String,
    #[serde(default)]
    name: Option<String>,
    hard_limit: u64,
    #[serde(default)]
    window_seconds: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGrant {
    step: String,
    #[serde(default)]
    tool: Option<String>,
    resource: String,
    amount: u64,
    #[serde(default = "default_lease_seconds")]
    lease_seconds: u64,
}

fn default_lease_seconds() -> u64 {
    300
}

fn invalid(detail: impl Into<String>) -> CapacitySetupError {
    CapacitySetupError::ConfigInvalid {
        detail: detail.into(),
    }
}

/// Parse and validate a `.rigorix/capacity.toml` document.
///
/// Validation is deliberately strict and fail-closed: an ambiguous declaration
/// is refused rather than guessed, because a guessed declaration would govern
/// the wrong thing.
pub fn parse(text: &str) -> Result<CapacityLedgerConfig, CapacitySetupError> {
    let raw: RawConfig = toml::from_str(text)
        .map_err(|e| invalid(format!(".rigorix/capacity.toml could not be parsed: {e}")))?;

    if raw.resources.len() > MAX_RESOURCES {
        return Err(invalid(format!(
            "too many resources declared ({}, max {MAX_RESOURCES})",
            raw.resources.len()
        )));
    }
    if raw.grants.len() > MAX_GRANTS {
        return Err(invalid(format!(
            "too many grant rules declared ({}, max {MAX_GRANTS})",
            raw.grants.len()
        )));
    }

    let mut resources: Vec<ResourceSpec> = Vec::with_capacity(raw.resources.len());
    for r in &raw.resources {
        if r.id.trim().is_empty() {
            return Err(invalid("a declared resource has an empty id"));
        }
        if r.hard_limit == 0 {
            return Err(invalid(format!(
                "resource '{}' declares hard_limit = 0, which can never be satisfied",
                r.id
            )));
        }
        if resources.iter().any(|e| e.id().as_str() == r.id) {
            return Err(invalid(format!("resource '{}' is declared twice", r.id)));
        }
        let window = match r.window_seconds {
            Some(0) | None => Window::UNBOUNDED,
            Some(secs) => Window::bounded(secs),
        };
        let name = r.name.clone().unwrap_or_else(|| r.id.clone());
        resources.push(ResourceSpec::new(
            ResourceId::new(r.id.clone()),
            name,
            r.hard_limit,
            window,
        ));
    }

    let mut grants: Vec<CapacityGrantRule> = Vec::with_capacity(raw.grants.len());
    for g in &raw.grants {
        if g.step.trim().is_empty() {
            return Err(invalid("a grant rule has an empty step name"));
        }
        if g.amount == 0 {
            return Err(invalid(format!(
                "grant rule for step '{}' claims 0, which would govern nothing",
                g.step
            )));
        }
        if g.lease_seconds == 0 || g.lease_seconds > MAX_LEASE_SECONDS {
            return Err(invalid(format!(
                "grant rule for step '{}' has lease_seconds = {} (must be 1..={MAX_LEASE_SECONDS})",
                g.step, g.lease_seconds
            )));
        }
        if !resources.iter().any(|r| r.id().as_str() == g.resource) {
            return Err(invalid(format!(
                "grant rule for step '{}' references undeclared resource '{}'",
                g.step, g.resource
            )));
        }
        grants.push(CapacityGrantRule {
            step: g.step.clone(),
            tool: g.tool.clone(),
            resource: g.resource.clone(),
            amount: g.amount,
            lease_seconds: g.lease_seconds,
        });
    }

    let backend = match raw.backend {
        None => LedgerBackendConfig::InProcess,
        Some(b) => match b.kind.as_str() {
            "in_process" => LedgerBackendConfig::InProcess,
            "http" => {
                let base_url = b
                    .base_url
                    .ok_or_else(|| invalid("backend kind 'http' requires base_url"))?;
                if !base_url.starts_with("https://") && !base_url.starts_with("http://") {
                    return Err(invalid("backend base_url must be an absolute http(s) URL"));
                }
                // The token is read from the environment at construction time and
                // never written to the config file or to a log line.
                let auth_token = match b.auth_token_env {
                    Some(var) => match std::env::var(&var) {
                        Ok(v) if !v.is_empty() => Some(v),
                        _ => {
                            return Err(invalid(format!(
                                "backend auth_token_env names '{var}' but the variable is missing or empty"
                            )));
                        }
                    },
                    None => None,
                };
                LedgerBackendConfig::Http {
                    base_url,
                    timeout_ms: b.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS),
                    max_response_body_bytes: b
                        .max_response_body_bytes
                        .unwrap_or(DEFAULT_MAX_RESPONSE_BODY_BYTES),
                    auth_token,
                }
            }
            other => {
                return Err(invalid(format!(
                    "unknown backend kind '{other}' (expected 'in_process' or 'http')"
                )));
            }
        },
    };

    Ok(CapacityLedgerConfig {
        backend,
        resources,
        grants,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
[[resources]]
id = "daily-payout"
hard_limit = 100
window_seconds = 86400

[[grants]]
step = "payout"
resource = "daily-payout"
amount = 5
lease_seconds = 300
"#;

    #[test]
    fn parses_a_minimal_declaration() {
        let cfg = parse(MINIMAL).expect("valid");
        assert_eq!(cfg.resources.len(), 1);
        assert_eq!(cfg.grants.len(), 1);
        assert!(cfg.is_armed());
        assert!(matches!(cfg.backend, LedgerBackendConfig::InProcess));
    }

    #[test]
    fn rule_matching_respects_optional_tool_narrowing() {
        let cfg = parse(MINIMAL).unwrap();
        assert!(cfg.rule_for("payout", "any-tool").is_some());
        assert!(cfg.rule_for("other", "any-tool").is_none());
    }

    #[test]
    fn undeclared_resource_reference_is_refused() {
        let text = r#"
[[resources]]
id = "a"
hard_limit = 1

[[grants]]
step = "s"
resource = "missing"
amount = 1
"#;
        assert!(parse(text).is_err(), "must refuse an undeclared resource");
    }

    #[test]
    fn zero_amount_is_refused() {
        let text = r#"
[[resources]]
id = "a"
hard_limit = 10

[[grants]]
step = "s"
resource = "a"
amount = 0
"#;
        assert!(parse(text).is_err(), "a zero claim governs nothing");
    }

    #[test]
    fn duplicate_resource_is_refused() {
        let text = r#"
[[resources]]
id = "a"
hard_limit = 1

[[resources]]
id = "a"
hard_limit = 2
"#;
        assert!(parse(text).is_err());
    }

    #[test]
    fn unknown_fields_are_refused_not_ignored() {
        let text = r#"
[[resources]]
id = "a"
hard_limit = 1
typo_field = true
"#;
        assert!(
            parse(text).is_err(),
            "a typo must not silently narrow governance"
        );
    }

    #[test]
    fn no_grants_means_not_armed() {
        let text = r#"
[[resources]]
id = "a"
hard_limit = 1
"#;
        let cfg = parse(text).unwrap();
        assert!(!cfg.is_armed());
    }
}
