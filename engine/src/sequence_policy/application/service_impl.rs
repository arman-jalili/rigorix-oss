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
//! - Matching is delegated to the crate-internal `Matcher`
//!   (application/matcher.rs), which the Matcher component issue tests
//!   directly (window semantics + determinism property)

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::sequence_policy::domain::{
    SequenceMatch, SequencePolicyError, SequenceRule, rule::tool_matches,
};
use crate::sequence_policy::infrastructure::ExecutionHistory;
use crate::sequence_policy::infrastructure::repository::SequencePolicyRepository;

use super::dto::{DispatchedStep, PlannedStep};
use super::matcher::{Matcher, StepView};
use super::service::SequencePolicyService;

/// Default `SequencePolicyService` implementation.
///
/// # Construction
/// - `new(repository)` — inject the rule-config repository (filesystem-backed
///   `TomlSequencePolicyRepository`, or a signed-bundle seam for enterprise,
///   P3)
/// - `.with_history(...)` — R7: inject the signed-execution-history port
///   (envelope-backed). Omitted (`None`) = history rules never match —
///   status quo, no cross-run gating.
pub struct SequencePolicyServiceImpl {
    repository: Box<dyn SequencePolicyRepository>,
    matcher: Matcher,
    history: Option<std::sync::Arc<dyn ExecutionHistory>>,
}

impl SequencePolicyServiceImpl {
    /// Create the service over the given rule-config repository.
    pub fn new(repository: Box<dyn SequencePolicyRepository>) -> Self {
        Self {
            repository,
            matcher: Matcher::new(),
            history: None,
        }
    }

    /// R7: attach the signed-execution-history port (envelope-backed).
    pub fn with_history(mut self, history: std::sync::Arc<dyn ExecutionHistory>) -> Self {
        self.history = Some(history);
        self
    }

    /// Load the rule config for this run. `Ok(None)` (no config file) is
    /// mapped to an empty rule set — fail-open-absent.
    async fn load_rules(&self) -> Result<Vec<SequenceRule>, SequencePolicyError> {
        match self.repository.load_config().await? {
            Some(config) => {
                tracing::debug!(
                    rules = config.rules.len(),
                    fail_closed = config.fail_closed,
                    "sequence_policy: rule config loaded"
                );
                Ok(config.rules)
            }
            None => {
                tracing::debug!("sequence_policy: no rule file — fail-open-absent, no gating");
                Ok(Vec::new())
            }
        }
    }

    /// R7: drop within-run matches whose rule carries a `history` predicate
    /// that the signed prior-execution history does NOT satisfy.
    ///
    /// A rule with `history` fires only when the within-run `steps[]` match
    /// AND a prior action (glob-matching `prior_node`, by the same principal
    /// when required, within `window_secs`) exists in the history. History is
    /// read once per evaluation over the widest required window. A missing
    /// history port means history rules never match (status quo). A read
    /// failure fails closed.
    async fn filter_matches_by_history(
        &self,
        rules: &[SequenceRule],
        matches: Vec<SequenceMatch>,
        principal: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<Vec<SequenceMatch>, SequencePolicyError> {
        // Only matches whose rule carries a history predicate need checking.
        let has_history = |rule_id: &str| -> bool {
            rules
                .iter()
                .find(|r| r.id == rule_id)
                .and_then(|r| r.history.as_ref())
                .is_some()
        };
        let needs_history_count = matches.iter().filter(|m| has_history(&m.rule_id)).count();
        if needs_history_count == 0 {
            return Ok(matches);
        }
        let Some(history) = &self.history else {
            // No history port wired — history rules cannot match (status quo).
            tracing::warn!(
                "sequence_policy: rule with history predicate matched within-run but no history \
                 port is wired — treating as no-match"
            );
            return Ok(matches
                .into_iter()
                .filter(|m| !has_history(&m.rule_id))
                .collect());
        };

        // Widest required window across the rules that matched.
        let widest = matches
            .iter()
            .filter(|m| has_history(&m.rule_id))
            .filter_map(|m| {
                rules
                    .iter()
                    .find(|r| r.id == m.rule_id)
                    .and_then(|r| r.history.as_ref())
                    .map(|h| h.window_secs)
            })
            .max()
            .unwrap_or(0);
        let since = now - chrono::Duration::seconds(widest as i64);
        let prior = history.prior_actions(since).await?;

        let kept = matches
            .into_iter()
            .filter(|m| {
                let Some(rule) = rules.iter().find(|r| r.id == m.rule_id) else {
                    return false;
                };
                let Some(h) = &rule.history else {
                    return true; // within-run only — unaffected by history
                };
                let cutoff = now - chrono::Duration::seconds(h.window_secs as i64);
                prior.iter().any(|a| {
                    a.at >= cutoff
                        && tool_matches(&h.prior_node, &a.node)
                        && (!h.same_principal
                            || (principal.is_some() && a.principal.as_deref() == principal))
                })
            })
            .collect::<Vec<_>>();
        if kept.len() < needs_history_count {
            tracing::info!(
                prior = prior.len(),
                "sequence_policy: cross-run history predicate filtered a within-run match"
            );
        }
        Ok(kept)
    }
}

#[async_trait]
impl SequencePolicyService for SequencePolicyServiceImpl {
    async fn evaluate_plan(
        &self,
        steps: &[PlannedStep],
        principal: Option<&str>,
    ) -> Result<Vec<SequenceMatch>, SequencePolicyError> {
        let rules = self.load_rules().await?;
        let matches = self.matcher.find_matches(&rules, steps)?;
        let matches = self
            .filter_matches_by_history(&rules, matches, principal, Utc::now())
            .await?;
        if !matches.is_empty() {
            tracing::info!(
                rules = rules.len(),
                steps = steps.len(),
                matches = matches.len(),
                "sequence_policy: plan-time matched sequence(s) — later steps gated"
            );
        }
        Ok(matches)
    }

