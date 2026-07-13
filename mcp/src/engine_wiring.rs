//! Engine wiring — constructs the real EngineFacade for the MCP composition root.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#enginefacade-impl
//! Implements: EngineFacade wiring — always uses the real rigorix-engine
//!
//! Constructs a real EngineFacadeImpl with all required rigorix-engine sub-services:
//! OrchestratorService, ExecutionEnforcer, and ExecutionRepository.

use std::sync::Arc;
use std::time::Duration;

use rigorix_engine::orchestrator::domain::OrchestratorConfig;

use crate::execution_tools::domain::entity::SharedEngineFacade;
use crate::execution_tools::infrastructure::{
    EngineFacadeConfig, EngineFacadeImpl, InMemoryExecutionRepository,
};
use crate::execution_tools::infrastructure::repository::ExecutionRepository;

/// Build a fully-wired EngineFacade implementation.
///
/// Always uses the real rigorix-engine (no stub fallback).
pub fn build_engine_facade(repo_root: &str) -> SharedEngineFacade {
    match build_real_engine_facade(repo_root) {
        Ok(facade) => {
            tracing::info!("EngineFacadeImpl initialized with rigorix-engine");
            facade
        }
        Err(e) => {
            tracing::error!("Failed to build engine facade: {}", e);
            panic!("EngineFacade construction failed — cannot proceed without engine: {e}");
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
    use rigorix_engine::state_persistence::infrastructure::filesystem_state_repository::FileSystemStateRepository;
    let state_dir = std::path::PathBuf::from(repo_root).join(".rigorix").join("state");
    let state_repo = Box::new(FileSystemStateRepository::new(state_dir).await?);
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
        .with_cancellation_service(cancellation_service)
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
        Arc::from(orchestrator),
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
fn build_real_engine_facade(_repo_root: &str) -> Result<SharedEngineFacade, Box<dyn std::error::Error>> {
    Err("real-engine feature not enabled — compile with --features real-engine".into())
}
