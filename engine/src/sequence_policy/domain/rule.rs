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
}
