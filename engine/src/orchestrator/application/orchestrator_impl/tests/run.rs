//! orchestrator_impl tests (split by concern, #917).

use super::*;

/// ADR-014 (R8): a step's domain-supplied `/effect_key` is recorded on the
/// signed envelope, so effect-keyed history rules can compare it across
/// runs (entity resolution happens outside rigorix).
#[tokio::test]
async fn test_run_from_template_records_effect_key_on_envelope() {
    let captured = Arc::new(std::sync::Mutex::new(None));
    let orch = OrchestratorServiceImpl::default_test().with_audit_service(Arc::new(
        CapturingAuditService {
            captured: captured.clone(),
        },
    ));

    let step = crate::orchestrator::dto::TemplateStepDef {
        name: "payout".to_string(),
        tool: "run_command".to_string(),
        description: "pay the vendor".to_string(),
        parameters: serde_json::json!({ "command": "true", "effect_key": "benef-acme" }),
        require_identity: false,
        requires_approval: false,
        timeout_secs: None,
        evaluate_score: false,
    };
    orch.run_from_template(RunFromTemplateInput {
        steps: vec![step],
        repo_root: "/tmp/t".into(),
        execution_id: Some(uuid::Uuid::new_v4()),
        template_name: "payout".into(),
        repository: None,
        identity: None,
        author: Some("user@org".to_string()),
        enforcement_preset: None,
    })
    .await
    .expect("run_from_template should succeed");

    let input = captured
        .lock()
        .unwrap()
        .take()
        .expect("audit service must receive the built envelope input");
    assert_eq!(
        input.effect_key.as_deref(),
        Some("benef-acme"),
        "the step's /effect_key must be recorded on the envelope"
    );
}

#[tokio::test]
async fn test_cancel() {
    let orch = OrchestratorServiceImpl::default_test();
    let out = orch
        .cancel(CancelInput {
            execution_id: Uuid::new_v4(),
            reason: Some("test".into()),
        })
        .await
        .unwrap();
    assert!(out.aborted);
}

#[tokio::test]
async fn test_run_from_template_refused_when_budget_exhausted() {
    // Budget-halt (spike's second half): a tight budget must refuse the
    // runbook BEFORE execution — deterministically, not halfway through a
    // consequential operation.
    use crate::budget_tracking::application::llm_budget_impl::LlmBudgetImpl;
    use crate::event_system::application::event_bus_service_impl::EventBusServiceImpl;
    use crate::execution_engine::application::service_impl::{
        ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
    };
    use crate::execution_engine::domain::ParallelExecutorConfig;

    let executor: Arc<dyn exec_svc::ParallelExecutionService> =
        Arc::new(ParallelExecutionServiceImpl::new(
            ParallelExecutorConfig::default(),
            Box::new(RetryEvaluationServiceImpl::new()),
            Arc::new(EventBusServiceImpl::default()),
        ));
    // Budget allows only 2 calls; the runbook needs 3.
    let budget: Arc<dyn budget_app::LlmBudgetService> =
        Arc::new(LlmBudgetImpl::new(2, 1000, "test".into()));
    let orch = OrchestratorServiceImpl::new(
        OrchestratorConfig::default(),
        Arc::new(super::super::super::orchestrator_mocks::MockPlanningService::new()),
        executor,
        Arc::new(super::super::super::orchestrator_mocks::MockStateService::new()),
        Arc::new(super::super::super::orchestrator_mocks::MockCancellationService),
        Arc::new(super::super::super::orchestrator_mocks::MockEventBusService::new()),
        None,
        budget,
        None,
    );

    let step_def = |name: &str| crate::orchestrator::application::dto::TemplateStepDef {
        name: name.into(),
        tool: "bash".into(),
        description: name.into(),
        parameters: serde_json::json!({}),
        require_identity: false,
        requires_approval: false,
        timeout_secs: None,
        evaluate_score: false,
    };
    let input = RunFromTemplateInput {
        steps: vec![step_def("a"), step_def("b"), step_def("c")],
        repo_root: "/tmp/t".into(),
        execution_id: None,
        template_name: "budget-test".into(),
        repository: None,
        identity: None,
        author: None,
        enforcement_preset: None,
    };

    let err = orch.run_from_template(input).await.unwrap_err();
    match err {
        OrchestratorError::Internal { detail, .. } => {
            assert!(
                detail.contains("Budget exhausted"),
                "expected budget-exhausted refusal, got: {detail}"
            );
            assert!(detail.contains("3 steps"), "should name the runbook size");
            assert!(detail.contains("2 of 2 calls"), "should name the budget");
        }
        e => panic!("expected Internal budget error, got {e:?}"),
    }
}
