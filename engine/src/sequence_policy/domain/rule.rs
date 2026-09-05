//! SequenceRule — one declarative rule over an ordered step sequence.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#sequencerule
//! Implements: Contract Freeze — SequenceRule, StepPredicate, ParamPredicate,
//!   ParamMatchKind, RuleAction
//! Issue: #838 (sequence-policy epic — contract freeze); behavior in
//!   ISSUE-SEQUENCE-POLICY-1 (SequenceRule) and ISSUE-SEQUENCE-POLICY-4
//!   (StepPredicate)
//!
//! A rule describes an **ordered pair (or windowed chain)** of step
//! predicates. Predicates match on `tool` (exact or glob) and optionally on
//! parameter values (JSON pointer + exact/glob/regex value predicate). Rules
//! carry an `action`:
//!
//! | Action | Semantics |
//! |--------|-----------|
//! | `promote` (default) | Later matched step becomes `requires_approval = true` — human decides |
//! | `deny` | Later matched step is denied before dispatch (structured `SequencePolicyDenied` node failure) |
//!
//! No rule may reference LLM output, model intent, or conversation content —
//! matching is over the execution plan/prefix only (R1, R4).
//!
//! # Contract (Frozen)
//! - Serde round-trip preserves every field (`id`, `name`, `description`,
//!   ordered `steps`, optional `window`, `action`)
//! - Wire kinds serialize lowercase: `kind = "exact" | "glob" | "regex"`,
//!   `action = "promote" | "deny"`
//! - `action` defaults to `promote` when omitted; `params` defaults to empty
//!   when omitted (backward-compatible config evolution)
//! - Matching *behavior* (tool glob, param exact/glob/regex) is implemented in
//!   the ISSUE-SEQUENCE-POLICY-1 / ISSUE-SEQUENCE-POLICY-4 / -5 issues — the
//!   data contract is frozen here

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::SequencePolicyError;

/// Action taken on the later matched step of a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    /// Later matched step is built `requires_approval = true` — the existing
    /// approval pause/resume chain decides (default).
    #[default]
    Promote,
    /// Later matched step is denied before dispatch (`SequencePolicyDenied`).
    Deny,
}

/// Kind of value comparison applied by a `ParamPredicate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamMatchKind {
    /// Exact string equality.
    Exact,
    /// Glob match over the value (e.g. `conf-*`).
    Glob,
    /// Regular-expression match over the value.
    Regex,
}

/// One parameter predicate: a JSON pointer into the step's parameter object
/// plus a value predicate (exact / glob / regex).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParamPredicate {
    /// JSON pointer into the step parameters, e.g. `"/event_id"`.
    pub pointer: String,
    /// How `value` is compared (exact | glob | regex).
    pub kind: ParamMatchKind,
    /// Expected value string for the predicate.
    pub value: String,
}

/// A step predicate: matches a tool name (exact or glob) plus optional
/// parameter predicates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepPredicate {
    /// Tool name — exact or glob, e.g. `"registration_remove"`,
    /// `"registration_*"`.
    pub tool: String,
    /// Optional parameter predicates; empty means "any parameters".
    #[serde(default)]
    pub params: Vec<ParamPredicate>,
}

impl StepPredicate {
    /// Whether a step (tool + parameter object) satisfies this predicate.
    ///
    /// # Matching contract (AC row 2)
    /// - `tool` matches exactly or by glob (`*` wildcard, e.g.
    ///   `registration_*`)
    /// - Every parameter predicate must match: its JSON pointer is resolved
    ///   against the step's parameter object, then the actual value is
    ///   compared by `kind` — exact string equality, glob, or regex
    /// - A **missing** pointed-to parameter or a non-matching value → the
    ///   predicate does **not** match
    /// - A predicate with no parameter predicates matches on tool alone
    ///
    /// # Errors
    /// - `SequencePolicyError::InvalidConfig` — a `regex` parameter predicate
    ///   fails to compile (operator config error; fail closed)
    pub fn matches(&self, tool: &str, parameters: &Value) -> Result<bool, SequencePolicyError> {
        if !tool_matches(&self.tool, tool) {
            return Ok(false);
        }
        for pp in &self.params {
            match json_pointer_lookup(parameters, &pp.pointer) {
                Some(actual) => {
                    if !value_matches(actual, pp)? {
                        return Ok(false);
                    }
                }
                None => return Ok(false),
            }
        }
        Ok(true)
    }
}

