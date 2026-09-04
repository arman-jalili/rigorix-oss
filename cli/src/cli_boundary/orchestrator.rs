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
use rigorix_engine::hooks::application::service::HookRunnerService;
use rigorix_engine::permission::application::enforcer_factory_impl::PermissionEnforcerFactoryImpl;
use rigorix_engine::permission::application::factory::PermissionEnforcerFactory;
use rigorix_engine::permission::domain::mode::PermissionMode;
use std::collections::HashMap;

use rigorix_engine::orchestrator::application::builder::OrchestratorBuilder;
use rigorix_engine::orchestrator::application::builder_impl::OrchestratorBuilderImpl;
use rigorix_engine::orchestrator::application::service::OrchestratorService;
use rigorix_engine::orchestrator::domain::OrchestratorConfig as OrchestratorDomainConfig;
use rigorix_engine::planning::application::factory::PlanningPipelineFactory;
use rigorix_engine::planning::application::pipeline_factory_impl::PlanningPipelineFactoryImpl;
use rigorix_engine::scored_evaluation::application::ScoredEvaluationServiceImpl;
use rigorix_engine::scored_evaluation::infrastructure::LocalEvaluationRepository;
use rigorix_engine::scored_evaluation::infrastructure::backends::HttpBackend;
use rigorix_engine::scored_evaluation::infrastructure::backends::LocalBackend;
use rigorix_engine::scored_evaluation::infrastructure::backends::McpBackend;

// ── Scored Evaluation Config (parsed from .rigorix/scored_evaluation.toml) ──
#[derive(serde::Deserialize)]
struct ScoredEvalConfig {
    scored_evaluation: ScoredEvalSection,
}
#[derive(serde::Deserialize)]
struct ScoredEvalSection {
    backends: Option<HashMap<String, BackendConfig>>,
    defaults: Option<ScoredEvalDefaults>,
}
#[derive(serde::Deserialize)]
struct BackendConfig {
    #[serde(rename = "type")]
    backend_type: String,
    script_path: Option<String>,
    timeout_ms: Option<u64>,
    url: Option<String>,
}
#[derive(serde::Deserialize)]
struct ScoredEvalDefaults {
    threshold: Option<f64>,
    on_failure: Option<String>,
}
use rigorix_engine::planning::infrastructure::claude_classifier::{
    ClaudeClassifier, ClaudeClassifierConfig,
};
use rigorix_engine::planning::infrastructure::llm_extractor::{
    ExtractorProvider, LlmExtractorConfig, LlmParameterExtractor,
};
use rigorix_engine::planning::infrastructure::openai_classifier::{
    OpenaiClassifier, OpenaiClassifierConfig,
};
use rigorix_engine::state_persistence::application::factory::{
    CreateStateManagerConfig, StateManagerFactory,
};
use rigorix_engine::state_persistence::application::state_manager_factory_impl::FileSystemStateManagerFactory;
use rigorix_engine::template_generation::domain::{
    ClaudeGeneratorConfig, ClaudeTemplateGenerator, OpenaiTemplateGenerator, TemplateGenerator,
};
use rigorix_engine::templates::application::service::TemplateEngineService;
use rigorix_engine::templates::application::template_engine_impl::TemplateEngineImpl;

use crate::cli_boundary::config::CliConfig;
use crate::cli_boundary::error::CliError;

/// Services available to the CLI for direct Tier 2 command dispatch.
///
/// Holds `Arc` references to engine services that Tier 2 commands
/// (history, explain, template, config, audit, logs, diff-plan, etc.)
/// need to call directly, bypassing the orchestrator lifecycle.
pub struct CliServices {
    pub state_manager:
        Arc<dyn rigorix_engine::state_persistence::application::service::StateManagerService>,
    pub template_service: Arc<dyn TemplateEngineService>,
    pub audit_repository:
        Arc<dyn rigorix_engine::audit::infrastructure::repository::AuditEnvelopeRepository>,
    pub dag_planning_service:
        Arc<dyn rigorix_engine::dag_engine::application::service::DagPlanningService>,
    pub event_bus: Arc<dyn rigorix_engine::event_system::application::EventBusService>,
    pub config: CliConfig,
}

