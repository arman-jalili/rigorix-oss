//! SequencePolicyServiceImpl — the concrete sequence-policy service.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#sequencepolicyservice
//! Implements: ISSUE-SEQUENCE-POLICY-2 — `evaluate_plan` / `evaluate_prefix`
//! Issue: #840 (SequencePolicyService)
//!
//! The service loads the rule config per run through the injected
//! `SequencePolicyRepository`, then evaluates the ordered step list
//! deterministically:
//!
//! - `evaluate_plan` (R2): every rule whose ordered predicates match a window
//!   in the fully-materialized step list yields a `SequenceMatch` naming the
//!   later matched step (the one to promote / deny).
//! - `evaluate_prefix` (R3): the completed dispatch prefix plus the next node
//!   are evaluated as one ordered list; only windows that **complete on the
//!   next node** (last matched index == `prefix.len()`) are returned — windows
//!   already completed inside the prefix were acted on at their own dispatch
//!   time and are not re-reported.
//!
//! Degradation semantics (frozen in the contract freeze):
//! - Missing config file (`Ok(None)`) → no rules → no matches (fail-open)
//! - Corrupt / over-cap config (`Err`) → propagated → plan refused / run halts
//!   (fail closed)
//!
//! Matching details (deterministic, R4):
//! - Predicates match `tool` exactly or by glob (`*` wildcard), plus optional
//!   parameter predicates (JSON pointer → exact / glob / regex value match)
//! - A rule with `steps = [A, B, …]` requires A before B before … in the
//!   evaluated list; `window` caps the span between the first and last matched
//!   index; `window = None` means adjacent (span == steps.len() - 1)
//! - Windows are found by earliest extension from each qualifying start index,
//!   scanning rules in config order — deterministic for identical input
//! - An invalid regex in a parameter predicate surfaces as
//!   `SequencePolicyError::InvalidConfig` (fail closed); per-load compilation
//!   lands with the Matcher hardening (ISSUE-SEQUENCE-POLICY-5)

use async_trait::async_trait;
use serde_json::Value;

use crate::sequence_policy::domain::{
    ParamMatchKind, ParamPredicate, SequenceMatch, SequencePolicyError, SequenceRule, StepPredicate,
};
use crate::sequence_policy::infrastructure::repository::SequencePolicyRepository;

use super::dto::{DispatchedStep, PlannedStep};
use super::service::SequencePolicyService;

/// Default `SequencePolicyService` implementation.
///
/// # Construction
/// - `new(repository)` — inject the rule-config repository (filesystem-backed
///   `TomlSequencePolicyRepository`, or a signed-bundle seam for enterprise,
///   P3)
pub struct SequencePolicyServiceImpl {
    repository: Box<dyn SequencePolicyRepository>,
}

impl SequencePolicyServiceImpl {
    /// Create the service over the given rule-config repository.
    pub fn new(repository: Box<dyn SequencePolicyRepository>) -> Self {
        Self { repository }
    }

    /// Load the rule config for this run. `Ok(None)` (no config file) is
    /// mapped to an empty rule set — fail-open-absent.
    async fn load_rules(&self) -> Result<Vec<SequenceRule>, SequencePolicyError> {
        match self.repository.load_config().await? {
            Some(config) => Ok(config.rules),
            None => Ok(Vec::new()),
        }
    }
}

#[async_trait]
impl SequencePolicyService for SequencePolicyServiceImpl {
    async fn evaluate_plan(
        &self,
        steps: &[PlannedStep],
    ) -> Result<Vec<SequenceMatch>, SequencePolicyError> {
        let rules = self.load_rules().await?;
        let views: Vec<StepView<'_>> = steps
            .iter()
            .map(|s| StepView {
                name: &s.name,
                tool: &s.tool,
                params: &s.parameters,
            })
            .collect();
        match_rules(&rules, &views)
    }

    async fn evaluate_prefix(
        &self,
        prefix: &[DispatchedStep],
        next: &PlannedStep,
    ) -> Result<Vec<SequenceMatch>, SequencePolicyError> {
        let rules = self.load_rules().await?;
        let next_idx = prefix.len();
        let mut views: Vec<StepView<'_>> = Vec::with_capacity(next_idx + 1);
        for step in prefix {
            views.push(StepView {
                name: &step.name,
                tool: &step.tool,
                params: &step.parameters,
            });
        }
        views.push(StepView {
            name: &next.name,
            tool: &next.tool,
            params: &next.parameters,
        });

        // Only windows that COMPLETE on the next node are actionable: a window
        // that finished entirely inside the completed prefix was already acted
        // on at the dispatch of its own later step.
        let matches = match_rules(&rules, &views)?;
        Ok(matches
            .into_iter()
            .filter(|m| m.matched_indices.last() == Some(&next_idx))
            .collect())
    }
}

