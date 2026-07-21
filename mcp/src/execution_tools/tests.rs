//! Unit tests for the Execution Tools module.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#tests
//! Implements: EngineFacadeImpl, handler, and repository tests

#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;

    use crate::execution_tools::application::dto::{ExecuteInput, ValidateInput};
    use crate::execution_tools::application::service::{
        CheckEnforcementHandler, ExecuteHandler, PlanHandler, ValidatePlanHandler,
    };
    use crate::execution_tools::application::service_impl::{
        CheckEnforcementHandlerImpl, ExecuteHandlerImpl, PlanHandlerImpl, ValidatePlanHandlerImpl,
    };
    use crate::execution_tools::domain::entity::{EngineFacade, SharedEngineFacade};
    use crate::execution_tools::domain::error::EngineFacadeError;
    use crate::execution_tools::domain::value::{
        BudgetStatus, EnforcementStatus, ExecutionId, ExecutionResult, ExecutionStatus,
        PlanTemplate, StepDefinition, StepResult, ValidationResult,
    };
    use crate::execution_tools::infrastructure::in_memory_repository::InMemoryExecutionRepository;
    use crate::execution_tools::infrastructure::repository::ExecutionRepository;

    // -----------------------------------------------------------------------
    // Mock EngineFacade for testing
    // -----------------------------------------------------------------------

    struct MockEngineFacade {
        execute_result: Option<Result<ExecutionResult, EngineFacadeError>>,
        validate_result: Option<Result<ValidationResult, EngineFacadeError>>,
        enforcement_status: Option<EnforcementStatus>,
    }

    impl MockEngineFacade {
        fn new() -> Self {
            Self {
                execute_result: None,
                validate_result: None,
                enforcement_status: None,
            }
        }

        fn with_execute_ok(mut self) -> Self {
            let steps = vec![StepResult::new(
                "step1".into(),
                true,
                None,
                serde_json::json!({"result": "ok"}),
                100,
            )];
            self.execute_result = Some(Ok(ExecutionResult::new(
                uuid::Uuid::nil(),
                ExecutionStatus::Completed,
                steps,
                100,
                Some(50),
                "rigorix://audit/test".into(),
            )));
            self
        }

        fn with_execute_err(mut self) -> Self {
            self.execute_result = Some(Err(EngineFacadeError::BudgetExceeded {
                tool_calls_remaining: 0,
                tokens_remaining: 0,
            }));
            self
        }

        fn with_validate_ok(mut self) -> Self {
            self.validate_result = Some(Ok(ValidationResult::new(
                true,
                vec!["warning: high cost".into()],
                vec![],
                None,
            )));
            self
        }

        fn with_validate_err(mut self) -> Self {
            self.validate_result = Some(Ok(ValidationResult::new(
                false,
                vec![],
                vec!["Blocked by policy".into()],
                None,
            )));
            self
        }

        fn with_enforcement_active(mut self) -> Self {
            self.enforcement_status = Some(EnforcementStatus::new(
                true,
                "strict".into(),
                BudgetStatus {
                    tool_calls_total: 100,
                    tool_calls_remaining: 50,
                    tokens_total: 10000,
                    tokens_remaining: 5000,
                },
                vec![],
            ));
            self
        }
    }

    #[async_trait]
    impl EngineFacade for MockEngineFacade {
        async fn execute(
            &self,
            _plan: PlanTemplate,
            _repository: Option<String>,
            _author: Option<String>,
        ) -> Result<ExecutionResult, EngineFacadeError> {
            self.execute_result
                .clone()
                .unwrap_or_else(|| Err(EngineFacadeError::EngineNotAvailable("mock".into())))
        }

        async fn validate_plan(
            &self,
            _plan: PlanTemplate,
        ) -> Result<ValidationResult, EngineFacadeError> {
            self.validate_result
                .clone()
                .unwrap_or_else(|| Ok(ValidationResult::new(true, vec![], vec![], None)))
        }

        async fn check_enforcement(&self) -> Result<EnforcementStatus, EngineFacadeError> {
            self.enforcement_status
                .clone()
                .ok_or_else(|| EngineFacadeError::EngineNotAvailable("mock".into()))
        }

        async fn get_execution_cost(
            &self,
            _execution_id: &ExecutionId,
        ) -> Result<crate::execution_tools::domain::value::CostBreakdown, EngineFacadeError>
        {
            Err(EngineFacadeError::ExecutionNotFound(uuid::Uuid::nil()))
        }

        async fn run_template(
            &self,
            _template_name: &str,
            _repository: Option<String>,
            _author: Option<String>,
        ) -> Result<ExecutionResult, EngineFacadeError> {
            self.execute_result
                .clone()
                .unwrap_or_else(|| Err(EngineFacadeError::EngineNotAvailable("mock".into())))
        }
    }

    // -----------------------------------------------------------------------
    // PlanTemplate helper
    // -----------------------------------------------------------------------

    fn make_test_plan() -> PlanTemplate {
        let step = StepDefinition::new(
            "build".into(),
            "bash".into(),
            serde_json::json!({"command": "cargo build"}),
            false,
            "Build the project".into(),
            None,
        );
        PlanTemplate::new(
            "test-plan".into(),
            "A test plan".into(),
            vec![step],
            None,
            HashMap::new(),
        )
        .expect("valid plan")
    }

    // -----------------------------------------------------------------------
    // EngineFacade tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_enginefacade_execute_success() {
        let mock = Arc::new(MockEngineFacade::new().with_execute_ok());
        let result = mock.execute(make_test_plan(), None, None).await;
        assert!(result.is_ok());
        let exec = result.unwrap();
        assert_eq!(*exec.status(), ExecutionStatus::Completed);
        assert!(exec.tokens_used().is_some());
    }

    #[tokio::test]
    async fn test_enginefacade_execute_budget_exceeded() {
        let mock = Arc::new(MockEngineFacade::new().with_execute_err());
        let result = mock.execute(make_test_plan(), None, None).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            EngineFacadeError::BudgetExceeded { .. } => {}
            e => panic!("Expected BudgetExceeded, got: {}", e),
        }
    }

    #[tokio::test]
    async fn test_enginefacade_validate_plan() {
        let mock = Arc::new(MockEngineFacade::new().with_validate_ok());
        let result = mock.validate_plan(make_test_plan()).await;
        assert!(result.is_ok());
        let validation = result.unwrap();
        assert!(validation.is_valid());
        assert!(!validation.warnings().is_empty());
    }

    #[tokio::test]
    async fn test_enginefacade_validate_plan_rejected() {
        let mock = Arc::new(MockEngineFacade::new().with_validate_err());
        let result = mock.validate_plan(make_test_plan()).await;
        assert!(result.is_ok());
        let validation = result.unwrap();
        assert!(!validation.is_valid());
        assert!(!validation.errors().is_empty());
    }

    #[tokio::test]
    async fn test_enginefacade_check_enforcement() {
        let mock = Arc::new(MockEngineFacade::new().with_enforcement_active());
        let status = mock.check_enforcement().await;
        assert!(status.is_ok());
        let s = status.unwrap();
        assert!(s.is_active());
        assert_eq!(s.preset(), "strict");
        assert_eq!(s.budget().tool_calls_remaining, 50);
    }

    // -----------------------------------------------------------------------
    // Repository tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_repository_save_and_find() {
        let repo = InMemoryExecutionRepository::new();
        let repo = Arc::new(repo);

        let result = ExecutionResult::new(
            uuid::Uuid::nil(),
            ExecutionStatus::Completed,
            vec![],
            100,
            None,
            "rigorix://audit/test".into(),
        );
        let exec_id = ExecutionId::from_uuid(uuid::Uuid::nil());

        repo.save_execution(&result).await.unwrap();
        let found = repo.find_execution(&exec_id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().duration_ms(), 100);

        let not_found = repo
            .find_execution(&ExecutionId::from_uuid(uuid::Uuid::new_v4()))
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    // -----------------------------------------------------------------------
    // Handler tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_execute_handler_returns_formatted_result() {
        let engine: SharedEngineFacade = Arc::new(MockEngineFacade::new().with_execute_ok());
        let handler = ExecuteHandlerImpl::new(engine, Duration::from_secs(60));

        let input = ExecuteInput {
            plan: Some(make_test_plan()),
            template_name: None,
            execution_id: None,
            repository: None,
            author: None,
        };

        let result = handler.handle(input).await;
        assert!(result.is_ok());
        let tc = result.unwrap();
        assert!(!tc.is_error);
        assert!(!tc.content.is_empty());
        assert!(tc.content[0].text.contains("Completed"));
    }

    #[tokio::test]
    async fn test_validate_handler_returns_formatted_result() {
        let engine: SharedEngineFacade = Arc::new(MockEngineFacade::new().with_validate_ok());
        let handler = ValidatePlanHandlerImpl::new(engine);

        let input = ValidateInput {
            plan: make_test_plan(),
        };

        let result = handler.handle(input).await;
        assert!(result.is_ok());
        let tc = result.unwrap();
        assert!(!tc.content.is_empty());
    }

    #[tokio::test]
    async fn test_validate_handler_rejected() {
        let engine: SharedEngineFacade = Arc::new(MockEngineFacade::new().with_validate_err());
        let handler = ValidatePlanHandlerImpl::new(engine);

        let input = ValidateInput {
            plan: make_test_plan(),
        };

        let result = handler.handle(input).await;
        assert!(result.is_ok());
        let tc = result.unwrap();
        assert!(tc.is_error);
    }

    #[tokio::test]
    async fn test_check_enforcement_handler_returns_status() {
        let engine: SharedEngineFacade =
            Arc::new(MockEngineFacade::new().with_enforcement_active());
        let handler = CheckEnforcementHandlerImpl::new(engine);

        let result = handler.handle().await;
        assert!(result.is_ok());
        let tc = result.unwrap();
        assert!(!tc.is_error);
        assert!(tc.content[0].text.contains("strict"));
    }

    #[tokio::test]
    async fn test_enginefacade_in_memory_repository_roundtrip() {
        let repo = InMemoryExecutionRepository::new();
        let repo = Arc::new(repo);

        let step = StepResult::new("step1".into(), true, None, serde_json::json!({}), 50);
        let result = ExecutionResult::new(
            uuid::Uuid::new_v4(),
            ExecutionStatus::Completed,
            vec![step],
            150,
            Some(100),
            "rigorix://audit/test".into(),
        );

        let exec_id = ExecutionId::from_uuid(*result.execution_id());
        repo.save_execution(&result).await.unwrap();

        let cost = repo.find_cost_breakdown(&exec_id).await.unwrap();
        assert!(cost.is_some());
        assert_eq!(cost.unwrap().total_tool_calls(), 1);
    }

    #[test]
    fn test_plan_template_validation() {
        // Empty steps should fail
        let result = PlanTemplate::new(
            "empty".into(),
            "No steps".into(),
            vec![],
            None,
            HashMap::new(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_plan_template_from_json() {
        let json = serde_json::json!({
            "name": "test",
            "description": "desc",
            "steps": [{
                "name": "step1",
                "tool": "bash",
                "parameters": {"cmd": "echo hi"},
                "requires_approval": false,
                "description": "Test step"
            }]
        });
        let plan = PlanTemplate::from_json(json);
        assert!(
            plan.is_ok(),
            "PlanTemplate::from_json failed: {:?}",
            plan.err()
        );
        assert_eq!(plan.unwrap().name(), "test");
    }

    #[test]
    fn test_execution_id_display() {
        let id = ExecutionId::from_uuid(uuid::Uuid::nil());
        assert_eq!(id.to_string(), "00000000-0000-0000-0000-000000000000");
    }

    #[test]
    fn test_step_definition_accessors() {
        let step = StepDefinition::new(
            "test".into(),
            "read".into(),
            serde_json::json!({"path": "/tmp"}),
            true,
            "Read a file".into(),
            Some(30),
        );
        assert_eq!(step.name(), "test");
        assert_eq!(step.tool(), "read");
        assert!(step.requires_approval());
        assert_eq!(step.timeout_secs(), Some(30));
    }

    // -------------------------------------------------------------------
    // PlanHandler tests
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn test_plan_handler_returns_dag() {
        let engine: SharedEngineFacade = Arc::new(MockEngineFacade::new().with_validate_ok());
        let handler = PlanHandlerImpl::new(engine);

        // Build a template_tools::PlanTemplate
        let now = chrono::Utc::now();
        let step = crate::template_tools::domain::value::StepDefinition::new(
            "read-file".into(),
            "file_read".into(),
            serde_json::json!({"path": "src/main.rs"}),
            false,
            "Read main.rs".into(),
            None,
        );
        let template = crate::template_tools::domain::value::PlanTemplate::new(
            "test-template".into(),
            "A test plan".into(),
            "1.0.0".into(),
            vec!["test".into()],
            vec![step],
            None,
            HashMap::new(),
            now,
            now,
        )
        .unwrap();

        let result = handler.handle(&template).await;
        assert!(result.is_ok());
        let tc = result.unwrap();
        assert!(!tc.is_error);

        // Parse the JSON output
        let output: serde_json::Value =
            serde_json::from_str(&tc.content[0].text).expect("valid JSON");
        assert_eq!(output["template_name"], "test-template");
        assert_eq!(output["description"], "A test plan");
        assert_eq!(output["version"], "1.0.0");
        assert_eq!(output["graph"]["sealed"], true);
        assert_eq!(output["graph"]["node_count"], 1);
        assert_eq!(output["graph"]["nodes"][0]["name"], "read-file");
        assert_eq!(output["graph"]["nodes"][0]["tool"], "file_read");
        assert_eq!(output["enforcement"]["valid"], true);
    }
}
