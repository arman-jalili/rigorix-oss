//! Precondition — the domain model of one operator-authored dispatch-time
//! authority check.
//!
//! @canonical .pi/architecture/modules/consequence-gating.md#precondition
//! @canonical .pi/architecture/modules/consequence-gating.md#config
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: Contract Freeze — Precondition, FailureAction, SafetyCaps,
//!   PreconditionConfig
//! Issue: #938 (consequence-gating epic — contract freeze); behavior closed in
//!   ISSUE-CONSEQUENCE-GATING-1 (Precondition) and -2 (PreconditionRepository)
//!
//! A precondition is a **deterministic, operator-authored external check**
//! evaluated at the moment of consequence. It reuses the frozen
//! [`StepPredicate`](crate::sequence_policy::domain::StepPredicate) matcher
//! (tool exact/glob + JSON-pointer parameter predicates) rather than forking a
//! second matcher.
//!
//! Operator TOML (`.rigorix/preconditions.toml`, operator-owned; agents cannot
//! write `.rigorix/**` — ADR-013 R5):
//!
//! ```toml
//! [[preconditions]]
//! id = "beneficiary-eligible"
//! match = { tool = "payment_execute" }
//! require_params = ["/beneficiary", "/amount"]
//! command = ["/opt/rigorix/checks/beneficiary-eligible"]
//! timeout_ms = 5000
//! failure = "deny"
//! capture_output = false
//!
//! [gating]
//! release_dependents_on_failure = false
//! ```
//!
//! # Contract (Frozen)
//! - `id`, `match`, `require_params`, `command`, `timeout_ms`, `failure`,
//!   `capture_output` are the complete config surface (serde round-trip
//!   preserves every field)
//! - `timeout_ms` defaults to 5000; `failure` defaults to `deny` (the only v1
//!   action); `capture_output` defaults to `false` (stdout is never recorded
//!   by default — SpanPrivacy)
//! - `command` is an **argv array**, never a shell string; `command[0]` is the
//!   check program and must resolve outside the agent-writable workspace
//! - `require_params` are JSON pointers that must resolve to a present,
//!   non-null value; missing → refuse **before** the command runs (ADR-015
//!   presence-only semantics)
//! - A missing `.rigorix/preconditions.toml` is **not** an error
//!   (`Ok(None)` — fail-open-absent); a malformed / over-cap one is
//!   **fail-closed** (`Err`)
//! - `validate(&SafetyCaps)` is a load-time gate; its behavior lands in the
//!   PreconditionRepository / fail-closed implementation issue

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::sequence_policy::domain::StepPredicate;

use super::error::PreconditionError;
use super::gating::GatingMode;

/// Default wall-clock timeout for a precondition check (milliseconds).
pub const DEFAULT_TIMEOUT_MS: u64 = 5_000;

const fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}

const fn default_fail_closed() -> bool {
    true
}

/// Action taken when a matched precondition's check is refused.
///
/// v1 has exactly one action: `deny`. `failure = "deny"` is the operator's
/// explicit statement that a negative answer stops the consequence. There is
/// deliberately **no allow-on-error mode** (ADR-017 §R1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FailureAction {
    /// Refuse the matched step; the tool is never called. Default (and only)
    /// v1 action.
    #[default]
    Deny,
}

/// One operator-authored dispatch-time precondition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Precondition {
    /// Stable identifier, e.g. `"beneficiary-eligible"`.
    pub id: String,
    /// Selects the steps this precondition governs — the reused ADR-013
    /// `StepPredicate` matcher (tool exact/glob + parameter predicates).
    ///
    /// `match` is a Rust keyword; serde renames the field to the operator TOML
    /// key `match`.
    #[serde(rename = "match")]
    pub r#match: StepPredicate,
    /// JSON pointers (e.g. `"/beneficiary"`) that MUST resolve to a present,
    /// non-null value in the matched step's parameters. Missing → refuse
    /// **before** the command runs. Defaults empty.
    #[serde(default)]
    pub require_params: Vec<String>,
    /// The check program and its arguments as an **argv array** — no shell, no
    /// string interpolation. `command[0]` must resolve outside the
    /// agent-writable workspace (trust boundary).
    pub command: Vec<String>,
    /// Wall-clock timeout (milliseconds). Defaults to
    /// [`DEFAULT_TIMEOUT_MS`].
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// Action on a refused check. Defaults to [`FailureAction::Deny`].
    #[serde(default)]
    pub failure: FailureAction,
    /// When `true`, the check's (truncated, redacted) stdout may be recorded.
    /// Defaults `false` — stdout is never captured into evidence by default
    /// (SpanPrivacy).
    #[serde(default)]
    pub capture_output: bool,
}