/// Build non-LLM CLI services for Tier 2 commands.
///
/// Builds state manager, template service, audit repository, dag planning
/// service, and event bus from `CliConfig`. These services are cheap to
/// construct and don't require an API key (except dag planning which is
/// stateless). Use this for commands like `history`, `template list`,
/// `config show`, `audit`, `logs`, `diff-plan`.
pub async fn build_cli_services(config: CliConfig) -> Result<CliServices, CliError> {
    let engine_config = config.engine_config()?;
    let repo_root = String::new(); // default to CWD

    let rigorix_dir = PathBuf::from(&repo_root).join(".rigorix");
    for sub in &["state", "templates", "audit"] {
        tokio::fs::create_dir_all(rigorix_dir.join(sub)).await.ok();
    }

    // ── State manager ──────────────────────────────────────────────────
    let state_manager = Arc::from(
        FileSystemStateManagerFactory
            .create(
                rigorix_dir.join("state"),
                CreateStateManagerConfig::default(),
            )
            .await
            .map_err(|e| CliError::General(format!("state manager: {e}")))?,
    );

    // ── Template service ───────────────────────────────────────────────
    let cli_template_service: Arc<dyn TemplateEngineService> = Arc::new(TemplateEngineImpl::new());

    // GAP-A-20: the engine's config-gated filesystem TemplateRepository is
    // the single template-loading path here (no hand-rolled fs loop). A
    // missing dir yields no templates, not an error — same as before.
    let tpl_dir = PathBuf::from(&repo_root).join(".rigorix/templates");
    {
        let template_repo =
            rigorix_engine::templates::infrastructure::template_repository_from_config(&[tpl_dir
                .to_string_lossy()
                .to_string()]);
        if let Ok(files) = template_repo.list_template_files(".", "toml").await {
            for path in files {
                if let Ok(content) = template_repo.read_template_file(&path).await
                    && let Ok(template) =
                        toml::from_str::<rigorix_engine::templates::domain::Template>(&content)
                {
                    let input = rigorix_engine::templates::application::dto::RegisterInput {
                        template,
                        overwrite: true,
                    };
                    let _ = cli_template_service.register(input).await;
                }
            }
        }
    }

    // ── Audit repository (local filesystem) ────────────────────────────
    let audit_repo = Arc::new(
        rigorix_engine::audit::infrastructure::local_audit_repository::LocalAuditEnvelopeRepository::new(
            rigorix_dir.join("audit"),
        ),
    ) as Arc<dyn rigorix_engine::audit::infrastructure::repository::AuditEnvelopeRepository>;

    // ── Dag planning service (stateless) ───────────────────────────────
    let dag_planning = Arc::new(
        rigorix_engine::dag_engine::application::service_impl::DagPlanningServiceImpl::new(),
    )
        as Arc<dyn rigorix_engine::dag_engine::application::service::DagPlanningService>;

    // ── Event bus (needed for log replay) ──────────────────────────────
    let event_bus: Arc<dyn rigorix_engine::event_system::application::EventBusService> = Arc::from(
        rigorix_engine::event_system::application::event_bus_factory_impl::EventBusFactoryImpl
            .create_default()
            .await
            .map_err(|e| CliError::General(format!("event bus: {e}")))?,
    );

    let _ = engine_config;

    Ok(CliServices {
        state_manager,
        template_service: cli_template_service,
        audit_repository: audit_repo,
        dag_planning_service: dag_planning,
        event_bus,
        config,
    })
}

/// Build a fully wired `OrchestratorService` and exposed `CliServices`.
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
) -> Result<(Box<dyn OrchestratorService>, CliServices), CliError> {
    build_orchestrator_with_budget(config, _cancellation_token, repo_root, None, None).await
}

