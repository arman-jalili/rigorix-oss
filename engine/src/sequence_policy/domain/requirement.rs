//! StepRequirement — R9 operator-controlled obligations on matched steps.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#r9--operator-controlled-step-requirements-adr-015
//! @canonical .pi/architecture/decisions/ADR-015-operator-step-requirements.md
//! Implements: ISSUE #884 (F-20260921-02 / GAP-A-28) — StepRequirement,
//!   RequirementAction, RequirementFinding
//!
//! Sequence rules express **negative constraints over values that are
//! present** ("if A then B is forbidden"). They cannot state an obligation
//! that a matched step MUST carry a property. Requirements close that gap:
//! an operator declares that every step matching a [`StepPredicate`] MUST be
//! attested (`require_identity`) and/or MUST carry a set of canonical
//! parameters (`require_params`).
//!
//! Requirements are **operator config** (same trust surface as rules),
//! evaluated at plan time for every plan — intent, template, and MCP
//! `rigorix_execute` — independent of what the (agent-authored) plan
//! declares. They carry no domain semantics: the operator names a pointer
//! (`/beneficiary`), the domain supplies its value, and the engine checks
//! presence only.
//!
//! # Contract (Frozen)
//! - `match` reuses the frozen [`StepPredicate`] matcher (tool exact/glob +
//!   parameter predicates)
//! - `require_identity` defaults `false`; when unmet the result is always a
//!   refusal (`Deny`, never promotable — a human approval cannot stand in for
//!   an identity)
//! - `require_params` are JSON pointers that must resolve to a present,
//!   non-null value; missing → the configured `action`
//! - `action` defaults `deny`
//! - A [`RequirementFinding`] records pointer **names** only — parameter
//!   **values** are never captured (SpanPrivacy)

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::SequencePolicyError;
use super::rule::{ParamMatchKind, StepPredicate, json_pointer_lookup};

/// Action taken when a matched step does not satisfy a requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RequirementAction {
    /// Refuse the plan before dispatch (`RequirementUnmet`). Default.
    #[default]
    Deny,
    /// Set `requires_approval = true` on the matched step — the existing
    /// approval pause/resume chain decides. Promotion applies to parameter
    /// obligations only; an unmet identity is always refused.
    Promote,
}

/// One operator-controlled obligation over steps matching a [`StepPredicate`].
///
/// Operator TOML (in `.rigorix/sequence-policy.toml`):
///
/// ```toml
/// [[requirements]]
/// id = "payout-guard"
/// name = "Payout commands must be attested and carry canonical effect data"
/// description = "A raw run_command must not reach the payout script without identity + effect key"
/// match = { tool = "run_command", params = [{ pointer = "/command", kind = "glob", value = "*execute_payout.sh*" }] }
/// require_identity = true
/// require_params = ["/beneficiary", "/effect_key"]
/// action = "deny"   # deny (default) | promote
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepRequirement {
    /// Stable identifier, e.g. `"payout-guard"`.
    pub id: String,
    /// Human-readable requirement name (surfaced to approvers / audit).
    pub name: String,
    /// Why the requirement exists.
    #[serde(default)]
    pub description: String,
    /// Selects the steps the requirement governs (same matcher as rules).
    ///
    /// `match` is a Rust keyword — serde renames the field to the operator
    /// TOML key `match`.
    #[serde(rename = "match")]
    pub r#match: StepPredicate,
    /// When `true`, the caller MUST present an attested identity
    /// (`idp_token` / `local_principal`). Unmet → refuse (`IdentityRequired`);
    /// never promotable. Defaults `false`.
    #[serde(default)]
    pub require_identity: bool,
    /// JSON pointers (e.g. `"/beneficiary"`) that MUST resolve to a present,
    /// non-null value in the matched step's parameters. Defaults empty.
    #[serde(default)]
    pub require_params: Vec<String>,
    /// Action for unmet parameter obligations: `deny` (default) or `promote`.
    #[serde(default)]
    pub action: RequirementAction,
}

impl StepRequirement {
    /// Evaluate this requirement against one planned step.
    ///
    /// Returns `Ok(None)` when the step does not match, or when it satisfies
    /// every obligation. Returns `Ok(Some(finding))` when the step matches
    /// but fails at least one obligation — the caller branches on
    /// `finding.action` (identity failures are forced to
    /// [`RequirementAction::Deny`]).
    ///
    /// # Errors
    /// - `SequencePolicyError::InvalidConfig` — a `regex` parameter predicate
    ///   in `match` fails to compile (operator config error; fail closed)
    pub fn evaluate(
        &self,
        step_name: &str,
        tool: &str,
        parameters: &Value,
        identity_attested: bool,
    ) -> Result<Option<RequirementFinding>, SequencePolicyError> {
        if !self.r#match.matches(tool, parameters)? {
            return Ok(None);
        }
        let unmet_identity = self.require_identity && !identity_attested;
        let unmet_params: Vec<String> = self
            .require_params
            .iter()
            .filter(|pointer| !pointer_present(parameters, pointer))
            .cloned()
            .collect();
        if !unmet_identity && unmet_params.is_empty() {
            return Ok(None);
        }
        // Attestation is never promotable: a human approval cannot substitute
        // for an identity (ADR-015).
        let action = if unmet_identity {
            RequirementAction::Deny
        } else {
            self.action
        };
        Ok(Some(RequirementFinding {
            requirement_id: self.id.clone(),
            requirement_name: self.name.clone(),
            step: step_name.to_string(),
            unmet_identity,
            unmet_params,
            action,
        }))
    }
}