    async fn evaluate_prefix(
        &self,
        prefix: &[DispatchedStep],
        next: &PlannedStep,
        principal: Option<&str>,
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
        let matches = self.matcher.find_matches(&rules, &views)?;
        let matches = self
            .filter_matches_by_history(&rules, matches, principal, Utc::now())
            .await?;
        let actionable: Vec<SequenceMatch> = matches
            .into_iter()
            .filter(|m| m.matched_indices.last() == Some(&next_idx))
            .collect();
        if !actionable.is_empty() {
            tracing::info!(
                rules = rules.len(),
                prefix_len = prefix.len(),
                matches = actionable.len(),
                "sequence_policy: dispatch-boundary matched sequence(s) — node gated"
            );
        }
        Ok(actionable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence_policy::domain::{
        HistoryAction, HistoryPredicate, ParamMatchKind, ParamPredicate, RuleAction, SafetyCaps,
        SequencePolicyConfig, SequenceRule, StepPredicate,
    };
    use crate::sequence_policy::infrastructure::ExecutionHistory;
    use chrono::{DateTime, Utc};
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
            history: None,
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

    /// R7 fake history port — in-memory prior actions.
    struct FakeHistory {
        actions: std::sync::Mutex<Vec<HistoryAction>>,
    }
    #[async_trait::async_trait]
    impl ExecutionHistory for FakeHistory {
        async fn prior_actions(
            &self,
            _since: DateTime<Utc>,
        ) -> Result<Vec<HistoryAction>, SequencePolicyError> {
            Ok(self.actions.lock().unwrap().clone())
        }
    }

    fn history_rule() -> SequenceRule {
        // remove(conf-2026) in a PRIOR run + add(conf-2026) now, same
        // principal, within 15 min — the cross-run Jeff case (R7).
        SequenceRule {
            id: "no-cross-run-remove-reassign".to_string(),
            name: "No cross-run remove-then-reassign".to_string(),
            description: "d".to_string(),
            steps: vec![StepPredicate {
                tool: "registration_add".to_string(),
                params: vec![ParamPredicate {
                    pointer: "/event_id".to_string(),
                    kind: ParamMatchKind::Exact,
                    value: "conf-2026".to_string(),
                }],
            }],
            window: None,
            action: RuleAction::Deny,
            history: Some(HistoryPredicate {
                prior_node: "registration_remove".to_string(),
                same_principal: true,
                window_secs: 900,
            }),
        }
    }

    fn history_config() -> SequencePolicyConfig {
        SequencePolicyConfig {
            fail_closed: true,
            rules: vec![history_rule()],
        }
    }

    fn history_svc(actions: Vec<HistoryAction>) -> SequencePolicyServiceImpl {
        SequencePolicyServiceImpl::new(Box::new(StubRepository {
            outcome: Ok(Some(history_config())),
        }))
        .with_history(std::sync::Arc::new(FakeHistory {
            actions: std::sync::Mutex::new(actions),
        }))
    }

    fn history_at(secs_ago: i64) -> HistoryAction {
        HistoryAction {
            node: "registration_remove".to_string(),
            principal: Some("jeff@corp".to_string()),
            at: chrono::Utc::now() - chrono::Duration::seconds(secs_ago),
        }
    }

    /// R7 scene 9: run 1 removed (in the signed history) by the SAME
    /// principal → run 2's add is DENIED at plan time.
    #[tokio::test]
    async fn cross_run_same_principal_prior_remove_denies_add() {
        let svc = history_svc(vec![history_at(120)]);
        let runbook = vec![planned("add", "registration_add", "conf-2026")];
        let matches = svc
            .evaluate_plan(&runbook, Some("jeff@corp"))
            .await
            .expect("evaluate");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].rule_id, "no-cross-run-remove-reassign");
        assert_eq!(matches[0].action, RuleAction::Deny);
    }