/// Like `build_orchestrator` but with explicit CLI budget overrides.
pub async fn build_orchestrator_with_budget(
    config: CliConfig,
    _cancellation_token: tokio_util::sync::CancellationToken,
    repo_root: String,
    max_llm_calls: Option<u32>,
    max_llm_tokens: Option<u64>,
) -> Result<(Box<dyn OrchestratorService>, CliServices), CliError> {
    let engine_config = config.engine_config()?;

    // Ensure runtime directories exist
    let rigorix_dir = PathBuf::from(&repo_root).join(".rigorix");
    for sub in &["state", "state/graphs", "templates"] {
        tokio::fs::create_dir_all(rigorix_dir.join(sub)).await.ok();
    }

    // ── 0. Convert config → orchestrator domain config ────────────────
    let audit_enabled = engine_config.audit_backend_url.is_some();
    let model_version = if engine_config.llm.model.is_empty() {
        None
    } else {
        Some(engine_config.llm.model.clone())
    };
    let orch_domain_config = OrchestratorDomainConfig {
        event_buffer_capacity: 10_000,
        audit_enabled,
        execution_timeout_secs: engine_config.orchestrator.default_timeout_secs.max(60),
        planning_timeout_secs: 60,
        state_persistence_timeout_secs: 10,
        save_intermediate_state: false,
        propagate_cancellation: true,
        capture_planning_prompt: false,
        max_llm_calls: None,
        max_llm_tokens: None,
        model_version,
    };

    // ── 1. CancellationService ────────────────────────────────────────
    let cancellation = CancellationManagerFactoryImpl
        .create_default()
        .await
        .map_err(|e| CliError::General(format!("cancellation: {e}")))?;

    // ── 2. EventBusService ─────────────────────────────────────────────
    let event_bus: Arc<dyn rigorix_engine::event_system::application::EventBusService> = Arc::from(
        EventBusFactoryImpl
            .create_default()
            .await
            .map_err(|e| CliError::General(format!("event bus: {e}")))?,
    );

    // ── 3. StateManagerService ─────────────────────────────────────────
    let state_manager = Arc::from(
        FileSystemStateManagerFactory
            .create(
                rigorix_dir.join("state"),
                CreateStateManagerConfig::default(),
            )
            .await
            .map_err(|e| CliError::General(format!("state manager: {e}")))?,
    );

    // ── 4. LlmBudgetService ────────────────────────────────────────────
    let budget = if let (Some(calls), Some(tokens)) = (max_llm_calls, max_llm_tokens) {
        LlmBudgetFactoryImpl
            .create_custom(
                calls,
                tokens.min(u32::MAX as u64) as u32,
                "cli-run".to_string(),
            )
            .await
            .map_err(|e| CliError::General(format!("budget: {e}")))?
    } else {
        LlmBudgetFactoryImpl
            .create_default()
            .await
            .map_err(|e| CliError::General(format!("budget: {e}")))?
    };

    // ── 5. ParallelExecutionService ────────────────────────────────────
    let permission_mode = resolve_permission_mode(engine_config.permission_mode.as_ref());
    let permission_enforcer: Option<
        Arc<dyn rigorix_engine::permission::application::enforcer::PermissionEnforcer>,
    > = match PermissionEnforcerFactoryImpl
        .create_with_mode(permission_mode)
        .await
    {
        Ok(enforcer) => Some(Arc::from(enforcer)),
        Err(e) => {
            tracing::warn!("permission enforcer unavailable ({e}); continuing without mode gating");
            None
        }
    };
    let hook_runner = load_hook_runner(repo_root.as_str());
    let approval_binding =
        rigorix_engine::execution_engine::application::factory::ApprovalBindingSetup::from_env(
            std::path::Path::new(&repo_root),
        );
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
                approval_repo_path: None,
            },
            register_event_handlers: true,
            enable_progress_callbacks: true,
            event_channel_capacity: 1024,
            event_bus: Some(Arc::clone(&event_bus)),
            permission_enforcer,
            hook_runner,
            approval_binding,
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

    // Build classifier and template generator based on provider
    // Base URL without endpoint path — classifier and generator append their own paths.
    let api_base_url = llm.base_url.clone().unwrap_or_else(|| match llm.provider {
        LlmProvider::Anthropic => "https://api.anthropic.com/v1".into(),
        _ => "https://api.openai.com/v1".into(),
    });
    let api_key_for_generator = api_key.clone();

    let classifier: Box<dyn rigorix_engine::planning::domain::classification::Classifier> =
        match llm.provider {
            LlmProvider::Anthropic => {
                let url = format!("{}/messages", api_base_url.trim_end_matches('/'));
                Box::new(ClaudeClassifier::new(
                    api_key.clone(),
                    Some(ClaudeClassifierConfig {
                        api_url: url,
                        model: llm.model.clone(),
                        max_tokens: llm.max_tokens,
                        temperature: llm.temperature,
                        timeout_secs: 120,
                        requests_per_second: 10,
                    }),
                ))
            }
            _ => {
                let url = format!("{}/chat/completions", api_base_url.trim_end_matches('/'));
                Box::new(OpenaiClassifier::new(
                    api_key,
                    Some(OpenaiClassifierConfig {
                        api_url: url,
                        model: llm.model.clone(),
                        max_tokens: llm.max_tokens,
                        temperature: llm.temperature,
                        timeout_secs: 120,
                        requests_per_second: 10,
                    }),
                ))
            }
        };

    // Build LLM-based parameter extractor (replaces mock for real extraction)
    let extractor_api_url = match llm.provider {
        LlmProvider::Anthropic => format!("{}/messages", api_base_url.trim_end_matches('/')),
        _ => format!("{}/chat/completions", api_base_url.trim_end_matches('/')),
    };
    let extractor_provider = match llm.provider {
        LlmProvider::Anthropic => ExtractorProvider::Anthropic,
        _ => ExtractorProvider::OpenAI,
    };
    let extractor = Box::new(LlmParameterExtractor::new(
        api_key_for_generator.clone(),
        Some(LlmExtractorConfig {
            api_url: extractor_api_url,
            model: llm.model.clone(),
            max_tokens: llm.max_tokens,
            timeout_secs: 120,
            temperature: llm.temperature,
            provider: extractor_provider,
        }),
    ))
        as Box<dyn rigorix_engine::planning::domain::extractor::ParameterExtractor>;

    let template_service: Arc<dyn TemplateEngineService> = Arc::new(TemplateEngineImpl::new());

    // Load existing templates from .rigorix/templates/ directory
    let tpl_dir = std::path::PathBuf::from(&repo_root).join(".rigorix/templates");
    if tpl_dir.exists()
        && let Ok(mut entries) = tokio::fs::read_dir(&tpl_dir).await
    {
        let mut files = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                files.push(path);
            }
        }
        for path in files {
            if let Ok(content) = tokio::fs::read_to_string(&path).await
                && let Ok(template) =
                    toml::from_str::<rigorix_engine::templates::domain::Template>(&content)
            {
                let input = rigorix_engine::templates::application::dto::RegisterInput {
                    template,
                    overwrite: true,
                };
                let _ = template_service.register(input).await;
            }
        }
    }

    // Create template generator for LLM-based plan generation when no template matches.
    // Resolve the API URL with the correct endpoint path per provider.
    // The api_base_url from models.json is the base (e.g. https://api.deepseek.com/v1).
    // OpenAI-compatible providers need /chat/completions, Anthropic needs /messages.
    let generator_api_url = match llm.provider {
        LlmProvider::Anthropic => format!("{}/messages", api_base_url.trim_end_matches('/')),
        _ => format!("{}/chat/completions", api_base_url.trim_end_matches('/')),
    };
    let generator_config = Some(ClaudeGeneratorConfig {
        api_url: generator_api_url,
        model: llm.model.clone(),
        max_tokens: llm.max_tokens,
        timeout_secs: 120,
        temperature: llm.temperature,
        max_retries: 1,
    });
    let generator: Option<Box<dyn TemplateGenerator>> = match llm.provider {
        LlmProvider::Anthropic => Some(Box::new(ClaudeTemplateGenerator::new(
            api_key_for_generator.clone(),
            generator_config.clone(),
        ))),
        _ => Some(Box::new(OpenaiTemplateGenerator::new(
            api_key_for_generator.clone(),
            generator_config.clone(),
        ))),
    };

    let planning = PlanningPipelineFactoryImpl::new()
        .create_custom(
            classifier,
            extractor,
            Arc::clone(&template_service),
            generator,
            None,
        )
        .await
        .map_err(|e| CliError::General(format!("planning: {e}")))?;

    // ── 7. AuditService (optional) ─────────────────────────────────────
    let envelope_factory: Box<
        dyn rigorix_engine::audit::application::factory::AuditEnvelopeFactory,
    > = Box::new(AuditEnvelopeFactoryImpl::new(resolve_hmac_key(
        engine_config.audit_hmac_key.as_ref(),
    )));
    let audit_backend_url = engine_config.audit_backend_url.clone();
    let audit_backend_key = engine_config.audit_backend_key.clone();
    let sender: Arc<dyn AuditSender> =
        Arc::new(AuditSenderImpl::new(None, audit_backend_url).with_api_key(audit_backend_key));
    let queue: Box<dyn AuditQueue> = Box::new(AuditQueueImpl::default());
    let audit = Arc::new(AuditServiceImpl::new(
        envelope_factory,
        sender,
        queue,
        audit_enabled,
    )) as Arc<dyn rigorix_engine::audit::application::service::AuditService>;

    // ── 8. ScoredEvaluationService (optional) ──────────────────────────
    let scored_evaluation_config_path = rigorix_dir.join("scored_evaluation.toml");
    let scored_evaluation_svc = if scored_evaluation_config_path.exists() {
        match std::fs::read_to_string(&scored_evaluation_config_path) {
            Ok(config_content) => match toml::from_str::<ScoredEvalConfig>(&config_content) {
                Ok(ref cfg) => {
                    if let Some(ref defaults) = cfg.scored_evaluation.defaults {
                        tracing::debug!(threshold = ?defaults.threshold, on_failure = ?defaults.on_failure, "Scored evaluation defaults loaded");
                    }
                    let mut backends: HashMap<
                        String,
                        Box<dyn rigorix_engine::scored_evaluation::domain::ScoringBackend>,
                    > = HashMap::new();
                    if let Some(ref backends_config) = cfg.scored_evaluation.backends {
                        for (name, bcfg) in backends_config.iter() {
                            match bcfg.backend_type.as_str() {
                                "local" => {
                                    if let Some(script_path) = &bcfg.script_path {
                                        let full_path = rigorix_dir
                                            .parent()
                                            .map(|p| p.join(script_path))
                                            .unwrap_or_else(|| {
                                                std::path::PathBuf::from(script_path)
                                            });
                                        backends.insert(
                                            name.clone(),
                                            Box::new(LocalBackend::new(
                                                full_path.to_string_lossy().to_string(),
                                                bcfg.timeout_ms.unwrap_or(30_000),
                                            )),
                                        );
                                    }
                                }
                                "mcp" => {
                                    if let Some(url) = &bcfg.url {
                                        backends.insert(
                                            name.clone(),
                                            Box::new(McpBackend::new(
                                                url.clone(),
                                                bcfg.timeout_ms.unwrap_or(30_000),
                                            )),
                                        );
                                    }
                                }
                                "http" => {
                                    if let Some(url) = &bcfg.url {
                                        backends.insert(
                                            name.clone(),
                                            Box::new(HttpBackend::new(
                                                url.clone(),
                                                HashMap::new(),
                                                bcfg.timeout_ms.unwrap_or(30_000),
                                            )),
                                        );
                                    }
                                }
                                _ => {
                                    tracing::warn!(backend = %name, type = %bcfg.backend_type, "Unsupported scored evaluation backend type; skipping");
                                }
                            }
                        }
                    }

                    if !backends.is_empty() {
                        let eval_repo = Box::new(LocalEvaluationRepository::new(
                            rigorix_dir.join("evaluations"),
                        ));
                        Some(Arc::new(
                                ScoredEvaluationServiceImpl::new(backends, eval_repo),
                            ) as Arc<dyn rigorix_engine::scored_evaluation::application::ScoredEvaluationService>)
                    } else {
                        tracing::warn!(
                            "scored_evaluation.toml found but no valid backends configured"
                        );
                        None
                    }
                }
                Err(e) => {
                    tracing::warn!(path = %scored_evaluation_config_path.display(), error = %e, "Failed to parse scored_evaluation.toml");
                    None
                }
            },
            Err(e) => {
                tracing::warn!(path = %scored_evaluation_config_path.display(), error = %e, "Failed to read scored_evaluation.toml");
                None
            }
        }
    } else {
        tracing::debug!(
            "No scored_evaluation.toml found at {}",
            scored_evaluation_config_path.display()
        );
        None
    };

    // ── Wire everything ────────────────────────────────────────────────
    let mut builder = OrchestratorBuilderImpl::new(orch_domain_config)
        .with_repo_root(repo_root)
        .with_cancellation_service(Arc::from(cancellation))
        .with_event_bus(Arc::clone(&event_bus))
        .with_state_manager(Arc::clone(&state_manager))
        .with_budget_service(Arc::from(budget))
        .with_execution_service(Arc::from(execution))
        .with_planning_pipeline(Arc::from(planning))
        .with_audit_service(audit);

    if let Some(se_svc) = scored_evaluation_svc {
        builder = builder.with_scored_evaluation_service(se_svc);
    }

    let orchestrator = builder.build().await.map_err(CliError::Engine)?;

    // Build audit repository and dag planning service for CliServices
    let audit_repo = Arc::new(
        rigorix_engine::audit::infrastructure::local_audit_repository::LocalAuditEnvelopeRepository::new(
            rigorix_dir.join("audit"),
        ),
    ) as Arc<dyn rigorix_engine::audit::infrastructure::repository::AuditEnvelopeRepository>;

    let dag_planning = Arc::new(
        rigorix_engine::dag_engine::application::service_impl::DagPlanningServiceImpl::new(),
    )
        as Arc<dyn rigorix_engine::dag_engine::application::service::DagPlanningService>;

    let services = CliServices {
        state_manager,
        template_service: Arc::clone(&template_service),
        audit_repository: audit_repo,
        dag_planning_service: dag_planning,
        event_bus: event_bus.clone(),
        config,
    };

    Ok((orchestrator, services))
}

