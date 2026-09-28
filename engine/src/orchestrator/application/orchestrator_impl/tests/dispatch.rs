//! orchestrator_impl tests (split by concern, #917).

use super::*;

/// ADR-016 (GAP-A-30) F1: WITHOUT the `allow_unanchored` opt-in, the same
/// cross-run deny rule is a STRUCTURED refusal
/// (`OrchestratorError::HistoryUnanchoredRefused`) — not a rule denial and
/// not an opaque evaluation failure.
#[tokio::test]
async fn test_r7_unanchored_history_refusal_is_structured() {
    use crate::audit::application::dto::BuildEnvelopeInput;
    use crate::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
    use crate::audit::application::factory::AuditEnvelopeFactory;
    use crate::audit::domain::envelope::{EventStatus, ExecutionEventRef};
    use crate::audit::infrastructure::local_audit_repository::LocalAuditEnvelopeRepository;
    use crate::event_system::application::event_bus_service_impl::EventBusServiceImpl;
    use crate::execution_engine::application::service_impl::{
        ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
    };
    use crate::execution_engine::domain::ParallelExecutorConfig;
    use crate::sequence_policy::application::service_impl::SequencePolicyServiceImpl;
    use crate::sequence_policy::domain::{HistoryPredicate, RuleAction};
    use crate::sequence_policy::infrastructure::EnvelopeHistoryAdapter;
    use serde_json::json;

    let dir = tempfile::tempdir().expect("tempdir");
    let repo: std::sync::Arc<
        dyn crate::audit::infrastructure::repository::AuditEnvelopeRepository,
    > = std::sync::Arc::new(LocalAuditEnvelopeRepository::new(dir.path().to_path_buf()));
    let factory = AuditEnvelopeFactoryImpl::new(Some("test-key".to_string()));
    let run1 = factory
        .build_envelope(BuildEnvelopeInput {
            execution_id: uuid::Uuid::new_v4(),
            template_id: "attendance-remove".to_string(),
            planning_prompt: "run 1".to_string(),
            events: vec![ExecutionEventRef {
                event_type: "node_completed".to_string(),
                summary: "remove completed".to_string(),
                occurred_at: chrono::Utc::now(),
                correlation_id: None,
                status: EventStatus::Success,
                payload: Some(json!({ "step_name": "registration_remove" })),
            }],
            source: Some("rigorix_demo".to_string()),
            total_tokens: 0,
            duration_ms: 10,
            git_commit: None,
            git_branch: None,
            model_version: None,
            planning_prompt_content: None,
            file_paths: vec![],
            metadata: None,
            scoring_results: std::collections::HashMap::new(),
            sign: true,
            repository: None,
            author: Some("jeff@corp".to_string()),
            identity: None,
            effect_key: None,
            producer_id: None,
            sequence: None,
            prev_hash: None,
            history_policy: None,
        })
        .await
        .expect("run-1 envelope");
    repo.save(&run1).await.expect("run-1 envelope saved");

    let policy_cfg = crate::sequence_policy::domain::SequencePolicyConfig {
        fail_closed: true,
        requirements: Vec::new(),
        rules: vec![crate::sequence_policy::domain::SequenceRule {
            id: "no-cross-run-remove-reassign".to_string(),
            name: "No cross-run remove-then-reassign".to_string(),
            description: "d".to_string(),
            steps: vec![crate::sequence_policy::domain::StepPredicate {
                tool: "registration_add".to_string(),
                params: vec![crate::sequence_policy::domain::ParamPredicate {
                    pointer: "/event_id".to_string(),
                    kind: crate::sequence_policy::domain::ParamMatchKind::Exact,
                    value: Some("conf-2026".to_string()),
                    step: None,
                }],
            }],
            window: None,
            action: RuleAction::Deny,
            history: Some(HistoryPredicate {
                prior_node: "registration_remove".to_string(),
                same_principal: true,
                window_secs: 900,
                effect_key: false,
            }),
        }],
    };
    // NO `.with_unanchored_history_allowed(true)` — default fail-closed.
    let policy = std::sync::Arc::new(
        SequencePolicyServiceImpl::new(Box::new(FixedPolicyRepo {
            config: Some(policy_cfg),
        }))
        .with_history(std::sync::Arc::new(EnvelopeHistoryAdapter::new(repo))),
    );
    let executor: Arc<dyn exec_svc::ParallelExecutionService> =
        Arc::new(ParallelExecutionServiceImpl::new(
            ParallelExecutorConfig::default(),
            Box::new(RetryEvaluationServiceImpl::new()),
            Arc::new(EventBusServiceImpl::default()),
        ));
    let orch = OrchestratorServiceImpl::new(
        OrchestratorConfig::default(),
        Arc::new(super::super::super::orchestrator_mocks::MockPlanningService::new()),
        executor,
        Arc::new(super::super::super::orchestrator_mocks::MockStateService::new()),
        Arc::new(super::super::super::orchestrator_mocks::MockCancellationService),
        Arc::new(super::super::super::orchestrator_mocks::MockEventBusService::new()),
        None,
        Arc::new(super::super::super::orchestrator_mocks::MockBudgetService),
        None,
    )
    .with_sequence_policy(policy);

    let step = crate::orchestrator::dto::TemplateStepDef {
        name: "registration_add".to_string(),
        tool: "registration_add".to_string(),
        description: "add jeff".to_string(),
        parameters: json!({ "event_id": "conf-2026" }),
        require_identity: false,
        requires_approval: false,
        timeout_secs: None,
        evaluate_score: false,
    };
    let err = orch
        .run_from_template(RunFromTemplateInput {
            steps: vec![step],
            repo_root: dir.path().to_string_lossy().into_owned(),
            execution_id: Some(uuid::Uuid::new_v4()),
            template_name: "attendance-add".into(),
            repository: None,
            identity: None,
            author: Some("jeff@corp".to_string()),
            enforcement_preset: None,
        })
        .await
        .expect_err("unanchored cross-run denial must refuse (fail closed)");

    match err {
        OrchestratorError::HistoryUnanchoredRefused { rule_id, step } => {
            assert_eq!(rule_id, "no-cross-run-remove-reassign");
            assert_eq!(step, "registration_add");
        }
        other => panic!("expected HistoryUnanchoredRefused, got {other:?}"),
    }
}
