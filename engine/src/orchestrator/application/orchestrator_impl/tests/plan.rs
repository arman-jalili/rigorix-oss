//! orchestrator_impl tests (split by concern, #917).

use super::*;

#[tokio::test]
async fn test_plan_only() {
    let orch = OrchestratorServiceImpl::default_test();
    assert!(
        orch.plan_only(PlanOnlyInput {
            intent: "test".into(),
            config: serde_json::json!({}),
            repo_root: "/tmp/t".into(),
        })
        .await
        .is_ok()
    );
}

#[tokio::test]
async fn test_planning_failure() {
    struct FailPlan;
    #[async_trait]
    impl crate::planning::application::PlanningPipelineService for FailPlan {
        async fn plan(
            &self,
            _: planning_dto::PlanInput,
        ) -> Result<planning_dto::PlanOutput, crate::planning::domain::PlanningError>
        {
            Err(crate::planning::domain::PlanningError::NoMatchingTemplate {
                intent_preview: "test".into(),
                templates_evaluated: 0,
            })
        }
        async fn plan_with_graph(
            &self,
            _: planning_dto::PlanWithGraphInput,
        ) -> Result<planning_dto::PlanWithGraphOutput, crate::planning::domain::PlanningError>
        {
            Err(crate::planning::domain::PlanningError::NoMatchingTemplate {
                intent_preview: "test".into(),
                templates_evaluated: 0,
            })
        }
        async fn check_budget(
            &self,
            _: planning_dto::CheckBudgetInput,
        ) -> Result<planning_dto::CheckBudgetOutput, crate::planning::domain::PlanningError>
        {
            unimplemented!()
        }
        async fn classify_intent(
            &self,
            _: crate::planning::domain::intent::UserIntent,
        ) -> Result<
            crate::planning::domain::classification::ClassificationResult,
            crate::planning::domain::PlanningError,
        > {
            unimplemented!()
        }
        async fn extract_parameters(
            &self,
            _: planning_dto::ExtractParametersInput,
        ) -> Result<planning_dto::ExtractParametersOutput, crate::planning::domain::PlanningError>
        {
            unimplemented!()
        }
        async fn generate_graph(
            &self,
            _: planning_dto::GenerateGraphInput,
        ) -> Result<planning_dto::GenerateGraphOutput, crate::planning::domain::PlanningError>
        {
            unimplemented!()
        }
        async fn validate_plan(
            &self,
            _: planning_dto::ValidatePlanInput,
        ) -> Result<planning_dto::ValidatePlanOutput, crate::planning::domain::PlanningError>
        {
            unimplemented!()
        }
        async fn request_clarification(
            &self,
            _: planning_dto::RequestClarificationInput,
        ) -> Result<
            planning_dto::RequestClarificationOutput,
            crate::planning::domain::PlanningError,
        > {
            unimplemented!()
        }
        async fn available_templates(
            &self,
        ) -> Result<
            planning_dto::AvailableTemplatesOutput,
            crate::planning::domain::PlanningError,
        > {
            unimplemented!()
        }
        fn execution_id(&self) -> Uuid {
            Uuid::new_v4()
        }
    }
    let orch = OrchestratorServiceImpl::new(
        OrchestratorConfig::default(),
        Arc::new(FailPlan),
        Arc::new(super::super::super::orchestrator_mocks::MockExecutionService),
        Arc::new(super::super::super::orchestrator_mocks::MockStateService::new()),
        Arc::new(super::super::super::orchestrator_mocks::MockCancellationService),
        Arc::new(super::super::super::orchestrator_mocks::MockEventBusService::new()),
        None,
        Arc::new(super::super::super::orchestrator_mocks::MockBudgetService),
        None,
    );
    let e = orch
        .run(RunInput {
            intent: "test".into(),
            config: serde_json::json!({}),
            repo_root: "/tmp/t".into(),
            author: None,
            identity: None,
            repository: None,
            enforcement_preset: None,
        })
        .await
        .unwrap_err();
    match e {
        OrchestratorError::PlanningFailed { detail, intent } => {
            assert!(detail.contains("No matching template"));
            assert_eq!(intent, "test");
        }
        _ => panic!("expected PlanningFailed"),
    }
}

#[test]
fn test_build_graph_from_steps_chains_sequentially() {
    // Frozen contract (template-tools value.rs): "Step order is significant".
    // Steps must execute as a sequential runbook, not a parallel batch — a
    // migration template (validate → backup → migrate → verify) would
    // otherwise race the destructive step ahead of the backup.
    let orch = OrchestratorServiceImpl::default_test();
    let steps = [
        crate::orchestrator::application::dto::TemplateStepDef {
            name: "validate".into(),
            tool: "bash".into(),
            description: "validate".into(),
            parameters: serde_json::json!({}),
            require_identity: false,
            requires_approval: false,
            timeout_secs: None,
            evaluate_score: false,
        },
        crate::orchestrator::application::dto::TemplateStepDef {
            name: "backup".into(),
            tool: "bash".into(),
            description: "backup".into(),
            parameters: serde_json::json!({}),
            require_identity: false,
            requires_approval: false,
            timeout_secs: None,
            evaluate_score: false,
        },
        crate::orchestrator::application::dto::TemplateStepDef {
            name: "migrate".into(),
            tool: "bash".into(),
            description: "migrate".into(),
            parameters: serde_json::json!({}),
            require_identity: false,
            requires_approval: true,
            timeout_secs: None,
            evaluate_score: false,
        },
    ];
    let graph = orch.build_graph_from_steps(&steps).unwrap();
    let nodes: Vec<_> = graph.nodes().collect();
    assert_eq!(nodes.len(), 3, "all three steps must be in the graph");

    // First step has no dependency; every later step depends on its
    // immediate predecessor (and transitively on all earlier steps).
    assert!(
        nodes[0].dependencies.is_empty(),
        "first step must have no dependencies"
    );
    assert_eq!(
        nodes[1].dependencies,
        vec![nodes[0].id],
        "step 2 must depend on step 1"
    );
    assert_eq!(
        nodes[2].dependencies,
        vec![nodes[1].id],
        "step 3 must depend on step 2"
    );

    // Approval flag survives the graph construction.
    assert!(!nodes[0].requires_approval());
    assert!(!nodes[1].requires_approval());
    assert!(
        nodes[2].requires_approval(),
        "requires_approval must propagate to the graph node"
    );
}