// ---------------------------------------------------------------------------
// Permission mode + HMAC key resolution
// ---------------------------------------------------------------------------

/// Resolve the effective permission mode for tool-execution gating.
///
/// Resolution order: configured value (rigorix.toml / `RIGORIX__PERMISSION_MODE`)
/// → `RIGORIX_PERMISSION_MODE` env var → `workspace_write` (safe default).
/// Accepts both `dangerous_full_access` (action.yml spelling) and
/// `danger_full_access` (engine serde spelling).
fn resolve_permission_mode(configured: Option<&String>) -> PermissionMode {
    let raw = configured
        .cloned()
        .or_else(|| std::env::var("RIGORIX_PERMISSION_MODE").ok())
        .unwrap_or_else(|| "workspace_write".to_string());
    match raw.as_str() {
        "read_only" => PermissionMode::ReadOnly,
        "dangerous_full_access" | "danger_full_access" => PermissionMode::DangerousFullAccess,
        _ => PermissionMode::WorkspaceWrite,
    }
}

/// Resolve the HMAC-SHA256 audit signing key.
///
/// Resolution order: configured value (rigorix.toml / `RIGORIX__AUDIT_HMAC_KEY`)
/// → `RIGORIX_HMAC_KEY` env var. When `None`, audit envelopes are emitted
/// unsigned (and `AuditEnvelopeFactory` refuses to verify them).
fn resolve_hmac_key(configured: Option<&String>) -> Option<String> {
    configured
        .cloned()
        .or_else(|| std::env::var("RIGORIX_HMAC_KEY").ok())
        .filter(|k| !k.is_empty())
}

