//! Plan-time policy pipeline for `OrchestratorServiceImpl` (extracted from
//! `orchestrator_impl.rs`, #916): the composed R2 sequence-policy → R9
//! operator-requirement gate, run in a fixed fail-closed order.

use super::*;

impl OrchestratorServiceImpl {
    /// L1 identity gate (F-20260907-05): refuse at plan time when ANY step
    /// declares `require_identity = true` and the caller has no attested
    /// identity (no claim, or source = `Unverified`). Fail closed — the step
    /// never dispatches. Runs before the sequence-policy gate so the error
    /// names the missing identity.
    pub(super) fn enforce_identity_requirement(
        &self,
        steps: &[super::super::dto::TemplateStepDef],
        identity: Option<&crate::identity::domain::IdentityRef>,
    ) -> Result<(), OrchestratorError> {
        check_identity_gate(steps, identity)
    }

    /// R2 — plan-time sequence-policy evaluation over an ordered runbook.
    ///
    /// Module spec: the graph-build insertion point — evaluation happens on
    /// the ordered step list **before** `build_graph_from_steps` seals the
    /// graph, so matched later steps are promoted at the same call site that
    /// already applies `step.requires_approval`.
    ///
    /// Returns `(enforced, findings)`:
    /// - `enforced: None` — no service configured, or no rule matched
    ///   (runbook executes with its declared approval flags, unchanged).
    /// - `enforced: Some(steps)` — at least one `promote` rule matched; the
    ///   later matched step(s) have `requires_approval = true` set.
    /// - `findings` — structured `SequencePolicyFinding`s for every matched
    ///   `promote` rule (surfaced by plan preview to MCP `validate_plan`).
    /// - `Err(SequencePolicyDenied)` — a `deny` rule matched: the plan is
    ///   refused before any step executes (fail closed; the denied step's
    ///   tool is never called). Deny wins over promote deterministically.
    /// - `Err(SequencePolicyEvaluationFailed)` — evaluation itself failed
    ///   (corrupt/over-cap config or internal): also fail closed.
    pub(super) async fn apply_plan_time_sequence_policy(
        &self,
        steps: &[super::super::dto::TemplateStepDef],
        execution_id: Option<uuid::Uuid>,
        principal: Option<&str>,
    ) -> Result<
        (
            Option<Vec<super::super::dto::TemplateStepDef>>,
            Vec<super::super::dto::SequencePolicyFinding>,
        ),
        OrchestratorError,
    > {
        let Some(svc) = &self.sequence_policy_service else {
            return Ok((None, Vec::new()));
        };
        let planned: Vec<PlannedStep> = steps
            .iter()
            .map(|s| PlannedStep {
                name: s.name.clone(),
                tool: s.tool.clone(),
                parameters: s.parameters.clone(),
            })
            .collect();
        let matches = svc
            .evaluate_plan(&planned, principal)
            .await
            .map_err(|e| match e {
                crate::sequence_policy::domain::SequencePolicyError::HistoryUnanchored {
                    rule_id,
                    step,
                } => OrchestratorError::HistoryUnanchoredRefused { rule_id, step },
                other => OrchestratorError::SequencePolicyEvaluationFailed {
                    detail: other.to_string(),
                },
            })?;
        if matches.is_empty() {
            return Ok((None, Vec::new()));
        }

        let mut promoted: Vec<String> = Vec::new();
        let mut findings: Vec<super::super::dto::SequencePolicyFinding> = Vec::new();
        for m in &matches {
            match m.action {
                crate::sequence_policy::domain::RuleAction::Deny => {
                    return Err(OrchestratorError::SequencePolicyDenied {
                        later_step: m.later_step.clone(),
                        rule_id: m.rule_id.clone(),
                    });
                }
                crate::sequence_policy::domain::RuleAction::Promote => {
                    promoted.push(m.later_step.clone());
                    // R6 evidence: every plan-time promotion is a first-class
                    // event before anything executes — the envelope derives
                    // `sequence_policy_findings[]` from it. Summary is
                    // pre-redacted (SpanPrivacy — parameter values never
                    // captured). Plan PREVIEW (None execution id) records no
                    // events: the finding data travels in PlanOnlyOutput.
                    if let Some(execution_id) = execution_id {
                        self.event_bus
                            .publish(event_app::PublishEventInput {
                                event: crate::event_system::domain::ExecutionEvent::SequenceRuleMatched {
                                    execution_id,
                                    rule_id: m.rule_id.clone(),
                                    action: "promote".to_string(),
                                    later_step: m.later_step.clone(),
                                    matched_indices: m.matched_indices.clone(),
                                    summary: m.decision_summary(),
                                    timestamp: chrono::Utc::now(),
                                },
                            })
                            .await
                            .map_err(|e| OrchestratorError::Internal {
                                detail: format!("Sequence-policy evidence publish failed: {e}"),
                                source_module: "orchestrator".into(),
                            })?;
                    }
                    findings.push(super::super::dto::SequencePolicyFinding {
                        rule_id: m.rule_id.clone(),
                        later_step: m.later_step.clone(),
                        action: "promote".to_string(),
                    });
                }
            }
        }
        if promoted.is_empty() {
            return Ok((None, findings));
        }
        let mut enforced = steps.to_vec();
        for def in &mut enforced {
            if promoted.iter().any(|name| name == &def.name) {
                def.requires_approval = true;
            }
        }
        Ok((Some(enforced), findings))
    }