impl Precondition {
    /// Whether this precondition matches the given tool + parameters.
    ///
    /// Delegates to the frozen `StepPredicate` matcher; a malformed operator
    /// predicate (e.g. an uncompilable `regex`) is a `Match` error (fail
    /// closed).
    ///
    /// # Errors
    /// - `PreconditionError::Match` — the predicate could not be evaluated
    ///
    /// # Implementation
    /// TODO: ISSUE-CONSEQUENCE-GATING-1 — delegate to
    /// `StepPredicate::matches`, mapping `SequencePolicyError` to
    /// `PreconditionError::Match`.
    pub fn matches(&self, _tool: &str, _parameters: &Value) -> Result<bool, PreconditionError> {
        todo!("ISSUE-CONSEQUENCE-GATING-1: delegate match to StepPredicate::matches")
    }
}

/// Safety caps for the loaded precondition set (mirrors the sequence-policy
/// `SafetyCaps` fail-closed posture). Every cap is a resource-exhaustion
/// bound: the engine runs operator-supplied processes, so the config surface
/// itself must be bounded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafetyCaps {
    /// Maximum number of `[[preconditions]]` entries per config file.
    pub max_preconditions_per_file: u32,
    /// Maximum number of argv elements in a precondition `command`.
    pub max_argv_args: u32,
    /// Maximum wall-clock `timeout_ms` an operator may request.
    pub max_timeout_ms: u64,
    /// Maximum `require_params` pointers per precondition.
    pub max_require_params: u32,
}

impl Default for SafetyCaps {
    /// Concrete default caps (the values the frozen contract tests use):
    /// 100 preconditions / 32 argv args / 60s timeout / 16 required params.
    fn default() -> Self {
        Self {
            max_preconditions_per_file: 100,
            max_argv_args: 32,
            max_timeout_ms: 60_000,
            max_require_params: 16,
        }
    }
}

/// The loaded `.rigorix/preconditions.toml`.
///
/// Additive: a missing file is `Ok(None)` at the repository boundary; an
/// empty `[[preconditions]]` list means no gating (fail-open-absent).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreconditionConfig {
    /// Fail closed on config errors. Defaults `true` — a corrupt precondition
    /// file must refuse a matching step rather than silently degrade.
    #[serde(default = "default_fail_closed")]
    pub fail_closed: bool,
    /// The ordered precondition set (empty → no dispatch-time gating).
    #[serde(default)]
    pub preconditions: Vec<Precondition>,
    /// Step-outcome gating (`[gating]`). Additive; defaults to today's
    /// behavior.
    #[serde(default)]
    pub gating: GatingMode,
}

impl Default for PreconditionConfig {
    fn default() -> Self {
        Self {
            fail_closed: true,
            preconditions: Vec::new(),
            gating: GatingMode::default(),
        }
    }
}

impl PreconditionConfig {
    /// Whether any precondition is configured. `false` → the dispatch gate is a
    /// no-op (fail-open-absent, status quo).
    pub fn is_empty(&self) -> bool {
        self.preconditions.is_empty()
    }

    /// Validate this config against the safety caps.
    ///
    /// Returns `Ok(())` when every precondition is within the caps and
    /// structurally valid (non-empty `id`, non-empty `command`, pointers start
    /// with `/`, timeout within `max_timeout_ms`).
    ///
    /// # Errors
    /// - `PreconditionError::ConfigInvalid` — a structurally invalid entry
    ///
    /// # Implementation
    /// TODO: ISSUE-CONSEQUENCE-GATING-2 — enforce `SafetyCaps` over
    /// `self.preconditions` (fail closed).
    pub fn validate(&self, _caps: &SafetyCaps) -> Result<(), PreconditionError> {
        todo!("ISSUE-CONSEQUENCE-GATING-2: enforce SafetyCaps over self.preconditions")
    }

    /// Load-time convenience: validate against the concrete default caps.
    ///
    /// # Implementation
    /// TODO: ISSUE-CONSEQUENCE-GATING-2.
    pub fn validate_with_default_caps(&self) -> Result<(), PreconditionError> {
        todo!("ISSUE-CONSEQUENCE-GATING-2: validate against SafetyCaps::default()")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_are_deny_timeout_and_no_capture() {
        let parsed: Precondition = serde_json::from_value(json!({
            "id": "beneficiary-eligible",
            "match": { "tool": "payment_execute" },
            "command": ["/opt/rigorix/checks/beneficiary-eligible"]
        }))
        .expect("minimal precondition parses");
        assert_eq!(parsed.timeout_ms, DEFAULT_TIMEOUT_MS);
        assert_eq!(parsed.failure, FailureAction::Deny);
        assert!(!parsed.capture_output);
        assert!(parsed.require_params.is_empty());
    }

    #[test]
    fn config_defaults_are_fail_closed_and_release_dependents() {
        let config = PreconditionConfig::default();
        assert!(config.fail_closed);
        assert!(config.is_empty());
        assert!(config.gating.release_dependents_on_failure);
    }
}