/// Load a `HookRunnerService` from `.rigorix/hooks.toml` (optional).
///
/// When the file exists and parses as a `HookConfig`, every tool execution
/// runs the configured PreToolUse/PostToolUse shell hooks. Returns `None`
/// (no hook interception) when the file is absent — never fatal.
fn load_hook_runner(repo_root: &str) -> Option<Arc<dyn HookRunnerService>> {
    use rigorix_engine::hooks::application::factory::HookRunnerFactory;
    use rigorix_engine::hooks::application::runner_factory_impl::HookRunnerFactoryImpl;
    use rigorix_engine::hooks::domain::config::HookConfig;

    let path = std::path::PathBuf::from(repo_root).join(".rigorix/hooks.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    let config: HookConfig = toml::from_str(&content).ok()?;
    match HookRunnerFactoryImpl.create(config) {
        Ok(runner) => {
            tracing::info!("Hooks enabled from {}", path.display());
            Some(Arc::from(runner))
        }
        Err(e) => {
            tracing::warn!("hooks config invalid: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(s: &str) -> Option<String> {
        Some(s.to_string())
    }

    #[test]
    fn permission_mode_defaults_to_workspace_write() {
        assert_eq!(
            resolve_permission_mode(None),
            PermissionMode::WorkspaceWrite
        );
        assert_eq!(
            resolve_permission_mode(mode("bogus").as_ref()),
            PermissionMode::WorkspaceWrite
        );
    }

    #[test]
    fn permission_mode_parses_all_modes() {
        assert_eq!(
            resolve_permission_mode(mode("read_only").as_ref()),
            PermissionMode::ReadOnly
        );
        assert_eq!(
            resolve_permission_mode(mode("workspace_write").as_ref()),
            PermissionMode::WorkspaceWrite
        );
        // Both documented spellings of the unrestricted mode are accepted.
        assert_eq!(
            resolve_permission_mode(mode("dangerous_full_access").as_ref()),
            PermissionMode::DangerousFullAccess
        );
        assert_eq!(
            resolve_permission_mode(mode("danger_full_access").as_ref()),
            PermissionMode::DangerousFullAccess
        );
    }

    #[test]
    fn hmac_key_uses_configured_value() {
        assert_eq!(
            resolve_hmac_key(mode("secret-key").as_ref()),
            Some("secret-key".to_string())
        );
        assert_eq!(resolve_hmac_key(None), None);
        assert_eq!(resolve_hmac_key(mode("").as_ref()), None);
    }
}
