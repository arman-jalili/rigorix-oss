//! Engine wiring — constructs a real or stub EngineFacade for the MCP composition root.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#enginefacade-impl
//! Implements: EngineFacade wiring — real rigorix-engine or stub for development
//!
//! By default, returns a `StubEngineFacade` that returns canned responses.
//! Set `RIGORIX_USE_REAL_ENGINE=1` to construct a real EngineFacadeImpl with
//! all required rigorix-engine sub-services.
//!
//! The real engine wiring constructs:
//! - OrchestratorService with PlanningPipeline, DagExecution, StateManager,
//!   CancellationService, EventBus, LlmBudget
//! - ExecutionEnforcer for budget/policy enforcement
//! - ExecutionRepository for persistence

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use rigorix_engine::orchestrator::domain::OrchestratorConfig;

use crate::execution_tools::domain::entity::{EngineFacade, SharedEngineFacade};
use crate::execution_tools::domain::error::EngineFacadeError;
use crate::execution_tools::domain::value::{
    BudgetStatus, CostBreakdown, EnforcementStatus, ExecutionResult, ExecutionStatus, PlanTemplate,
    StepResult, ValidationResult,
};
use crate::execution_tools::infrastructure::{
    EngineFacadeConfig, EngineFacadeImpl, InMemoryExecutionRepository,
};
use crate::execution_tools::infrastructure::repository::ExecutionRepository;

// =========================================================================
// StubEngineFacade — development stub (default)
// =========================================================================

/// Development stub for the rigorix-engine facade.
///
/// Returns deterministic canned responses for all operations.
/// Used by default when `RIGORIX_USE_REAL_ENGINE` is not set.
pub struct StubEngineFacade;

#[async_trait]
impl EngineFacade for StubEngineFacade {
    async fn execute(&self, plan: PlanTemplate) -> Result<ExecutionResult, EngineFacadeError> {
        let step_count = plan.steps().len();
        let steps: Vec<StepResult> = plan
            .steps()
            .iter()
            .map(|s| {
                StepResult::new(
                    s.name().to_string(),
                    true,
                    None,
                    serde_json::json!({"status": "completed"}),
                    100,
                )
            })
            .collect();

        let duration = step_count as u64 * 100;
        let tokens = step_count as u64 * 50;

        Ok(ExecutionResult::new(
            uuid::Uuid::new_v4(),
            ExecutionStatus::Completed,
            steps,
            duration,
            Some(tokens),
            format!("rigorix://audit/{}", uuid::Uuid::new_v4()),
        ))
    }

    async fn validate_plan(
        &self,
        _plan: PlanTemplate,
    ) -> Result<ValidationResult, EngineFacadeError> {
        Ok(ValidationResult::new(true, vec![], vec![], None))
    }

    async fn check_enforcement(&self) -> Result<EnforcementStatus, EngineFacadeError> {
        Ok(EnforcementStatus::new(
            true,
            "default".into(),
            BudgetStatus {
                tool_calls_total: 1000,
                tool_calls_remaining: 750,
                tokens_total: 100000,
                tokens_remaining: 75000,
            },
            vec![],
        ))
    }

    async fn get_execution_cost(
        &self,
        _execution_id: &crate::execution_tools::domain::value::ExecutionId,
    ) -> Result<CostBreakdown, EngineFacadeError> {
        Ok(CostBreakdown::new(
            uuid::Uuid::new_v4(),
            vec![],
            500,
            10,
            None,
        ))
    }
}

// =========================================================================
// Real engine wiring (behind RIGORIX_USE_REAL_ENGINE=1)
// =========================================================================

/// Build an EngineFacade implementation.
///
/// Returns a `StubEngineFacade` by default.
/// When `RIGORIX_USE_REAL_ENGINE=1` is set and all required engine services
/// can be constructed, returns a real `EngineFacadeImpl`.
pub fn build_engine_facade(repo_root: &str) -> SharedEngineFacade {
    if std::env::var("RIGORIX_USE_REAL_ENGINE").as_deref() != Ok("1") {
        tracing::info!("Using StubEngineFacade (set RIGORIX_USE_REAL_ENGINE=1 for real engine)");
        return Arc::new(StubEngineFacade);
    }

    match build_real_engine_facade(repo_root) {
        Ok(facade) => {
            tracing::info!("Using real EngineFacadeImpl with rigorix-engine");
            facade
        }
        Err(e) => {
            tracing::warn!(
                "Failed to build real engine facade (falling back to stub): {}",
                e
            );
            Arc::new(StubEngineFacade)
        }
    }
}

