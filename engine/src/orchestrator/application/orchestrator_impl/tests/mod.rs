pub(crate) use super::*;

pub(crate) use crate::sequence_policy::domain::{
    ParamMatchKind, ParamPredicate, RequirementAction, RuleAction, SequencePolicyConfig,
    SequenceRule, StepPredicate, StepRequirement,
};

/// Captures the `BuildEnvelopeInput` the orchestrator sends to audit.
///
/// Used by the identity AC#6 test — verifies `RunInput.identity` flows
/// into the envelope identity block (redacted).
struct CapturingAuditService {
    captured: Arc<std::sync::Mutex<Option<audit_app::BuildEnvelopeInput>>>,
}

#[async_trait]
impl audit_app::AuditService for CapturingAuditService {
    async fn build_and_send(
        &self,
        input: audit_app::BuildEnvelopeInput,
    ) -> Result<audit_app::BuildEnvelopeOutput, crate::audit::domain::AuditError> {
        *self.captured.lock().unwrap() = Some(input);
        Ok(audit_app::BuildEnvelopeOutput {
            envelope: crate::audit::domain::AuditEnvelope {
                execution_id: Uuid::new_v4(),
                timestamp: chrono::Utc::now(),
                template_id: "captured".into(),
                planning_hash: "hash".into(),
                source: None,
                repository: None,
                author: None,
                identity: None,
                effect_key: None,
                total_tokens: 0,
                duration_ms: 0,
                git_commit: None,
                git_branch: None,
                model_version: None,
                planning_prompt: None,
                file_paths: vec![],
                events: vec![],
                scoring_results: std::collections::HashMap::new(),
                approval_events: Vec::new(),
                sequence_policy_findings: Vec::new(),
                requirement_findings: Vec::new(),
                scope_violations: Vec::new(),
                decision_context_ref: None,
                signature: None,
                evidence_degraded: false,
                producer_id: None,
                sequence: None,
                prev_hash: None,
                history_integrity: None,
                anchor_head: None,
                history_policy: None,
            },
            signed: false,
            event_count: 0,
        })
    }

    async fn retry_pending(
        &self,
    ) -> Result<audit_app::RetryPendingOutput, crate::audit::domain::AuditError> {
        Ok(audit_app::RetryPendingOutput {
            delivered: 0,
            still_pending: 0,
            dropped: 0,
        })
    }

    async fn status(
        &self,
    ) -> Result<audit_app::AuditStatusOutput, crate::audit::domain::AuditError> {
        Ok(audit_app::AuditStatusOutput {
            pending_count: 0,
            circuit_breaker_state: crate::audit::domain::CircuitBreakerState::Closed,
            backend_available: false,
        })
    }
}

/// In-memory sequence-policy repository serving a fixed rule config.
struct FixedPolicyRepo {
    config: Option<crate::sequence_policy::domain::SequencePolicyConfig>,
}

#[async_trait]
impl crate::sequence_policy::infrastructure::repository::SequencePolicyRepository
    for FixedPolicyRepo
{
    async fn load_config(
        &self,
    ) -> Result<
        Option<crate::sequence_policy::domain::SequencePolicyConfig>,
        crate::sequence_policy::domain::SequencePolicyError,
    > {
        Ok(self.config.clone())
    }
}

/// The canonical conference rule with the given action (module spec
/// §Configuration): remove(conf-2026) → add(conf-2026), window 3.
fn conference_policy(action: RuleAction) -> SequencePolicyConfig {
    let param = || ParamPredicate {
        pointer: "/event_id".to_string(),
        kind: ParamMatchKind::Exact,
        value: Some("conf-2026".to_string()),
        step: None,
    };
    SequencePolicyConfig {
        fail_closed: true,
        requirements: Vec::new(),
        rules: vec![SequenceRule {
            id: "registration-remove-then-reassign".to_string(),
            name: "No remove-then-reassign of a full event seat".to_string(),
            description: "conference seat".to_string(),
            steps: vec![
                StepPredicate {
                    tool: "registration_remove".to_string(),
                    params: vec![param()],
                },
                StepPredicate {
                    tool: "registration_add".to_string(),
                    params: vec![param()],
                },
            ],
            window: Some(3),
            action,
            history: None,
        }],
    }
}

/// Orchestrator with a REAL execution engine + REAL sequence-policy
/// service evaluating the canonical conference rule.
fn real_orchestrator_with_conference_policy(action: RuleAction) -> OrchestratorServiceImpl {
    use crate::event_system::application::event_bus_service_impl::EventBusServiceImpl;
    use crate::execution_engine::application::service_impl::{
        ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
    };
    use crate::execution_engine::domain::ParallelExecutorConfig;
    use crate::sequence_policy::application::service_impl::SequencePolicyServiceImpl;

    let executor: Arc<dyn exec_svc::ParallelExecutionService> =
        Arc::new(ParallelExecutionServiceImpl::new(
            ParallelExecutorConfig::default(),
            Box::new(RetryEvaluationServiceImpl::new()),
            Arc::new(EventBusServiceImpl::default()),
        ));
    let policy = Arc::new(SequencePolicyServiceImpl::new(Box::new(FixedPolicyRepo {
        config: Some(conference_policy(action)),
    })));
    OrchestratorServiceImpl::new(
        OrchestratorConfig::default(),
        Arc::new(super::super::orchestrator_mocks::MockPlanningService::new()),
        executor,
        Arc::new(super::super::orchestrator_mocks::MockStateService::new()),
        Arc::new(super::super::orchestrator_mocks::MockCancellationService),
        Arc::new(super::super::orchestrator_mocks::MockEventBusService::new()),
        None,
        Arc::new(super::super::orchestrator_mocks::MockBudgetService),
        None,
    )
    .with_sequence_policy(policy)
}

