//! TDD integration tests for execution-tools.
//!
//! These tests fulfill the pre-generated TDD test contracts from
//! `tests/unit/execution-tools/`. They verify that all components
//! are properly defined, constructed, and interact correctly.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use rigorix_mcp::execution_tools::application::dto::{ExecuteInput, ValidateInput};
use rigorix_mcp::execution_tools::application::service::{
    CheckEnforcementHandler, ExecuteHandler, ValidatePlanHandler,
};
use rigorix_mcp::execution_tools::application::service_impl::{
    CheckEnforcementHandlerImpl, ExecuteHandlerImpl, ValidatePlanHandlerImpl,
};
use rigorix_mcp::execution_tools::domain::entity::{EngineFacade, SharedEngineFacade};
use rigorix_mcp::execution_tools::domain::error::EngineFacadeError;
use rigorix_mcp::execution_tools::domain::value::{
    BudgetStatus, EnforcementStatus, ExecutionId, ExecutionResult, ExecutionStatus, PlanTemplate,
    StepDefinition, StepResult, ValidationResult,
};
use rigorix_mcp::execution_tools::infrastructure::in_memory_repository::InMemoryExecutionRepository;
use rigorix_mcp::execution_tools::infrastructure::repository::ExecutionRepository;

// -----------------------------------------------------------------------
// Shared mock EngineFacade for all TDD tests
// -----------------------------------------------------------------------

struct TddMockEngine {
    execute_result: Option<Result<ExecutionResult, EngineFacadeError>>,
    validate_result: Option<Result<ValidationResult, EngineFacadeError>>,
    enforcement_status: Option<EnforcementStatus>,
}

impl TddMockEngine {
    fn with_defaults() -> Self {
        Self {
            execute_result: Some(Ok(ExecutionResult::new(
                uuid::Uuid::nil(),
                ExecutionStatus::Completed,
                vec![StepResult::new(
                    "step1".into(),
                    true,
                    None,
                    serde_json::json!({}),
                    50,
                )],
                100,
                Some(50),
                "rigorix://audit/test".into(),
            ))),
            validate_result: Some(Ok(ValidationResult::new(true, vec![], vec![], None))),
            enforcement_status: Some(EnforcementStatus::new(
                true,
                "default".into(),
                BudgetStatus {
                    tool_calls_total: 1000,
                    tool_calls_remaining: 750,
                    tokens_total: 100000,
                    tokens_remaining: 75000,
                },
                vec![],
            )),
        }
    }
}

#[async_trait]
impl EngineFacade for TddMockEngine {
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
        execution_id: &ExecutionId,
    ) -> Result<rigorix_mcp::execution_tools::domain::value::CostBreakdown, EngineFacadeError> {
        Err(EngineFacadeError::ExecutionNotFound(
            *execution_id.as_uuid(),
        ))
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

    async fn approve_execution(
        &self,
        execution_id: &ExecutionId,
        step_names: Vec<String>,
        _identity: Option<rigorix_mcp::execution_tools::domain::value::ApprovalIdentity>,
    ) -> Result<rigorix_mcp::execution_tools::domain::value::ApprovalResult, EngineFacadeError>
    {
        Ok(
            rigorix_mcp::execution_tools::domain::value::ApprovalResult::new(
                *execution_id.as_uuid(),
                step_names,
                vec![],
                vec![],
                true,
            ),
        )
    }

    async fn execution_state(
        &self,
        _: &ExecutionId,
    ) -> Result<rigorix_mcp::execution_tools::domain::value::ExecutionStateInfo, EngineFacadeError>
    {
        unimplemented!("mock execution_state not needed by current tests")
    }
}

fn make_plan() -> PlanTemplate {
    PlanTemplate::new(
        "tdd-plan".into(),
        "TDD test plan".into(),
        vec![StepDefinition::new(
            "build".into(),
            "bash".into(),
            serde_json::json!({"cmd": "echo ok"}),
            false,
            "Build".into(),
            None,
        )],
        None,
        HashMap::new(),
    )
    .expect("valid plan")
}

// -----------------------------------------------------------------------
// TDD: EngineFacade is defined
// -----------------------------------------------------------------------

#[test]
fn test_enginefacade_is_defined() {
    // EngineFacade is a trait — verify it can be implemented
    let engine: Arc<dyn EngineFacade> = Arc::new(TddMockEngine::with_defaults());
    let _instance: SharedEngineFacade = engine;
    // If this compiles, EngineFacade trait is properly defined
}

#[tokio::test]
async fn test_enginefacade_executes_plan() {
    let engine = Arc::new(TddMockEngine::with_defaults());
    let result = engine.execute(make_plan(), None, None).await;
    assert!(result.is_ok(), "EngineFacade should execute a plan");
    let exec = result.unwrap();
    assert_eq!(*exec.status(), ExecutionStatus::Completed);
    assert_eq!(exec.duration_ms(), 100);
}