/// A recorded, redacted requirement outcome for one matched step.
///
/// Summary fields only: the requirement id/name, the step, which obligations
/// were unmet, and the action taken. Parameter **names** may appear
/// (`unmet_params`); parameter **values** never do (SpanPrivacy).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementFinding {
    /// Stable id of the requirement that fired.
    pub requirement_id: String,
    /// Human-readable requirement name.
    pub requirement_name: String,
    /// Name of the matched step that failed the requirement.
    pub step: String,
    /// Whether the identity obligation was unmet.
    pub unmet_identity: bool,
    /// Pointer names whose value was missing / null (never their values).
    pub unmet_params: Vec<String>,
    /// Effective action: `deny` (including every unmet identity) or `promote`.
    pub action: RequirementAction,
}

impl RequirementFinding {
    /// Redacted one-line summary — parameter values are never included.
    pub fn decision_summary(&self) -> String {
        let mut unmet: Vec<String> = Vec::new();
        if self.unmet_identity {
            unmet.push("attested identity".to_string());
        }
        for p in &self.unmet_params {
            unmet.push(format!("required parameter '{p}'"));
        }
        format!(
            "operator requirement '{}' {} step '{}' (missing: {})",
            self.requirement_id,
            match self.action {
                RequirementAction::Deny => "denied",
                RequirementAction::Promote => "promoted",
            },
            self.step,
            unmet.join(", ")
        )
    }
}

/// Whether a JSON pointer resolves to a present, non-null value.
fn pointer_present(parameters: &Value, pointer: &str) -> bool {
    json_pointer_lookup(parameters, pointer).is_some_and(|v| !v.is_null())
}

/// Whether a requirement `match` predicate uses any parameter kind that is
/// invalid for a single-step obligation. `equals_step` compares against an
/// **earlier matched step** of a sequence rule — there is no such context for
/// a requirement, so an operator using it is a fail-closed config error.
pub(crate) fn invalid_match_predicate(predicate: &StepPredicate) -> Option<String> {
    for p in &predicate.params {
        if matches!(p.kind, ParamMatchKind::EqualsStep) {
            return Some(format!(
                "match parameter predicate '{}' uses equals_step, which requires an earlier \
                 matched step and is not valid for a step requirement",
                p.pointer
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn payout_requirement() -> StepRequirement {
        StepRequirement {
            id: "payout-guard".to_string(),
            name: "Payout commands must be attested and carry effect data".to_string(),
            description: "d".to_string(),
            r#match: StepPredicate {
                tool: "run_command".to_string(),
                params: vec![super::super::rule::ParamPredicate {
                    pointer: "/command".to_string(),
                    kind: ParamMatchKind::Glob,
                    value: Some("*execute_payout.sh*".to_string()),
                    step: None,
                }],
            },
            require_identity: true,
            require_params: vec!["/beneficiary".to_string(), "/effect_key".to_string()],
            action: RequirementAction::Deny,
        }
    }

    #[test]
    fn non_matching_step_produces_no_finding() {
        let req = payout_requirement();
        let f = req
            .evaluate(
                "probe",
                "run_command",
                &json!({ "command": "curl http://example.com" }),
                true,
            )
            .expect("eval");
        assert!(f.is_none());
    }

    #[test]
    fn missing_identity_and_params_reports_deny() {
        let req = payout_requirement();
        let f = req
            .evaluate(
                "pay",
                "run_command",
                &json!({ "command": "bash execute_payout.sh a b" }),
                false,
            )
            .expect("eval")
            .expect("finding");
        assert!(f.unmet_identity);
        assert_eq!(f.unmet_params, vec!["/beneficiary", "/effect_key"]);
        // Identity unmet is never promotable, even if the config said promote.
        assert_eq!(f.action, RequirementAction::Deny);
    }

    #[test]
    fn satisfied_requirement_produces_no_finding() {
        let req = payout_requirement();
        let f = req
            .evaluate(
                "pay",
                "run_command",
                &json!({
                    "command": "bash execute_payout.sh a b",
                    "beneficiary": "acct-1",
                    "effect_key": "eff-1",
                }),
                true,
            )
            .expect("eval");
        assert!(f.is_none());
    }

    #[test]
    fn null_pointer_is_absent() {
        let mut req = payout_requirement();
        req.require_identity = false;
        let f = req
            .evaluate(
                "pay",
                "run_command",
                &json!({
                    "command": "bash execute_payout.sh a b",
                    "beneficiary": "acct-1",
                    "effect_key": null,
                }),
                true,
            )
            .expect("eval")
            .expect("finding");
        assert_eq!(f.unmet_params, vec!["/effect_key"]);
        assert_eq!(f.action, RequirementAction::Deny);
        assert!(!f.decision_summary().contains("acct-1"));
    }

    #[test]
    fn promote_action_is_applied_to_parameter_obligations() {
        let mut req = payout_requirement();
        req.require_identity = false;
        req.action = RequirementAction::Promote;
        let f = req
            .evaluate(
                "pay",
                "run_command",
                &json!({ "command": "bash execute_payout.sh a b" }),
                false,
            )
            .expect("eval")
            .expect("finding");
        assert_eq!(f.action, RequirementAction::Promote);
        assert!(!f.unmet_identity);
    }

    #[test]
    fn equals_step_match_predicate_is_rejected_by_helper() {
        let mut req = payout_requirement();
        req.r#match.params[0].kind = ParamMatchKind::EqualsStep;
        assert!(invalid_match_predicate(&req.r#match).is_some());
    }
}