    /// R9 / ADR-015 — plan-time operator step-requirement gate.
    ///
    /// Runs after the sequence-policy gate over the (possibly sequence-
    /// enforced) ordered step list, so every plan — including agent-composed
    /// `rigorix_execute` plans — is subject to the operator's obligations
    /// regardless of what the plan declares.
    ///
    /// Returns `(enforced, findings)`:
    /// - `Err(IdentityRequired)` — a matched step's `require_identity`
    ///   obligation is unmet (absent or `Unverified` caller). Never
    ///   promotable.
    /// - `Err(RequirementUnmet)` — a matched step is missing a required
    ///   parameter and the requirement's action is `deny` (default).
    /// - `enforced: Some(steps)` — at least one `promote` requirement matched;
    ///   the matched step(s) have `requires_approval = true` set.
    /// - `findings` — redacted evidence for every fired requirement.
    pub(super) async fn apply_plan_time_requirements(
        &self,
        steps: &[super::super::dto::TemplateStepDef],
        execution_id: Option<uuid::Uuid>,
        identity: Option<&crate::identity::domain::IdentityRef>,
    ) -> Result<
        (
            Option<Vec<super::super::dto::TemplateStepDef>>,
            Vec<super::super::dto::RequirementPolicyFinding>,
        ),
        OrchestratorError,
    > {
        let Some(svc) = &self.sequence_policy_service else {
            return Ok((None, Vec::new()));
        };
        let attested = identity
            .map(|i| {
                !matches!(
                    i.source,
                    crate::identity::domain::IdentitySource::Unverified
                )
            })
            .unwrap_or(false);
        let planned: Vec<PlannedStep> = steps
            .iter()
            .map(|s| PlannedStep {
                name: s.name.clone(),
                tool: s.tool.clone(),
                parameters: s.parameters.clone(),
            })
            .collect();
        let findings = svc
            .evaluate_requirements(&planned, attested)
            .await
            .map_err(|e| OrchestratorError::SequencePolicyEvaluationFailed {
                detail: e.to_string(),
            })?;
        if findings.is_empty() {
            return Ok((None, Vec::new()));
        }

        let status = if identity.is_some() {
            "unverified"
        } else {
            "unauthenticated"
        };
        let mut promoted: Vec<String> = Vec::new();
        let mut out: Vec<super::super::dto::RequirementPolicyFinding> = Vec::new();
        for f in &findings {
            match f.action {
                crate::sequence_policy::domain::RequirementAction::Deny => {
                    // Attestation is never promotable — a human approval cannot
                    // substitute for an identity (ADR-015).
                    if f.unmet_identity {
                        return Err(OrchestratorError::IdentityRequired {
                            step: f.step.clone(),
                            status: status.to_string(),
                        });
                    }
                    return Err(OrchestratorError::RequirementUnmet {
                        requirement_id: f.requirement_id.clone(),
                        step: f.step.clone(),
                        unmet: f.unmet_list(),
                    });
                }
                crate::sequence_policy::domain::RequirementAction::Promote => {
                    promoted.push(f.step.clone());
                    // R9 evidence: promotion is a first-class event. Plan
                    // PREVIEW (None execution id) records no events — the
                    // finding data travels in PlanOnlyOutput.
                    if let Some(execution_id) = execution_id {
                        self.event_bus
                            .publish(event_app::PublishEventInput {
                                event: crate::event_system::domain::ExecutionEvent::RequirementPromoted {
                                    execution_id,
                                    requirement_id: f.requirement_id.clone(),
                                    step: f.step.clone(),
                                    unmet: f.unmet_list(),
                                    action: "promote".to_string(),
                                    summary: f.decision_summary(),
                                    timestamp: chrono::Utc::now(),
                                },
                            })
                            .await
                            .map_err(|e| OrchestratorError::Internal {
                                detail: format!("Requirement evidence publish failed: {e}"),
                                source_module: "orchestrator".into(),
                            })?;
                    }
                    out.push(super::super::dto::RequirementPolicyFinding {
                        requirement_id: f.requirement_id.clone(),
                        step: f.step.clone(),
                        action: "promote".to_string(),
                        unmet: f.unmet_list(),
                    });
                }
            }
        }
        if promoted.is_empty() {
            return Ok((None, out));
        }
        let mut enforced = steps.to_vec();
        for def in &mut enforced {
            if promoted.iter().any(|name| name == &def.name) {
                def.requires_approval = true;
            }
        }
        Ok((Some(enforced), out))
    }
}