#[tokio::test]
async fn test_enginefacade_validates_plan() {
    let engine = Arc::new(TddMockEngine::with_defaults());
    let result = engine.validate_plan(make_plan()).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_valid());
}

#[tokio::test]
async fn test_enginefacade_checks_enforcement() {
    let engine = Arc::new(TddMockEngine::with_defaults());
    let result = engine.check_enforcement().await;
    assert!(result.is_ok());
    let status = result.unwrap();
    assert!(status.is_active());
    assert_eq!(status.budget().tool_calls_remaining, 750);
}

// -----------------------------------------------------------------------
// TDD: ExecuteHandler is defined
// -----------------------------------------------------------------------

#[test]
fn test_executehandler_is_defined() {
    let engine: SharedEngineFacade = Arc::new(TddMockEngine::with_defaults());
    let handler = ExecuteHandlerImpl::new(engine, Duration::from_secs(60));
    assert_eq!(handler.timeout_duration(), Duration::from_secs(60));
}

#[tokio::test]
async fn test_executehandler_handles_execution() {
    let engine: SharedEngineFacade = Arc::new(TddMockEngine::with_defaults());
    let handler = ExecuteHandlerImpl::new(engine, Duration::from_secs(60));

    let result = handler
        .handle(ExecuteInput {
            plan: Some(make_plan()),
            template_name: None,
            execution_id: None,
            repository: None,
            author: None,
        })
        .await;

    assert!(result.is_ok(), "ExecuteHandler should handle execution");
    let tc = result.unwrap();
    assert!(!tc.is_error);
    assert!(!tc.content.is_empty());
    assert!(tc.content[0].text.contains("Completed"));
}

// -----------------------------------------------------------------------
// TDD: ValidatePlanHandler is defined
// -----------------------------------------------------------------------

#[test]
fn test_validateplanhandler_is_defined() {
    let engine: SharedEngineFacade = Arc::new(TddMockEngine::with_defaults());
    let _handler = ValidatePlanHandlerImpl::new(engine);
}

#[tokio::test]
async fn test_validateplanhandler_handles_validation() {
    let engine: SharedEngineFacade = Arc::new(TddMockEngine::with_defaults());
    let handler = ValidatePlanHandlerImpl::new(engine);

    let result = handler.handle(ValidateInput { plan: make_plan() }).await;

    assert!(result.is_ok());
    let tc = result.unwrap();
    assert!(!tc.is_error);
}

// -----------------------------------------------------------------------
// TDD: CheckEnforcementHandler is defined
// -----------------------------------------------------------------------

#[test]
fn test_checkenforcementhandler_is_defined() {
    let engine: SharedEngineFacade = Arc::new(TddMockEngine::with_defaults());
    let _handler = CheckEnforcementHandlerImpl::new(engine);
}

#[tokio::test]
async fn test_checkenforcementhandler_checks_enforcement() {
    let engine: SharedEngineFacade = Arc::new(TddMockEngine::with_defaults());
    let handler = CheckEnforcementHandlerImpl::new(engine);

    let result = handler.handle().await;
    assert!(result.is_ok());
    let tc = result.unwrap();
    assert!(!tc.is_error);
    assert!(tc.content[0].text.contains("default"));
}

// -----------------------------------------------------------------------
// TDD: InMemoryExecutionRepository is defined and works
// -----------------------------------------------------------------------

#[tokio::test]
async fn test_repository_saves_and_retrieves() {
    let repo = InMemoryExecutionRepository::new();
    let repo: Arc<dyn ExecutionRepository> = Arc::new(repo);

    let exec_id = ExecutionId::from_uuid(uuid::Uuid::nil());
    let result = ExecutionResult::new(
        uuid::Uuid::nil(),
        ExecutionStatus::Completed,
        vec![],
        200,
        None,
        "rigorix://audit/test".into(),
    );

    repo.save_execution(&result).await.unwrap();
    let found = repo.find_execution(&exec_id).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().duration_ms(), 200);
}

// -----------------------------------------------------------------------
// TDD: Error types are properly constructable
// -----------------------------------------------------------------------

#[test]
fn test_enginefacade_error_constructs() {
    let err = EngineFacadeError::BudgetExceeded {
        tool_calls_remaining: 0,
        tokens_remaining: 0,
    };
    assert!(err.to_string().contains("Budget exceeded"));

    let err = EngineFacadeError::Timeout {
        operation: "test".into(),
        duration_secs: 30,
    };
    assert!(err.to_string().contains("timed out"));
}

#[test]
fn test_plan_template_validates() {
    // Empty steps should be rejected
    let result = PlanTemplate::new("empty".into(), "desc".into(), vec![], None, HashMap::new());
    assert!(result.is_err());

    // Valid plan should be accepted
    let result = PlanTemplate::new(
        "valid".into(),
        "desc".into(),
        vec![StepDefinition::new(
            "step1".into(),
            "tool".into(),
            serde_json::json!({}),
            false,
            "desc".into(),
            None,
        )],
        None,
        HashMap::new(),
    );
    assert!(result.is_ok());
}
