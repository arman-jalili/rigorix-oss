//! Orchestrator builder — fully wires the engine's OrchestratorService for the CLI.
//! @canonical .pi/architecture/modules/cli-boundary.md#orchestrator

use std::path::PathBuf;
use std::sync::Arc;

use rigorix_engine::audit::application::audit_queue_impl::AuditQueueImpl;
use rigorix_engine::audit::application::audit_sender_impl::AuditSenderImpl;
use rigorix_engine::audit::application::audit_service_impl::AuditServiceImpl;
use rigorix_engine::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use rigorix_engine::audit::application::service::{AuditQueue, AuditSender};
use rigorix_engine::budget_tracking::application::factory::LlmBudgetFactory;
use rigorix_engine::budget_tracking::application::llm_budget_factory_impl::LlmBudgetFactoryImpl;
use rigorix_engine::cancellation::application::cancellation_manager_factory_impl::CancellationManagerFactoryImpl;
use rigorix_engine::cancellation::application::factory::CancellationManagerFactory;
use rigorix_engine::configuration::domain::config::LlmProvider;
use rigorix_engine::event_system::application::event_bus_factory_impl::EventBusFactoryImpl;
use rigorix_engine::event_system::application::factory::EventBusFactory;
use rigorix_engine::execution_engine::application::factory::{
    ParallelExecutionFactory, ParallelExecutionFactoryConfig,
};
use rigorix_engine::execution_engine::application::factory_impl::ParallelExecutionFactoryImpl;
use rigorix_engine::execution_engine::domain::{ParallelExecutorConfig, RetryPolicy};
use rigorix_engine::orchestrator::application::builder::OrchestratorBuilder;
use rigorix_engine::orchestrator::application::builder_impl::OrchestratorBuilderImpl;
use rigorix_engine::orchestrator::application::service::OrchestratorService;
use rigorix_engine::orchestrator::domain::OrchestratorConfig as OrchestratorDomainConfig;
use rigorix_engine::planning::application::factory::PlanningPipelineFactory;
use rigorix_engine::planning::application::mock_extractor::MockParameterExtractor;
use rigorix_engine::planning::application::pipeline_factory_impl::PlanningPipelineFactoryImpl;
use rigorix_engine::planning::infrastructure::claude_classifier::{
    ClaudeClassifier, ClaudeClassifierConfig,
};
use rigorix_engine::planning::infrastructure::openai_classifier::{
    OpenaiClassifier, OpenaiClassifierConfig,
};
use rigorix_engine::state_persistence::application::factory::{
    CreateStateManagerConfig, StateManagerFactory,
};
use rigorix_engine::state_persistence::application::state_manager_factory_impl::FileSystemStateManagerFactory;
use rigorix_engine::templates::application::service::TemplateEngineService;
use rigorix_engine::templates::application::template_engine_impl::TemplateEngineImpl;

use crate::cli_boundary::config::CliConfig;
use crate::cli_boundary::error::CliError;