/// Composed plan-time gates: sequence policy (R2) → operator requirements
/// (R9), in a fixed **fail-closed** order. Requirements evaluate the
/// sequence-enforced step list, so an agent-authored plan cannot escape the
/// operator's obligations.
pub(super) struct PolicyPipeline<'a> {
    service: &'a OrchestratorServiceImpl,
}

impl<'a> PolicyPipeline<'a> {
    /// Bind the pipeline to the orchestrator it gates.
    pub(super) fn new(service: &'a OrchestratorServiceImpl) -> Self {
        Self { service }
    }

    /// Run the gates in order and return the final step list plus both gates'
    /// findings. Any error propagates (the plan is refused).
    pub(super) async fn apply(
        &self,
        steps: &[super::super::dto::TemplateStepDef],
        execution_id: Option<uuid::Uuid>,
        principal: Option<&str>,
        identity: Option<&crate::identity::domain::IdentityRef>,
    ) -> Result<
        (
            Vec<super::super::dto::TemplateStepDef>,
            Vec<super::super::dto::SequencePolicyFinding>,
            Vec<super::super::dto::RequirementPolicyFinding>,
        ),
        OrchestratorError,
    > {
        let (sequence_enforced, sequence_findings) = self
            .service
            .apply_plan_time_sequence_policy(steps, execution_id, principal)
            .await?;
        let sequence_steps: &[super::super::dto::TemplateStepDef] =
            sequence_enforced.as_deref().unwrap_or(steps);
        let (requirement_enforced, requirement_findings) = self
            .service
            .apply_plan_time_requirements(sequence_steps, execution_id, identity)
            .await?;
        let steps = requirement_enforced
            .or(sequence_enforced)
            .unwrap_or_else(|| steps.to_vec());
        Ok((steps, sequence_findings, requirement_findings))
    }
}