/// Exact or glob (`*` wildcard) string match.
fn tool_matches(pattern: &str, text: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == text;
    }
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<usize> = None;
    let mut backtrack_ti = 0usize;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '*' || p[pi] == t[ti]) {
            if p[pi] == '*' {
                star = Some(pi);
                backtrack_ti = ti;
                pi += 1;
            } else {
                pi += 1;
                ti += 1;
            }
        } else if let Some(star_pi) = star {
            pi = star_pi + 1;
            backtrack_ti += 1;
            ti = backtrack_ti;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Resolve a JSON pointer (`/a/b`) against a value object. Array-index
/// segments are not part of the frozen step-parameter schema.
fn json_pointer_lookup<'a>(value: &'a Value, pointer: &str) -> Option<&'a Value> {
    if pointer.is_empty() {
        return Some(value);
    }
    let mut current = value;
    for segment in pointer.split('/').skip(1) {
        match current {
            Value::Object(map) => current = map.get(segment)?,
            _ => return None,
        }
    }
    Some(current)
}

/// Apply a parameter predicate's kind (exact / glob / regex) to an actual
/// value. Invalid regex patterns surface as a config error (fail closed).
fn value_matches(actual: &Value, predicate: &ParamPredicate) -> Result<bool, SequencePolicyError> {
    let text = match actual {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    match predicate.kind {
        ParamMatchKind::Exact => Ok(text == predicate.value),
        ParamMatchKind::Glob => Ok(tool_matches(&predicate.value, &text)),
        ParamMatchKind::Regex => match regex::Regex::new(&predicate.value) {
            Ok(re) => Ok(re.is_match(&text)),
            Err(e) => Err(SequencePolicyError::InvalidConfig(format!(
                "regex predicate '{}' failed to compile: {e}",
                predicate.value
            ))),
        },
    }
}

/// One declarative rule over an ordered step sequence.
///
/// Example (the conference case from the module spec):
///
/// ```toml
/// [[rules]]
/// id = "registration-remove-then-reassign"
/// name = "No remove-then-reassign of a full event seat"
/// description = "Removing an attendee to free a seat, then registering the requester, is never autonomous"
/// steps = [
///   { tool = "registration_remove", params = [{ pointer = "/event_id", kind = "exact", value = "conf-2026" }] },
///   { tool = "registration_add",    params = [{ pointer = "/event_id", kind = "exact", value = "conf-2026" }] },
/// ]
/// window = 3
/// action = "promote"   # the add step pauses for a human
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceRule {
    /// Stable identifier, e.g. `"registration-remove-then-reassign"`.
    pub id: String,
    /// Human-readable rule name.
    pub name: String,
    /// Why the rule exists (surfaced to approvers / audit summaries).
    pub description: String,
    /// Ordered step predicates — a match requires `steps[i]` to appear before
    /// `steps[i + 1]` in the evaluated ordered step list.
    pub steps: Vec<StepPredicate>,
    /// Maximum index gap between the first and last matched step
    /// (default: adjacent pair, gap of 1).
    pub window: Option<u32>,
    /// Action on match: `promote` (default) or `deny`.
    #[serde(default)]
    pub action: RuleAction,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The canonical conference rule exactly as an operator authors it in
    /// `.rigorix/sequence-policy.toml` (module spec §Configuration).
    const CONFERENCE_RULE_TOML: &str = r#"
[[rules]]
id = "registration-remove-then-reassign"
name = "No remove-then-reassign of a full event seat"
description = "Removing an attendee to free a seat, then registering the requester, is never autonomous"
steps = [
  { tool = "registration_remove", params = [{ pointer = "/event_id", kind = "exact", value = "conf-2026" }] },
  { tool = "registration_add",    params = [{ pointer = "/event_id", kind = "exact", value = "conf-2026" }] },
]
window = 3
action = "promote"
"#;

    /// Wrapper struct mirroring the `[[rules]]` table under `[sequence_policy]`.
    #[derive(serde::Deserialize)]
    struct RulesFile {
        #[serde(default)]
        rules: Vec<SequenceRule>,
    }

    #[test]
    fn toml_parse_yields_rule_with_ordered_predicates_and_action() {
        let file: RulesFile =
            toml::from_str(CONFERENCE_RULE_TOML).expect("operator TOML parses into SequenceRule");
        assert_eq!(file.rules.len(), 1);
        let rule = &file.rules[0];

        assert_eq!(rule.id, "registration-remove-then-reassign");
        assert_eq!(rule.name, "No remove-then-reassign of a full event seat");
        // Ordered predicates preserved in rule order.
        assert_eq!(rule.steps.len(), 2);
        assert_eq!(rule.steps[0].tool, "registration_remove");
        assert_eq!(rule.steps[1].tool, "registration_add");
        // Param predicates (JSON pointer + kind + value) survive the parse.
        assert_eq!(rule.steps[0].params[0].pointer, "/event_id");
        assert_eq!(rule.steps[0].params[0].kind, ParamMatchKind::Exact);
        assert_eq!(rule.steps[0].params[0].value, "conf-2026");
        assert_eq!(rule.window, Some(3));
        assert_eq!(rule.action, RuleAction::Promote);
    }

    #[test]
    fn toml_round_trip_preserves_all_fields() {
        let file: RulesFile = toml::from_str(CONFERENCE_RULE_TOML).expect("parse");
        let rule = &file.rules[0];

        // Serialize back to TOML and re-parse — every field must survive.
        let encoded = toml::to_string(rule).expect("serialize to TOML");
        let decoded: SequenceRule = toml::from_str(&encoded).expect("re-parse TOML");
        assert_eq!(decoded, *rule);
    }

    #[test]
    fn toml_parse_defaults_action_to_promote_and_params_to_empty() {
        // Action omitted → promote (safe default). Param predicate omitted →
        // tool-only match. Window omitted → adjacent-pair semantics.
        let minimal: SequenceRule = toml::from_str(
            r#"
id = "tool-only-pair"
name = "n"
description = "d"
steps = [
  { tool = "registration_remove" },
  { tool = "registration_add" },
]
"#,
        )
        .expect("minimal rule parses");
        assert_eq!(minimal.action, RuleAction::Promote);
        assert_eq!(minimal.window, None);
        assert!(minimal.steps[0].params.is_empty());
        assert!(minimal.steps[1].params.is_empty());
    }

    #[test]
    fn toml_parse_supports_deny_action_and_glob_and_regex_kinds() {
        let deny: SequenceRule = toml::from_str(
            r#"
id = "hard-deny"
name = "n"
description = "d"
steps = [
  { tool = "registration_*", params = [{ pointer = "/event_id", kind = "glob", value = "conf-*" }] },
  { tool = "registration_add" },
]
action = "deny"
"#,
        )
        .expect("deny rule parses");
        assert_eq!(deny.action, RuleAction::Deny);
        assert_eq!(deny.steps[0].params[0].kind, ParamMatchKind::Glob);

        let regex_rule: SequenceRule = toml::from_str(
            r#"
id = "regex-param"
name = "n"
description = "d"
steps = [
  { tool = "user_delete", params = [{ pointer = "/email", kind = "regex", value = "^admin@.*" }] },
  { tool = "user_create" },
]
"#,
        )
        .expect("regex rule parses");
        assert_eq!(regex_rule.steps[0].params[0].kind, ParamMatchKind::Regex);
    }
    // ── StepPredicate::matches (AC row 2) ──────────────────────────────────

    fn predicate(tool: &str, params: Vec<ParamPredicate>) -> StepPredicate {
        StepPredicate {
            tool: tool.to_string(),
            params,
        }
    }

    fn param(pointer: &str, kind: ParamMatchKind, value: &str) -> ParamPredicate {
        ParamPredicate {
            pointer: pointer.to_string(),
            kind,
            value: value.to_string(),
        }
    }

    #[test]
    fn step_predicate_tool_exact_match() {
        let p = predicate("registration_remove", vec![]);
        assert!(p.matches("registration_remove", &json!({})).expect("m"));
        assert!(!p.matches("registration_add", &json!({})).expect("m"));
    }

    #[test]
    fn step_predicate_tool_glob_match() {
        let p = predicate("registration_*", vec![]);
        assert!(p.matches("registration_remove", &json!({})).expect("m"));
        assert!(p.matches("registration_add", &json!({})).expect("m"));
        // Glob is not a substring match — must anchor at the start.
        assert!(!p.matches("audit_registration_add", &json!({})).expect("m"));
        assert!(!p.matches("registration", &json!({})).expect("m"));
    }

    #[test]
    fn step_predicate_param_exact_match_and_non_match() {
        let p = predicate(
            "registration_remove",
            vec![param("/event_id", ParamMatchKind::Exact, "conf-2026")],
        );
        // Matching param value.
        assert!(
            p.matches("registration_remove", &json!({ "event_id": "conf-2026" }))
                .expect("m")
        );
        // Non-matching param value → no match.
        assert!(
            !p.matches("registration_remove", &json!({ "event_id": "conf-2025" }))
                .expect("m")
        );
        // Missing pointed-to parameter → no match.
        assert!(
            !p.matches("registration_remove", &json!({ "other": "x" }))
                .expect("m")
        );
        // Tool mismatch is decisive even when params would match.
        assert!(
            !p.matches("registration_add", &json!({ "event_id": "conf-2026" }))
                .expect("m")
        );
    }

    #[test]
    fn step_predicate_param_glob_and_regex_kinds() {
        let glob = predicate(
            "registration_add",
            vec![param("/event_id", ParamMatchKind::Glob, "conf-*")],
        );
        assert!(
            glob.matches("registration_add", &json!({ "event_id": "conf-2026" }))
                .expect("m")
        );
        assert!(
            !glob
                .matches(
                    "registration_add",
                    &json!({ "event_id": "unconference-2026" })
                )
                .expect("m")
        );

        let regex = predicate(
            "user_delete",
            vec![param("/email", ParamMatchKind::Regex, "^admin@")],
        );
        assert!(
            regex
                .matches("user_delete", &json!({ "email": "admin@example.com" }))
                .expect("m")
        );
        assert!(
            !regex
                .matches("user_delete", &json!({ "email": "user@example.com" }))
                .expect("m")
        );
    }

    #[test]
    fn step_predicate_all_param_predicates_must_match() {
        let p = predicate(
            "registration_add",
            vec![
                param("/event_id", ParamMatchKind::Exact, "conf-2026"),
                param("/seat_class", ParamMatchKind::Exact, "vip"),
            ],
        );
        assert!(
            p.matches(
                "registration_add",
                &json!({ "event_id": "conf-2026", "seat_class": "vip" }),
            )
            .expect("m")
        );
        // One of several predicates failing → no match.
        assert!(
            !p.matches(
                "registration_add",
                &json!({ "event_id": "conf-2026", "seat_class": "economy" }),
            )
            .expect("m")
        );
    }

    #[test]
    fn step_predicate_non_string_values_compare_as_strings() {
        // Numeric / boolean params compare via their string form.
        let p = predicate(
            "grant_role",
            vec![param("/role_id", ParamMatchKind::Exact, "42")],
        );
        assert!(
            p.matches("grant_role", &json!({ "role_id": 42 }))
                .expect("m")
        );
        let glob = predicate(
            "grant_role",
            vec![param("/role_id", ParamMatchKind::Glob, "4*")],
        );
        assert!(
            glob.matches("grant_role", &json!({ "role_id": 42 }))
                .expect("m")
        );
    }

    #[test]
    fn step_predicate_nested_pointer_and_absent_predicate_list() {
        let p = predicate(
            "run_command",
            vec![param("/env/region", ParamMatchKind::Exact, "eu")],
        );
        assert!(
            p.matches("run_command", &json!({ "env": { "region": "eu" } }))
                .expect("m")
        );
        assert!(
            !p.matches("run_command", &json!({ "env": { "region": "us" } }))
                .expect("m")
        );
        // No params → any parameters acceptable (tool-only predicate).
        let tool_only = predicate("run_command", vec![]);
        assert!(
            tool_only
                .matches("run_command", &json!({ "anything": [1, 2, 3] }))
                .expect("m")
        );
    }

    #[test]
    fn step_predicate_invalid_regex_is_a_config_error() {
        let p = predicate("x", vec![param("/a", ParamMatchKind::Regex, "(unclosed")]);
        let err = p
            .matches("x", &json!({ "a": "b" }))
            .expect_err("fail closed");
        assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));
        assert!(!err.is_retriable());
    }
}
