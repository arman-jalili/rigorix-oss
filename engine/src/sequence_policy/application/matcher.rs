//! Matcher — the deterministic windowed sequence matcher.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#r1--declarative-sequence-rules
//! Implements: ISSUE-SEQUENCE-POLICY-5 — windowed matching (AC #3) + determinism
//!   property (AC #4)
//! Issue: #843 (Matcher)
//!
//! Pure matching core consumed by `SequencePolicyServiceImpl` for both
//! plan-time (R2, `evaluate_plan`) and run-time prefix (R3,
//! `evaluate_prefix`) evaluation. It is crate-internal — the module's public
//! API stays the `SequencePolicyService` trait (contract freeze).
//!
//! # Semantics
//! - A rule with ordered predicates `[A, B, …]` matches when A appears before
//!   B before … in the evaluated ordered step list
//! - `window = None` → **adjacent** only: the matched chain occupies
//!   `steps.len()` consecutive indices
//! - `window = Some(gap)` → the span between the first and last matched index
//!   must be `≤ gap`; steps in between are allowed
//! - Windows are found by earliest extension from each qualifying start index,
//!   rules scanned in config order — **deterministic**: the same ordered plan
//!   and rules always produce the same match set (R4)

use serde_json::Value;

use crate::sequence_policy::domain::{
    SequenceMatch, SequencePolicyError, SequenceRule, StepPredicate,
};

use super::dto::{DispatchedStep, PlannedStep};

/// Uniform access to a step's matching-relevant data. Implemented for the
/// boundary DTOs (`PlannedStep`, `DispatchedStep`) and the borrowed
/// `StepView`, so plan-time and prefix-time evaluation share one matcher.
pub(crate) trait StepLike {
    /// Step name — the step identity `SequenceMatch::later_step` returns.
    fn step_name(&self) -> &str;
    /// The tool/action this step dispatches.
    fn step_tool(&self) -> &str;
    /// Full JSON parameter object of the step.
    fn step_parameters(&self) -> &Value;
}

impl StepLike for PlannedStep {
    fn step_name(&self) -> &str {
        &self.name
    }
    fn step_tool(&self) -> &str {
        &self.tool
    }
    fn step_parameters(&self) -> &Value {
        &self.parameters
    }
}

impl StepLike for DispatchedStep {
    fn step_name(&self) -> &str {
        &self.name
    }
    fn step_tool(&self) -> &str {
        &self.tool
    }
    fn step_parameters(&self) -> &Value {
        &self.parameters
    }
}

/// Borrowed step view (used to evaluate a heterogeneous prefix + next list
/// as one ordered list without copying).
pub(crate) struct StepView<'a> {
    /// Borrowed step name.
    pub name: &'a str,
    /// Borrowed tool name.
    pub tool: &'a str,
    /// Borrowed parameter object.
    pub params: &'a Value,
}

impl StepLike for StepView<'_> {
    fn step_name(&self) -> &str {
        self.name
    }
    fn step_tool(&self) -> &str {
        self.tool
    }
    fn step_parameters(&self) -> &Value {
        self.params
    }
}

/// Deterministic windowed sequence matcher.
pub(crate) struct Matcher;

impl Matcher {
    /// Construct the matcher (stateless — one shared instance suffices).
    pub(crate) fn new() -> Self {
        Self
    }

    /// Evaluate every rule against the ordered step list, in config order.
    /// Deterministic for identical input.
    pub(crate) fn find_matches<S: StepLike>(
        &self,
        rules: &[SequenceRule],
        steps: &[S],
    ) -> Result<Vec<SequenceMatch>, SequencePolicyError> {
        let mut matches = Vec::new();
        for rule in rules {
            matches.extend(find_matches_for_rule(rule, steps)?);
        }
        Ok(matches)
    }
}