    /// Different principal's prior remove does NOT deny this run.
    #[tokio::test]
    async fn cross_run_other_principal_prior_remove_does_not_deny() {
        let svc = history_svc(vec![history_at(120)]);
        let runbook = vec![planned("add", "registration_add", "conf-2026")];
        let matches = svc
            .evaluate_plan(&runbook, Some("organizer@corp"))
            .await
            .expect("evaluate");
        assert!(
            matches.is_empty(),
            "same_principal rule must not match another principal"
        );
    }

    /// Prior remove OUTSIDE the window does not deny.
    #[tokio::test]
    async fn cross_run_out_of_window_prior_remove_does_not_deny() {
        let svc = history_svc(vec![history_at(3600)]);
        let runbook = vec![planned("add", "registration_add", "conf-2026")];
        let matches = svc
            .evaluate_plan(&runbook, Some("jeff@corp"))
            .await
            .expect("evaluate");
        assert!(matches.is_empty(), "stale prior action must not deny");
    }

    /// No history port wired → history rules never match (status quo).
    #[tokio::test]
    async fn cross_run_without_history_port_never_denies() {
        let svc = service_with(history_config());
        let runbook = vec![planned("add", "registration_add", "conf-2026")];
        let matches = svc
            .evaluate_plan(&runbook, Some("jeff@corp"))
            .await
            .expect("evaluate");
        assert!(matches.is_empty(), "no history port ⇒ no cross-run gating");
    }

    /// No principal on the run + same_principal rule → no match (never a
    /// false denial).
    #[tokio::test]
    async fn cross_run_unknown_principal_never_denies() {
        let svc = history_svc(vec![history_at(120)]);
        let runbook = vec![planned("add", "registration_add", "conf-2026")];
        let matches = svc.evaluate_plan(&runbook, None).await.expect("evaluate");
        assert!(
            matches.is_empty(),
            "unknown principal ⇒ no same-principal match"
        );
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
        let matches = svc.evaluate_plan(&runbook, None).await.expect("evaluate");
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
        let matches = svc.evaluate_plan(&runbook, None).await.expect("evaluate");
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
        assert_eq!(svc.evaluate_plan(&within, None).await.expect("e").len(), 1);

        // Gap of 4 > window 3 → out of window, no match.
        let outside = vec![
            planned("registration_remove", "registration_remove", "conf-2026"),
            planned("s1", "audit_log", "x"),
            planned("s2", "audit_log", "x"),
            planned("s3", "audit_log", "x"),
            planned("s4", "audit_log", "x"),
            planned("registration_add", "registration_add", "conf-2026"),
        ];
        assert!(
            svc.evaluate_plan(&outside, None)
                .await
                .expect("e")
                .is_empty()
        );
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
                history: None,
            }],
        };
        let svc = service_with(config);

        // Adjacent A,B → matched.
        let adjacent = vec![planned("a", "step_a", "x"), planned("b", "step_b", "x")];
        let matches = svc.evaluate_plan(&adjacent, None).await.expect("e");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].action, RuleAction::Deny);
        assert_eq!(matches[0].later_step, "b");

        // A, …, B with a gap → NOT matched (adjacent only).
        let gapped = vec![
            planned("a", "step_a", "x"),
            planned("mid", "other", "x"),
            planned("b", "step_b", "x"),
        ];
        assert!(
            svc.evaluate_plan(&gapped, None)
                .await
                .expect("e")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn missing_config_file_yields_no_matches_without_error() {
        // Fail-open-absent: Ok(None) → no rules → no gating, not an error.
        let svc = SequencePolicyServiceImpl::new(Box::new(StubRepository { outcome: Ok(None) }));
        let runbook = vec![
            planned("registration_remove", "registration_remove", "conf-2026"),
            planned("registration_add", "registration_add", "conf-2026"),
        ];
        let matches = svc.evaluate_plan(&runbook, None).await.expect("no error");
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
        let err = svc
            .evaluate_plan(&runbook, None)
            .await
            .expect_err("fail closed");
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
        let matches = svc
            .evaluate_prefix(&prefix, &next, None)
            .await
            .expect("evaluate");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].later_step, "registration_add");
        assert_eq!(matches[0].matched_indices, vec![0, 1]);

        // An unrelated next node completes nothing → no matches.
        let unrelated = planned("backup", "run_backup", "x");
        let matches = svc
            .evaluate_prefix(&prefix, &unrelated, None)
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
        let matches = svc
            .evaluate_prefix(&prefix, &next, None)
            .await
            .expect("evaluate");
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
                history: None,
            }],
        };
        let svc = service_with(config);
        let runbook = vec![
            planned("remove", "registration_remove", "conf-2026"),
            planned("add", "registration_add", "conf-2026"),
        ];
        let matches = svc.evaluate_plan(&runbook, None).await.expect("evaluate");
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
            max_history_window_secs: 604_800,
        };
        assert!(caps.max_window >= 1);
    }
}