/// Build a fully wired `OrchestratorService` from CLI configuration.
///
/// Wires all 6 required engine sub-services + optional audit:
/// 1. CancellationService  — via CancellationManagerFactoryImpl
/// 2. EventBusService      — via EventBusFactoryImpl
/// 3. StateManagerService  — via FileSystemStateManagerFactory
/// 4. LlmBudgetService     — via LlmBudgetFactoryImpl
/// 5. ParallelExecutionService — via ParallelExecutionFactoryImpl
/// 6. PlanningPipelineService   — via PlanningPipelineFactoryImpl (needs LLM key)
/// 7. AuditService         — via AuditServiceImpl (optional)
///
/// Requires `RIGORIX__LLM__API_KEY` (or ANTHROPIC_API_KEY / OPENAI_API_KEY).
pub async fn build_orchestrator(
    config: CliConfig,
    _cancellation_token: tokio_util::sync::CancellationToken,
    repo_root: String,
) -> Result<Box<dyn OrchestratorService>, CliError> {
    let engine_config = config.engine_config()?;

    // Ensure runtime directories exist
    let rigorix_dir = PathBuf::from(&repo_root).join(".rigorix");
    for sub in &["state", "state/graphs", "templates"] {
        tokio::fs::create_dir_all(rigorix_dir.join(sub)).await.ok();
    }

    // ── 0. Convert config → orchestrator domain config ────────────────
    let orch_domain_config = OrchestratorDomainConfig {
        event_buffer_capacity: 10_000,
        audit_enabled: false,
        execution_timeout_secs: engine_config.orchestrator.default_timeout_secs.max(60),
        planning_timeout_secs: 60,
        state_persistence_timeout_secs: 10,
        save_intermediate_state: false,
        propagate_cancellation: true,
    };

    // ── 1. CancellationService ────────────────────────────────────────
    let cancellation = CancellationManagerFactoryImpl
        .create_default()
        .await
        .map_err(|e| CliError::General(format!("cancellation: {e}")))?;

    // ── 2. EventBusService ─────────────────────────────────────────────
    let event_bus = EventBusFactoryImpl
        .create_default()
        .await
        .map_err(|e| CliError::General(format!("event bus: {e}")))?;

    // ── 3. StateManagerService ─────────────────────────────────────────
    let state_manager = FileSystemStateManagerFactory
        .create(
            rigorix_dir.join("state"),
            CreateStateManagerConfig::default(),
        )
        .await
        .map_err(|e| CliError::General(format!("state manager: {e}")))?;

    // ── 4. LlmBudgetService ────────────────────────────────────────────
    let budget = LlmBudgetFactoryImpl
        .create_default()
        .await
        .map_err(|e| CliError::General(format!("budget: {e}")))?;

    // ── 5. ParallelExecutionService ────────────────────────────────────
    let execution = ParallelExecutionFactoryImpl
        .create(ParallelExecutionFactoryConfig {
            executor_config: ParallelExecutorConfig {
                max_concurrent_executions: engine_config.orchestrator.max_parallel_tasks,
                default_retry_policy: RetryPolicy::default(),
                enable_cancellation: true,
                enable_enforcement: true,
                max_total_retries_per_session: engine_config.orchestrator.max_retries,
                max_failures_before_abort: 0,
                enable_fallback: true,
                enable_validation: true,
            },
            register_event_handlers: true,
            enable_progress_callbacks: true,
            event_channel_capacity: 1024,
        })
        .await
        .map_err(|e| CliError::General(format!("execution: {e}")))?;

    // ── 6. PlanningPipelineService ─────────────────────────────────────
    let llm = &engine_config.llm;

    // Resolve API key from env vars
    let api_key = std::env::var("RIGORIX__LLM__API_KEY")
        .or_else(|_| match llm.provider {
            LlmProvider::Anthropic => std::env::var("ANTHROPIC_API_KEY"),
            _ => std::env::var("OPENAI_API_KEY"),
        })
        .map_err(|_| {
            let var = match llm.provider {
                LlmProvider::Anthropic => "ANTHROPIC_API_KEY",
                _ => "OPENAI_API_KEY",
            };
            CliError::Config(format!(
                "Missing API key for {:?}. Set RIGORIX__LLM__API_KEY or {}",
                llm.provider, var
            ))
        })?;

    // Build classifier based on provider
    let classifier: Box<dyn rigorix_engine::planning::domain::classification::Classifier> =
        match llm.provider {
            LlmProvider::Anthropic => Box::new(ClaudeClassifier::new(
                api_key,
                Some(ClaudeClassifierConfig {
                    api_url: "https://api.anthropic.com/v1/messages".into(),
                    model: llm.model.clone(),
                    max_tokens: llm.max_tokens,
                    temperature: llm.temperature,
                    timeout_secs: 120,
                }),
            )),
            _ => Box::new(OpenaiClassifier::new(
                api_key,
                Some(OpenaiClassifierConfig {
                    api_url: "https://api.openai.com/v1/chat/completions".into(),
                    model: llm.model.clone(),
                    max_tokens: llm.max_tokens,
                    temperature: llm.temperature,
                    timeout_secs: 120,
                }),
            )),
        };

    let extractor = Box::new(MockParameterExtractor::new())
        as Box<dyn rigorix_engine::planning::domain::extractor::ParameterExtractor>;

    let template_service: Box<dyn TemplateEngineService> = Box::new(TemplateEngineImpl::new());

    let planning = PlanningPipelineFactoryImpl::new()
        .create_default(classifier, extractor, template_service)
        .await
        .map_err(|e| CliError::General(format!("planning: {e}")))?;

    // ── 7. AuditService (optional) ─────────────────────────────────────
    let envelope_factory: Box<
        dyn rigorix_engine::audit::application::factory::AuditEnvelopeFactory,
    > = Box::new(AuditEnvelopeFactoryImpl::new(None));
    let sender: Arc<dyn AuditSender> = Arc::new(AuditSenderImpl::new(None, None));
    let queue: Box<dyn AuditQueue> = Box::new(AuditQueueImpl::default());
    let audit = Arc::new(AuditServiceImpl::new(
        envelope_factory,
        sender,
        queue,
        false,
    )) as Arc<dyn rigorix_engine::audit::application::service::AuditService>;

    // ── Wire everything ────────────────────────────────────────────────
    let orchestrator = OrchestratorBuilderImpl::new(orch_domain_config)
        .with_repo_root(repo_root)
        .with_cancellation_service(Arc::from(cancellation))
        .with_event_bus(Arc::from(event_bus))
        .with_state_manager(Arc::from(state_manager))
        .with_budget_service(Arc::from(budget))
        .with_execution_service(Arc::from(execution))
        .with_planning_pipeline(Arc::from(planning))
        .with_audit_service(audit)
        .build()
        .await
        .map_err(CliError::Engine)?;

    Ok(orchestrator)
}