/// The remove-then-add runbook: BOTH steps declare `requires_approval =
/// false` — without the promote rule nothing would pause.
fn conference_runbook() -> Vec<crate::orchestrator::dto::TemplateStepDef> {
    let step = |name: &str| crate::orchestrator::dto::TemplateStepDef {
        name: name.to_string(),
        tool: name.to_string(),
        description: name.to_string(),
        parameters: serde_json::json!({ "event_id": "conf-2026" }),
        require_identity: false,
        requires_approval: false,
        timeout_secs: None,
        evaluate_score: false,
    };
    vec![step("registration_remove"), step("registration_add")]
}

/// R9 (ADR-015): a requirement over a `run_command` reaching the payout
/// script — must be attested and carry canonical effect parameters.
fn payout_requirement(action: RequirementAction) -> SequencePolicyConfig {
    SequencePolicyConfig {
        fail_closed: true,
        requirements: vec![StepRequirement {
            id: "payout-guard".to_string(),
            name: "Payout commands must be attested and carry canonical effect data"
                .to_string(),
            description: "raw run_command must not reach the payout script".to_string(),
            r#match: StepPredicate {
                tool: "run_command".to_string(),
                params: vec![ParamPredicate {
                    pointer: "/command".to_string(),
                    kind: ParamMatchKind::Glob,
                    value: Some("*execute_payout.sh*".to_string()),
                    step: None,
                }],
            },
            require_identity: true,
            require_params: vec!["/beneficiary".to_string(), "/effect_key".to_string()],
            action,
        }],
        rules: Vec::new(),
    }
}

/// The composed-plan bypass this requirement closes: a raw `run_command`
/// that invokes the payout script, optionally carrying the parameters the
/// plan is supposed to declare.
fn payout_runbook(include_params: bool) -> Vec<crate::orchestrator::dto::TemplateStepDef> {
    let mut parameters = serde_json::json!({
        "command": "bash .rigorix/scripts/execute_payout.sh acct 100"
    });
    if include_params {
        parameters["beneficiary"] = serde_json::json!("acct-1");
        parameters["effect_key"] = serde_json::json!("eff-1");
    }
    vec![crate::orchestrator::dto::TemplateStepDef {
        name: "payout".to_string(),
        tool: "run_command".to_string(),
        description: "payout".to_string(),
        parameters,
        requires_approval: false,
        require_identity: false,
        timeout_secs: None,
        evaluate_score: false,
    }]
}

/// Orchestrator with a REAL execution engine + a sequence-policy service
/// evaluating the given R9 requirement config.
fn real_orchestrator_with_requirement(config: SequencePolicyConfig) -> OrchestratorServiceImpl {
    use crate::event_system::application::event_bus_service_impl::EventBusServiceImpl;
    use crate::execution_engine::application::service_impl::{
        ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
    };
    use crate::execution_engine::domain::ParallelExecutorConfig;
    use crate::sequence_policy::application::service_impl::SequencePolicyServiceImpl;

    let executor: Arc<dyn exec_svc::ParallelExecutionService> =
        Arc::new(ParallelExecutionServiceImpl::new(
            ParallelExecutorConfig::default(),
            Box::new(RetryEvaluationServiceImpl::new()),
            Arc::new(EventBusServiceImpl::default()),
        ));
    let policy = Arc::new(SequencePolicyServiceImpl::new(Box::new(FixedPolicyRepo {
        config: Some(config),
    })));
    OrchestratorServiceImpl::new(
        OrchestratorConfig::default(),
        Arc::new(super::super::orchestrator_mocks::MockPlanningService::new()),
        executor,
        Arc::new(super::super::orchestrator_mocks::MockStateService::new()),
        Arc::new(super::super::orchestrator_mocks::MockCancellationService),
        Arc::new(super::super::orchestrator_mocks::MockEventBusService::new()),
        None,
        Arc::new(super::super::orchestrator_mocks::MockBudgetService),
        None,
    )
    .with_sequence_policy(policy)
}

fn payout_input(
    orphan: bool,
    include_params: bool,
    identity: Option<crate::identity::domain::IdentityRef>,
) -> RunFromTemplateInput {
    RunFromTemplateInput {
        steps: payout_runbook(include_params),
        repo_root: "/tmp/t".into(),
        execution_id: if orphan { None } else { Some(Uuid::new_v4()) },
        template_name: "payout".into(),
        repository: None,
        identity,
        author: None,
        enforcement_preset: None,
    }
}

mod policies;
mod run;
mod plan;
mod approval;
mod dispatch;