/// Find every window in `steps` satisfying a single rule.
fn find_matches_for_rule<S: StepLike>(
    rule: &SequenceRule,
    steps: &[S],
) -> Result<Vec<SequenceMatch>, SequencePolicyError> {
    // A sequence rule needs at least an ordered pair — single-predicate rules
    // are per-action policy, not sequence policy. EXCEPTION (R7): a rule with
    // a `history` predicate matches the current run's action AND the signed
    // prior-execution trail — the cross-run case ("remove X" in run 1,
    // "add Jeff" in run 2) is a single current-run action gated by history.
    if rule.steps.is_empty()
        || steps.is_empty()
        || steps.len() < rule.steps.len()
        || (rule.steps.len() < 2 && rule.history.is_none())
    {
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
                later_step: steps[current].step_name().to_string(),
            });
        }
    }
    Ok(matches)
}

/// Whether a step predicate matches one step. Delegates the tool + parameter
/// matching to the domain component (`StepPredicate::matches`).
fn predicate_matches<S: StepLike>(
    predicate: &StepPredicate,
    step: &S,
) -> Result<bool, SequencePolicyError> {
    predicate.matches(step.step_tool(), step.step_parameters())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence_policy::domain::RuleAction;
    use serde_json::json;

    fn tool_predicate(tool: &str) -> StepPredicate {
        StepPredicate {
            tool: tool.to_string(),
            params: vec![],
        }
    }

    fn rule(id: &str, tools: &[&str], window: Option<u32>, action: RuleAction) -> SequenceRule {
        SequenceRule {
            id: id.to_string(),
            name: format!("rule {id}"),
            description: "test".to_string(),
            steps: tools.iter().map(|t| tool_predicate(t)).collect(),
            window,
            action,
            history: None,
        }
    }

    fn plan(tools: &[&str]) -> Vec<PlannedStep> {
        tools
            .iter()
            .enumerate()
            .map(|(i, tool)| PlannedStep {
                name: format!("{tool}-{i}"),
                tool: tool.to_string(),
                parameters: json!({}),
            })
            .collect()
    }

    fn match_on(rule: &SequenceRule, tools: &[&str]) -> Vec<SequenceMatch> {
        let matcher = Matcher::new();
        let steps = plan(tools);
        matcher
            .find_matches(std::slice::from_ref(rule), &steps)
            .expect("match")
    }

    #[test]
    fn adjacent_pair_matches_consecutive_steps_only() {
        // AC #3 — adjacent: A immediately followed by B.
        let rule = rule("adj", &["a", "b"], None, RuleAction::Promote);
        let matches = match_on(&rule, &["a", "b"]);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].matched_indices, vec![0, 1]);
        assert_eq!(matches[0].later_step, "b-1");

        // A … gap … B is NOT an adjacent match.
        assert!(match_on(&rule, &["a", "x", "b"]).is_empty());
        assert!(match_on(&rule, &["a", "x", "x", "b"]).is_empty());
    }

    #[test]
    fn windowed_match_allows_gap_up_to_window() {
        // AC #3 — windowed: gap ≤ window matches.
        let rule = rule("win", &["a", "b"], Some(3), RuleAction::Promote);
        // Adjacent (gap 1) matches.
        assert_eq!(match_on(&rule, &["a", "b"]).len(), 1);
        // Gap 2 (one step between) matches.
        let m = match_on(&rule, &["a", "x", "b"]);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].matched_indices, vec![0, 2]);
        // Gap 3 (two between) matches — span == window boundary.
        assert_eq!(match_on(&rule, &["a", "x", "x", "b"]).len(), 1);
        // Gap 4 > window 3 → out of window, no match.
        assert!(match_on(&rule, &["a", "x", "x", "x", "b"]).is_empty());
    }

    #[test]
    fn window_does_not_allow_reordering() {
        // Order is part of the contract: B before A never matches A → B.
        let rule = rule("order", &["a", "b"], Some(5), RuleAction::Promote);
        assert!(match_on(&rule, &["b", "a"]).is_empty());
    }

    #[test]
    fn longer_chain_matches_within_window() {
        // A → B → C windowed chain (gap between A and C ≤ window).
        let rule = rule("chain", &["a", "b", "c"], Some(4), RuleAction::Deny);
        let m = match_on(&rule, &["a", "x", "b", "y", "c"]);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].matched_indices, vec![0, 2, 4]);
        assert_eq!(m[0].later_step, "c-4");
        assert_eq!(m[0].action, RuleAction::Deny);

        // Span too wide → no match.
        assert!(match_on(&rule, &["a", "x", "b", "x", "x", "c"]).is_empty());
    }

    #[test]
    fn adjacent_chain_requires_consecutive_indices() {
        let rule = rule("adj-chain", &["a", "b", "c"], None, RuleAction::Promote);
        assert_eq!(match_on(&rule, &["a", "b", "c"]).len(), 1);
        assert!(match_on(&rule, &["a", "b", "x", "c"]).is_empty());
    }

    #[test]
    fn multiple_matches_reported_in_deterministic_order() {
        // Two disjoint pair occurrences produce two matches in plan order.
        let rule = rule("twice", &["a", "b"], None, RuleAction::Promote);
        let matches = match_on(&rule, &["a", "b", "x", "a", "b"]);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].matched_indices, vec![0, 1]);
        assert_eq!(matches[1].matched_indices, vec![3, 4]);
    }

    #[test]
    fn overlapping_windows_do_not_double_report_the_same_later_step_pair() {
        // Greedy earliest extension from each start: [A,B,B] reports the
        // adjacent pair at 0,1 — and start 1 (B) cannot open a new pair.
        let rule = rule("overlap", &["a", "b"], None, RuleAction::Promote);
        let matches = match_on(&rule, &["a", "b", "b"]);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].matched_indices, vec![0, 1]);
    }

    // ── Determinism property (AC #4) ───────────────────────────────────────

    /// A small corpus of rule × ordered-plan pairs. The property: identical
    /// input always yields the identical match set.
    fn determinism_corpus() -> Vec<(Vec<SequenceRule>, Vec<&'static str>)> {
        vec![
            // Adjacent pair.
            (
                vec![rule("adj", &["a", "b"], None, RuleAction::Promote)],
                vec!["a", "b"],
            ),
            // Windowed pair within and beyond bounds.
            (
                vec![rule("win", &["a", "b"], Some(3), RuleAction::Promote)],
                vec!["a", "x", "b"],
            ),
            (
                vec![rule("win", &["a", "b"], Some(3), RuleAction::Promote)],
                vec!["a", "x", "x", "x", "b"],
            ),
            // Two rules, several occurrences, promote + deny mix.
            (
                vec![
                    rule("r1", &["a", "b"], None, RuleAction::Promote),
                    rule("r2", &["b", "c"], Some(2), RuleAction::Deny),
                ],
                vec!["a", "b", "c", "a", "b"],
            ),
            // Chain rule.
            (
                vec![rule("chain", &["a", "b", "c"], Some(4), RuleAction::Deny)],
                vec!["a", "x", "b", "y", "c"],
            ),
            // No match at all.
            (
                vec![rule("none", &["a", "b"], None, RuleAction::Promote)],
                vec!["z", "z"],
            ),
        ]
    }

    #[test]
    fn same_ordered_plan_and_rules_produce_the_same_match_set_every_time() {
        // AC #4 — determinism property: repeated evaluation of identical input
        // yields an identical, fully-equal match set.
        let matcher = Matcher::new();
        for (rules, tools) in determinism_corpus() {
            let steps = plan(&tools);
            for _ in 0..50 {
                let first = matcher.find_matches(&rules, &steps).expect("match");
                let again = matcher.find_matches(&rules, &steps).expect("match");
                assert_eq!(first, again, "rule set {rules:?} over plan {tools:?}");
            }
        }
    }

    #[test]
    fn match_set_is_order_stable_rule_config_then_plan_order() {
        // Determinism includes output ORDER: matches follow rule config order,
        // then plan (start-index) order — never hash/iteration randomness.
        let matcher = Matcher::new();
        let rules = vec![
            rule("r2", &["a", "b"], Some(3), RuleAction::Deny),
            rule("r1", &["a", "b"], None, RuleAction::Promote),
        ];
        let steps = plan(&["a", "b"]);
        let matches = matcher.find_matches(&rules, &steps).expect("match");
        let rule_ids: Vec<&str> = matches.iter().map(|m| m.rule_id.as_str()).collect();
        assert_eq!(rule_ids, vec!["r2", "r1"], "config order wins");
    }
}