/// Borrowed view of a step's matching-relevant data (name, tool, parameters).
struct StepView<'a> {
    name: &'a str,
    tool: &'a str,
    params: &'a Value,
}

/// Evaluate every rule against the ordered step list, in config order.
/// Deterministic for identical input: rules are scanned in order, and for each
/// rule every qualifying start index is extended to its earliest completion.
fn match_rules(
    rules: &[SequenceRule],
    steps: &[StepView<'_>],
) -> Result<Vec<SequenceMatch>, SequencePolicyError> {
    let mut matches = Vec::new();
    for rule in rules {
        matches.extend(match_rule(rule, steps)?);
    }
    Ok(matches)
}

/// Find every window in `steps` satisfying a single rule.
fn match_rule(
    rule: &SequenceRule,
    steps: &[StepView<'_>],
) -> Result<Vec<SequenceMatch>, SequencePolicyError> {
    // A sequence rule needs at least an ordered pair — single-predicate rules
    // are per-action policy, not sequence policy.
    if rule.steps.len() < 2 || steps.len() < rule.steps.len() {
        return Ok(Vec::new());
    }
    let predicate_count = rule.steps.len();
    let mut matches = Vec::new();

    for start in 0..steps.len() {
        if !predicate_matches(&rule.steps[0], &steps[start])? {
            continue;
        }
        // Greedily extend to the earliest position of each later predicate.
        let mut indices = vec![start];
        let mut current = start;
        let mut complete = true;
        for k in 1..predicate_count {
            let mut found = None;
            for (t, step) in steps.iter().enumerate().skip(current + 1) {
                if predicate_matches(&rule.steps[k], step)? {
                    found = Some(t);
                    break;
                }
            }
            match found {
                Some(t) => {
                    indices.push(t);
                    current = t;
                }
                None => {
                    complete = false;
                    break;
                }
            }
        }
        if !complete {
            continue;
        }

        let span = current - start;
        let in_window = match rule.window {
            // `window = None` → adjacent chain (span == steps.len() - 1).
            None => span == predicate_count - 1,
            Some(max_gap) => span as u32 <= max_gap,
        };
        if in_window {
            matches.push(SequenceMatch {
                rule_id: rule.id.clone(),
                action: rule.action,
                matched_indices: indices,
                later_step: steps[current].name.to_string(),
            });
        }
    }
    Ok(matches)
}

/// Whether a step predicate matches one step view (tool + parameter
/// predicates). An invalid regex in a parameter predicate is a config error
/// (fail closed).
fn predicate_matches(
    predicate: &StepPredicate,
    step: &StepView<'_>,
) -> Result<bool, SequencePolicyError> {
    if !tool_matches(&predicate.tool, step.tool) {
        return Ok(false);
    }
    for pp in &predicate.params {
        match json_pointer_lookup(step.params, &pp.pointer) {
            // The pointed-to parameter is absent → the predicate does not
            // match (non-matching params must not match).
            None => return Ok(false),
            Some(actual) => {
                if !value_matches(actual, pp)? {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

/// Exact or glob (`*` wildcard) tool-name match.
fn tool_matches(pattern: &str, tool: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == tool;
    }
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = tool.chars().collect();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence_policy::domain::{RuleAction, SafetyCaps, SequencePolicyConfig};
    use serde_json::json;

    /// In-memory test repository serving a fixed config / outcome.
    struct StubRepository {
        outcome: Result<Option<SequencePolicyConfig>, SequencePolicyError>,
    }

    #[async_trait]
    impl SequencePolicyRepository for StubRepository {
        async fn load_config(&self) -> Result<Option<SequencePolicyConfig>, SequencePolicyError> {
            self.outcome.clone()
        }
    }

    fn conference_config() -> SequencePolicyConfig {
        SequencePolicyConfig {
            fail_closed: true,
            rules: vec![conference_rule()],
        }
    }

    /// The canonical conference rule — remove(conf-2026) then add(conf-2026),
    /// window 3, promote.
    fn conference_rule() -> SequenceRule {
        SequenceRule {
            id: "registration-remove-then-reassign".to_string(),
            name: "No remove-then-reassign of a full event seat".to_string(),
            description: "conference seat".to_string(),
            steps: vec![
                StepPredicate {
                    tool: "registration_remove".to_string(),
                    params: vec![ParamPredicate {
                        pointer: "/event_id".to_string(),
                        kind: ParamMatchKind::Exact,
                        value: "conf-2026".to_string(),
                    }],
                },
                StepPredicate {
                    tool: "registration_add".to_string(),
                    params: vec![ParamPredicate {
                        pointer: "/event_id".to_string(),
                        kind: ParamMatchKind::Exact,
                        value: "conf-2026".to_string(),
                    }],
                },
            ],
            window: Some(3),
            action: RuleAction::Promote,
        }
    }

    fn planned(name: &str, tool: &str, event_id: &str) -> PlannedStep {
        PlannedStep {
            name: name.to_string(),
            tool: tool.to_string(),
            parameters: json!({ "event_id": event_id }),
        }
    }

    fn dispatched(name: &str, tool: &str, event_id: &str) -> DispatchedStep {
        DispatchedStep {
            name: name.to_string(),
            tool: tool.to_string(),
            parameters: json!({ "event_id": event_id }),
        }
    }

    fn service_with(config: SequencePolicyConfig) -> SequencePolicyServiceImpl {
        SequencePolicyServiceImpl::new(Box::new(StubRepository {
            outcome: Ok(Some(config)),
        }))
    }

    #[tokio::test]
    async fn evaluate_plan_finds_remove_then_add_pair_and_returns_later_step() {
        // AC #5: a runbook containing remove-then-add yields a match whose
        // later step is the add step.
        let svc = service_with(conference_config());
        let runbook = vec![
            planned("registration_remove", "registration_remove", "conf-2026"),
            planned("registration_add", "registration_add", "conf-2026"),
        ];
        let matches = svc.evaluate_plan(&runbook).await.expect("evaluate");
        assert_eq!(matches.len(), 1);
        let m = &matches[0];
        assert_eq!(m.rule_id, "registration-remove-then-reassign");
        assert_eq!(m.later_step, "registration_add");
        assert_eq!(m.matched_indices, vec![0, 1]);
        assert_eq!(m.action, RuleAction::Promote);
    }

    #[tokio::test]
    async fn evaluate_plan_does_not_match_when_param_values_differ() {
        // Removing conf-2025 then adding conf-2026 is NOT the forbidden pair —
        // parameter predicates must not match non-matching params.
        let svc = service_with(conference_config());
        let runbook = vec![
            planned("registration_remove", "registration_remove", "conf-2025"),
            planned("registration_add", "registration_add", "conf-2026"),
        ];
        let matches = svc.evaluate_plan(&runbook).await.expect("evaluate");
        assert!(matches.is_empty());
    }

    #[tokio::test]
    async fn evaluate_plan_matches_within_window_but_not_outside() {
        let svc = service_with(conference_config());
        // Gap of 2 (one step between) ≤ window 3 → matched.
        let within = vec![
            planned("registration_remove", "registration_remove", "conf-2026"),
            planned("intermediate", "audit_log", "x"),
            planned("registration_add", "registration_add", "conf-2026"),
        ];
        assert_eq!(svc.evaluate_plan(&within).await.expect("e").len(), 1);

        // Gap of 4 > window 3 → out of window, no match.
        let outside = vec![
            planned("registration_remove", "registration_remove", "conf-2026"),
            planned("s1", "audit_log", "x"),
            planned("s2", "audit_log", "x"),
            planned("s3", "audit_log", "x"),
            planned("s4", "audit_log", "x"),
            planned("registration_add", "registration_add", "conf-2026"),
        ];
        assert!(svc.evaluate_plan(&outside).await.expect("e").is_empty());
    }

    #[tokio::test]
    async fn adjacent_default_only_matches_consecutive_steps() {
        let config = SequencePolicyConfig {
            fail_closed: true,
            rules: vec![SequenceRule {
                id: "adjacent-pair".to_string(),
                name: "n".to_string(),
                description: "d".to_string(),
                steps: vec![
                    StepPredicate {
                        tool: "step_a".to_string(),
                        params: vec![],
                    },
                    StepPredicate {
                        tool: "step_b".to_string(),
                        params: vec![],
                    },
                ],
                window: None, // adjacent
                action: RuleAction::Deny,
            }],
        };
        let svc = service_with(config);

        // Adjacent A,B → matched.
        let adjacent = vec![planned("a", "step_a", "x"), planned("b", "step_b", "x")];
        let matches = svc.evaluate_plan(&adjacent).await.expect("e");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].action, RuleAction::Deny);
        assert_eq!(matches[0].later_step, "b");

        // A, …, B with a gap → NOT matched (adjacent only).
        let gapped = vec![
            planned("a", "step_a", "x"),
            planned("mid", "other", "x"),
            planned("b", "step_b", "x"),
        ];
        assert!(svc.evaluate_plan(&gapped).await.expect("e").is_empty());
    }

    #[tokio::test]
    async fn missing_config_file_yields_no_matches_without_error() {
        // Fail-open-absent: Ok(None) → no rules → no gating, not an error.
        let svc = SequencePolicyServiceImpl::new(Box::new(StubRepository { outcome: Ok(None) }));
        let runbook = vec![
            planned("registration_remove", "registration_remove", "conf-2026"),
            planned("registration_add", "registration_add", "conf-2026"),
        ];
        let matches = svc.evaluate_plan(&runbook).await.expect("no error");
        assert!(matches.is_empty());
    }

    #[tokio::test]
    async fn corrupt_config_is_propagated_fail_closed() {
        let svc = SequencePolicyServiceImpl::new(Box::new(StubRepository {
            outcome: Err(SequencePolicyError::InvalidConfig("bad toml".to_string())),
        }));
        let runbook = vec![
            planned("registration_remove", "registration_remove", "conf-2026"),
            planned("registration_add", "registration_add", "conf-2026"),
        ];
        let err = svc.evaluate_plan(&runbook).await.expect_err("fail closed");
        assert!(!err.is_retriable());
    }

    #[tokio::test]
    async fn evaluate_prefix_reports_only_windows_completing_on_next() {
        // R3: dynamic plan — remove already dispatched (prefix), add proposed
        // next → the pair completes on `next` → reported.
        let svc = service_with(conference_config());
        let prefix = vec![dispatched(
            "registration_remove",
            "registration_remove",
            "conf-2026",
        )];
        let next = planned("registration_add", "registration_add", "conf-2026");
        let matches = svc.evaluate_prefix(&prefix, &next).await.expect("evaluate");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].later_step, "registration_add");
        assert_eq!(matches[0].matched_indices, vec![0, 1]);

        // An unrelated next node completes nothing → no matches.
        let unrelated = planned("backup", "run_backup", "x");
        let matches = svc
            .evaluate_prefix(&prefix, &unrelated)
            .await
            .expect("evaluate");
        assert!(matches.is_empty());
    }

    #[tokio::test]
    async fn evaluate_prefix_ignores_pairs_already_completed_in_prefix() {
        // Both steps already dispatched — the pair was acted on at the add
        // step's own dispatch; a later unrelated next node is not re-gated.
        let svc = service_with(conference_config());
        let prefix = vec![
            dispatched("registration_remove", "registration_remove", "conf-2026"),
            dispatched("registration_add", "registration_add", "conf-2026"),
        ];
        let next = planned("backup", "run_backup", "x");
        let matches = svc.evaluate_prefix(&prefix, &next).await.expect("evaluate");
        assert!(matches.is_empty());
    }

    #[tokio::test]
    async fn tool_glob_predicates_match() {
        // registration_* matches registration_remove / registration_add.
        let config = SequencePolicyConfig {
            fail_closed: true,
            rules: vec![SequenceRule {
                id: "glob-pair".to_string(),
                name: "n".to_string(),
                description: "d".to_string(),
                steps: vec![
                    StepPredicate {
                        tool: "registration_*".to_string(),
                        params: vec![],
                    },
                    StepPredicate {
                        tool: "registration_*".to_string(),
                        params: vec![],
                    },
                ],
                window: None,
                action: RuleAction::Promote,
            }],
        };
        let svc = service_with(config);
        let runbook = vec![
            planned("remove", "registration_remove", "conf-2026"),
            planned("add", "registration_add", "conf-2026"),
        ];
        let matches = svc.evaluate_plan(&runbook).await.expect("evaluate");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].later_step, "add");
    }

    #[test]
    fn safety_caps_are_exposed_for_validate() {
        // Caps are a load-time concern (fail-closed issue) — ensure the type
        // contract stays constructible for downstream issues.
        let caps = SafetyCaps {
            max_rules_per_file: 100,
            max_steps_per_rule: 8,
            max_window: 5,
            max_regex_predicates_per_file: 8,
        };
        assert!(caps.max_window >= 1);
    }
}