/// Construct a real EngineFacadeImpl with all required engine sub-services.
#[cfg(feature = "real-engine")]
fn build_real_engine_facade(repo_root: &str) -> Result<SharedEngineFacade, Box<dyn std::error::Error>> {
    use rigorix_engine::planning::application::pipeline_factory::PlanningPipelineFactory;
    use rigorix_engine::planning::application::pipeline_factory_impl::PlanningPipelineFactoryImpl;
    use rigorix_engine::planning::application::classifier_impl::ClassifierImpl;
    use rigorix_engine::planning::application::extractor_impl::ParameterExtractorImpl;
    use rigorix_engine::templates::application::service_impl::TemplateEngineServiceImpl;

    // ── Planning pipeline ──
    let classifier = Box::new(ClassifierImpl::new());
    let extractor = Box::new(ParameterExtractorImpl::new());
    let template_service = Arc::new(TemplateEngineServiceImpl::new(
        rigorix_engine::templates::application::dto::TemplateEngineConfig::default(),
    ));
    let planning_factory = PlanningPipelineFactoryImpl;
    let planning_pipeline = planning_factory
        .create_default(classifier, extractor, template_service)
        .await? preparation needed?;

    // ── Execution service ──
    use rigorix_engine::execution_engine::application::factory::ParallelExecutionFactory;
    use rigorix_engine::execution_engine::application::factory_impl::ParallelExecutionFactoryImpl;
    let exec_factory = ParallelExecutionFactoryImpl::new();
    let execution_service = exec_factory.create(/* config */).await?;

    // ── State manager ──
    use rigorix_engine::state_persistence::application::service::StateManagerService;
    use rigorix_engine::state_persistence::application::state_manager_service_impl::FileSystemStateManager;
    use rigorix_engine::state_persistence::infrastructure::graph_repository_impl::GraphRepositoryImpl;
    use rigorix_engine::state_persistence::infrastructure::state_repository_impl::StateRepositoryImpl;
    let state_repo = Box::new(StateRepositoryImpl::new(repo_root));
    let state_manager = Arc::new(FileSystemStateManager::new(state_repo));

    // ── Cancellation service ──
    use rigorix_engine::cancellation::application::CancellationService;
    use rigorix_engine::cancellation::application::cancellation_service_impl::CancellationManagerImpl;
    let cancellation_service = Arc::new(CancellationManagerImpl::default());

    // ── Event bus ──
    use rigorix_engine::event_system::application::EventBusService;
    use rigorix_engine::event_system::application::event_bus_service_impl::EventBusServiceImpl;
    use rigorix_engine::event_system::application::dto::EventBusConfig;
    let event_bus = Arc::new(EventBusServiceImpl::new(EventBusConfig::default()));

    // ── Budget service ──
    use rigorix_engine::budget_tracking::application::LlmBudgetService;
    use rigorix_engine::budget_tracking::application::llm_budget_impl::LlmBudgetImpl;
    let budget_service = Arc::new(LlmBudgetImpl::new(1000, 100_000, "mcp-server".into()));

    // ── Orchestrator ──
    use rigorix_engine::orchestrator::application::builder::OrchestratorBuilder;
    use rigorix_engine::orchestrator::application::builder_impl::OrchestratorBuilderImpl;
    use rigorix_engine::orchestrator::application::service::OrchestratorService;

    let orchestrator = OrchestratorBuilderImpl::new(OrchestratorConfig::default())
        .with_repo_root(repo_root.to_string())
        .with_planning_pipeline(planning_pipeline)
        .with_execution_service(execution_service)
        .with_state_manager(state_manager)
        .with_cancellation_service(Arc::new(cancellation_service))
        .with_event_bus(event_bus)
        .with_budget_service(budget_service)
        .build()
        .await?;

    // ── Execution enforcer ──
    use rigorix_engine::enforcement::application::service::ExecutionEnforcer;
    use rigorix_engine::enforcement::application::enforcer_factory::ExecutionEnforcerFactory;
    use rigorix_engine::enforcement::application::enforcer_factory_impl::ExecutionEnforcerFactoryImpl;
    let enforcer_factory = ExecutionEnforcerFactoryImpl;
    let enforcer = enforcer_factory
        .create_default()
        .await
        .map_err(|e| format!("enforcer: {}", e))?;

    // ── EngineFacadeImpl ──
    let execution_repo: Arc<dyn ExecutionRepository> = Arc::new(InMemoryExecutionRepository::new());

    let engine = EngineFacadeImpl::new(
        orchestrator,
        enforcer,
        execution_repo,
        EngineFacadeConfig {
            execute_timeout: Duration::from_secs(300),
            validate_timeout: Duration::from_secs(60),
            enforcement_enabled: true,
            repo_root: repo_root.to_string(),
        },
    );

    Ok(Arc::new(engine))
}

/// Fallback when real-engine feature is not enabled.
#[cfg(not(feature = "real-engine"))]
fn build_real_engine_facade(repo_root: &str) -> Result<SharedEngineFacade, Box<dyn std::error::Error>> {
    Err("real-engine feature not enabled — compile with --features real-engine".into())
}
