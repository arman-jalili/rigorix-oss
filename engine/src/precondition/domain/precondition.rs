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

use crate::sequence_policy::domain::{ParamMatchKind, StepPredicate};

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
    ///   (fail closed)
    pub fn matches(&self, tool: &str, parameters: &Value) -> Result<bool, PreconditionError> {
        // Reuse the frozen ADR-013 matcher (tool exact/glob + JSON-pointer
        // parameter predicates). A malformed operator predicate is a
        // fail-closed `Match` error, never a silent non-match.
        self.r#match
            .matches(tool, parameters)
            .map_err(|error| PreconditionError::Match {
                precondition_id: self.id.clone(),
                detail: error.to_string(),
            })
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
    /// - `PreconditionError::ConfigInvalid` — a structurally invalid or
    ///   over-cap entry (fail closed)
    pub fn validate(&self, caps: &SafetyCaps) -> Result<(), PreconditionError> {
        if self.preconditions.len() as u32 > caps.max_preconditions_per_file {
            return Err(PreconditionError::ConfigInvalid(format!(
                "{} preconditions exceeds cap max_preconditions_per_file={}",
                self.preconditions.len(),
                caps.max_preconditions_per_file
            )));
        }
        let mut seen_ids: Vec<&str> = Vec::with_capacity(self.preconditions.len());
        for precondition in &self.preconditions {
            if precondition.id.trim().is_empty() {
                return Err(PreconditionError::ConfigInvalid(
                    "precondition with empty `id`".to_string(),
                ));
            }
            if seen_ids.contains(&precondition.id.as_str()) {
                return Err(PreconditionError::ConfigInvalid(format!(
                    "duplicate precondition id '{}'",
                    precondition.id
                )));
            }
            seen_ids.push(&precondition.id);

            // The check program is `command[0]`; an empty argv cannot run and
            // is a config typo, not an implicit no-op (fail closed).
            if precondition.command.is_empty() {
                return Err(PreconditionError::ConfigInvalid(format!(
                    "precondition '{}': command must contain at least the program (argv[0])",
                    precondition.id
                )));
            }
            if precondition.command.len() as u32 > caps.max_argv_args {
                return Err(PreconditionError::ConfigInvalid(format!(
                    "precondition '{}': {} argv elements exceed cap max_argv_args={}",
                    precondition.id,
                    precondition.command.len(),
                    caps.max_argv_args
                )));
            }
            if precondition.timeout_ms == 0 || precondition.timeout_ms > caps.max_timeout_ms {
                return Err(PreconditionError::ConfigInvalid(format!(
                    "precondition '{}': timeout_ms {} must be in 1..={}",
                    precondition.id, precondition.timeout_ms, caps.max_timeout_ms
                )));
            }
            if precondition.require_params.len() as u32 > caps.max_require_params {
                return Err(PreconditionError::ConfigInvalid(format!(
                    "precondition '{}': {} required params exceed cap max_require_params={}",
                    precondition.id,
                    precondition.require_params.len(),
                    caps.max_require_params
                )));
            }
            for pointer in &precondition.require_params {
                if !pointer.starts_with('/') {
                    return Err(PreconditionError::ConfigInvalid(format!(
                        "precondition '{}': required parameter pointer '{}' must start with '/'",
                        precondition.id, pointer
                    )));
                }
            }

            // Validate the reused StepPredicate. A literal kind needs a value;
            // a regex must compile; `equals_step` needs an earlier matched step
            // of a sequence rule and is invalid for a single-step precondition
            // (fail closed).
            for predicate in &precondition.r#match.params {
                match predicate.kind {
                    ParamMatchKind::Exact | ParamMatchKind::Glob => {
                        if predicate.value.is_none() {
                            return Err(PreconditionError::ConfigInvalid(format!(
                                "precondition '{}': parameter predicate '{}' (kind {:?}) requires `value`",
                                precondition.id, predicate.pointer, predicate.kind
                            )));
                        }
                    }
                    ParamMatchKind::Regex => {
                        let Some(pattern) = predicate.value.as_deref() else {
                            return Err(PreconditionError::ConfigInvalid(format!(
                                "precondition '{}': regex predicate '{}' requires `value`",
                                precondition.id, predicate.pointer
                            )));
                        };
                        regex::Regex::new(pattern).map_err(|error| {
                            PreconditionError::ConfigInvalid(format!(
                                "precondition '{}': regex predicate '{}' failed to compile: {error}",
                                precondition.id, predicate.pointer
                            ))
                        })?;
                    }
                    ParamMatchKind::EqualsStep => {
                        return Err(PreconditionError::ConfigInvalid(format!(
                            "precondition '{}': match predicate '{}' uses equals_step, which requires \
                             an earlier matched step and is not valid for a single-step precondition",
                            precondition.id, predicate.pointer
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// Load-time convenience: validate against the concrete default caps.
    pub fn validate_with_default_caps(&self) -> Result<(), PreconditionError> {
        self.validate(&SafetyCaps::default())
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

    fn eligible() -> Precondition {
        serde_json::from_value(json!({
            "id": "beneficiary-eligible",
            "match": {
                "tool": "payment_execute",
                "params": [
                    { "pointer": "/beneficiary", "kind": "exact", "value": "acct-1" }
                ]
            },
            "require_params": ["/beneficiary", "/amount"],
            "command": ["/opt/rigorix/checks/beneficiary-eligible"]
        }))
        .expect("precondition parses")
    }

    #[test]
    fn matches_tool_and_parameter_predicates() {
        let precondition = eligible();
        assert!(
            precondition
                .matches("payment_execute", &json!({ "beneficiary": "acct-1" }))
                .expect("match")
        );
        // Wrong parameter value → no match.
        assert!(
            !precondition
                .matches("payment_execute", &json!({ "beneficiary": "acct-2" }))
                .expect("match")
        );
        // Wrong tool → no match.
        assert!(
            !precondition
                .matches("payment_cancel", &json!({ "beneficiary": "acct-1" }))
                .expect("match")
        );
    }

    #[test]
    fn invalid_operator_regex_is_a_fail_closed_match_error() {
        let precondition: Precondition = serde_json::from_value(json!({
            "id": "bad-regex",
            "match": {
                "tool": "payment_execute",
                "params": [{ "pointer": "/beneficiary", "kind": "regex", "value": "(unclosed" }]
            },
            "command": ["/opt/rigorix/checks/bad-regex"]
        }))
        .expect("precondition parses");
        let err = precondition
            .matches("payment_execute", &json!({ "beneficiary": "acct-1" }))
            .expect_err("fail closed");
        assert!(matches!(err, PreconditionError::Match { .. }));
        assert!(!err.is_retriable());
    }

    fn base() -> Precondition {
        serde_json::from_value(json!({
            "id": "base",
            "match": { "tool": "payment_execute" },
            "command": ["/opt/rigorix/checks/base"]
        }))
        .expect("base parses")
    }

    #[test]
    fn validate_accepts_a_within_caps_config() {
        let config = PreconditionConfig {
            fail_closed: true,
            preconditions: vec![base(), eligible()],
            gating: GatingMode::default(),
        };
        assert!(config.validate_with_default_caps().is_ok());
        assert!(!config.is_empty());
    }

    #[test]
    fn validate_rejects_duplicate_ids_and_empty_command() {
        let duplicate = PreconditionConfig {
            preconditions: vec![base(), base()],
            ..PreconditionConfig::default()
        };
        assert!(matches!(
            duplicate.validate_with_default_caps(),
            Err(PreconditionError::ConfigInvalid(_))
        ));

        let mut empty = base();
        empty.command = Vec::new();
        assert!(matches!(
            PreconditionConfig {
                preconditions: vec![empty],
                ..PreconditionConfig::default()
            }
            .validate_with_default_caps(),
            Err(PreconditionError::ConfigInvalid(_))
        ));
    }

    #[test]
    fn validate_rejects_over_cap_argv_and_timeout() {
        let caps = SafetyCaps {
            max_argv_args: 2,
            max_timeout_ms: 1_000,
            ..SafetyCaps::default()
        };
        let mut long_argv = base();
        long_argv.command = vec!["a".into(), "b".into(), "c".into()];
        assert!(matches!(
            PreconditionConfig {
                preconditions: vec![long_argv],
                ..PreconditionConfig::default()
            }
            .validate(&caps),
            Err(PreconditionError::ConfigInvalid(_))
        ));

        let mut slow = base();
        slow.timeout_ms = 5_000;
        assert!(matches!(
            PreconditionConfig {
                preconditions: vec![slow],
                ..PreconditionConfig::default()
            }
            .validate(&caps),
            Err(PreconditionError::ConfigInvalid(_))
        ));
    }

    #[test]
    fn validate_rejects_bad_pointer_and_equals_step() {
        let mut bad_pointer = base();
        bad_pointer.require_params = vec!["beneficiary".to_string()];
        assert!(matches!(
            PreconditionConfig {
                preconditions: vec![bad_pointer],
                ..PreconditionConfig::default()
            }
            .validate_with_default_caps(),
            Err(PreconditionError::ConfigInvalid(_))
        ));

        let mut equals_step: Precondition = serde_json::from_value(json!({
            "id": "eq",
            "match": {
                "tool": "payment_execute",
                "params": [{ "pointer": "/beneficiary", "kind": "equals_step", "step": 0 }]
            },
            "command": ["/opt/rigorix/checks/eq"]
        }))
        .expect("parses");
        equals_step.require_params.clear();
        assert!(matches!(
            PreconditionConfig {
                preconditions: vec![equals_step],
                ..PreconditionConfig::default()
            }
            .validate_with_default_caps(),
            Err(PreconditionError::ConfigInvalid(_))
        ));
    }
}
