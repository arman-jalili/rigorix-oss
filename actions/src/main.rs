//! Rigorix GitHub Action — binary entry point.
//!
//! Parses GitHub Action context, routes events to engine orchestrator calls,
//! and formats results as GitHub-native outputs (step summary, annotations,
//! output variables, PR comments).
//!
//! # Startup Flow
//!
//! 1. Initialize tracing (tracing-subscriber with RIGORIX_LOG filter)
//! 2. Read GitHub Action inputs from env vars (`INPUT_*`, `GITHUB_*`)
//! 3. Build engine orchestrator (wires all sub-services)
//! 4. Build action context and resolve execution mode
//! 5. Dispatch via ActionRouter (run, plan, validate, status)
//! 6. Write GitHub Action outputs (summary, annotations, variables)
//! 7. Exit with proper code (0 = success, 1 = failure)
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md

use std::path::PathBuf;
use std::sync::Arc;

use rigorix_actions::action_entrypoint::application::{
    context_builder_impl::ContextBuilderImpl,
    dto::{DispatchInput, ResolveModeInput},
    mode_resolver_impl::ModeResolverImpl,
    router_impl::ActionRouterImpl,
    service::{ActionRouter, ContextBuilder, ModeResolver},
};
use rigorix_actions::action_entrypoint::domain::{
    ActionContext, ActionMode, AnnotationLevel, DispatchStatus, GitHubEvent, WorkflowAnnotation,
};
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
use rigorix_engine::planning::application::pipeline_factory_impl::PlanningPipelineFactoryImpl;
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
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// CLI-style helpers for GitHub Actions output
// ---------------------------------------------------------------------------

/// Write a GitHub Actions workflow command to stdout.
///
/// Format: `::<command> <key1>=<value1>,<key2>=<value2>::<message>`
/// Write an annotation to stdout (error, warning, notice).
fn write_annotation(level: &str, annotation: &WorkflowAnnotation) {
    let mut props: Vec<String> = Vec::new();
    if let Some(file) = &annotation.file {
        props.push(format!("file={}", file));
    }
    if let Some(line) = annotation.line {
        props.push(format!("line={}", line));
    }
    if let Some(col) = annotation.column {
        props.push(format!("col={}", col));
    }
    if let Some(title) = &annotation.title {
        props.push(format!("title={}", title));
    }
    // Build the workflow command string directly
    if props.is_empty() {
        println!("::{level}::{msg}", msg = annotation.message);
    } else {
        let props_str = props.join(",");
        println!("::{level} {props_str}::{msg}", msg = annotation.message);
    }
}

/// Set a GitHub Actions output variable by writing to GITHUB_OUTPUT.
async fn set_github_output(key: &str, value: &str) {
    if let Ok(path) = std::env::var("GITHUB_OUTPUT") {
        let _ = tokio::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .await
            .map(|mut f| {
                use tokio::io::AsyncWriteExt;
                let sanitized = value.replace('\n', "\\n");
                let line = format!("{key}={sanitized}\n");
                async move {
                    let _ = f.write_all(line.as_bytes()).await;
                }
            });
    }
}

/// Write to GITHUB_STEP_SUMMARY.
async fn write_step_summary(content: &str) {
    if let Ok(path) = std::env::var("GITHUB_STEP_SUMMARY") {
        let _ = tokio::fs::write(&path, content).await;
    }
}

/// Build the markdown summary from a dispatch result.
fn build_step_summary(
    mode: &ActionMode,
    status: &DispatchStatus,
    summary: &str,
    execution_id: Option<&str>,
    duration_ms: u64,
) -> String {
    let mode_str = mode.as_str();
    let status_icon = match status {
        DispatchStatus::Success => "✅",
        DispatchStatus::Warning => "⚠️",
        DispatchStatus::Failure => "❌",
        DispatchStatus::Skipped => "⏭️",
    };

    let exec_line = execution_id
        .map(|id| format!("\n- **Execution ID**: `{}`", id))
        .unwrap_or_default();

    format!(
        "# {status_icon} Rigorix — {mode_str}\n\
         \n{summary}\n\
         \n---\n\
         **Details**\n\
         - **Mode**: `{mode_str}`\n\
         - **Status**: `{status:?}`{exec_line}\n\
         - **Duration**: `{duration_ms}ms`\n"
    )
}

// ---------------------------------------------------------------------------
// Orchestrator Builder (same pattern as CLI's build_orchestrator)
// ---------------------------------------------------------------------------

/// Build a fully wired OrchestratorService for the GitHub Action.
///
/// Wires all 6 required engine sub-services + optional audit:
/// 1. CancellationService
/// 2. EventBusService
/// 3. StateManagerService
/// 4. LlmBudgetService
/// 5. ParallelExecutionService
/// 6. PlanningPipelineService (needs LLM key)
/// 7. AuditService (optional)
async fn build_action_orchestrator(
    repo_root: &str,
    max_llm_calls: Option<u32>,
    max_llm_tokens: Option<u64>,
) -> Result<
    (
        Arc<dyn OrchestratorService>,
        Arc<dyn rigorix_engine::event_system::application::EventBusService>,
    ),
    String,
> {
    // Ensure runtime directories exist
    let rigorix_dir = PathBuf::from(repo_root).join(".rigorix");
    for sub in &["state", "state/graphs", "templates"] {
        tokio::fs::create_dir_all(rigorix_dir.join(sub)).await.ok();
    }

    let orch_domain_config = OrchestratorDomainConfig {
        event_buffer_capacity: 10_000,
        audit_enabled: false,
        execution_timeout_secs: 600, // 10 minutes for CI
        planning_timeout_secs: 120,
        state_persistence_timeout_secs: 10,
        save_intermediate_state: false,
        propagate_cancellation: true,
    };

    // ── 1. CancellationService ────────────────────────────────────────
    let cancellation = CancellationManagerFactoryImpl
        .create_default()
        .await
        .map_err(|e| format!("cancellation: {e}"))?;

    // ── 2. EventBusService ─────────────────────────────────────────────
    let event_bus: Arc<dyn rigorix_engine::event_system::application::EventBusService> = Arc::from(
        EventBusFactoryImpl
            .create_default()
            .await
            .map_err(|e| format!("event bus: {e}"))?,
    );

    // ── 3. StateManagerService ─────────────────────────────────────────
    let state_manager = Arc::from(
        FileSystemStateManagerFactory
            .create(
                rigorix_dir.join("state"),
                CreateStateManagerConfig::default(),
            )
            .await
            .map_err(|e| format!("state manager: {e}"))?,
    );

    // ── 4. LlmBudgetService ────────────────────────────────────────────
    let budget = LlmBudgetFactoryImpl
        .create_default()
        .await
        .map_err(|e| format!("budget: {e}"))?;

    // ── 5. ParallelExecutionService ────────────────────────────────────
    let execution = ParallelExecutionFactoryImpl
        .create(ParallelExecutionFactoryConfig {
            executor_config: ParallelExecutorConfig {
                max_concurrent_executions: 8,
                default_retry_policy: RetryPolicy::default(),
                enable_cancellation: true,
                enable_enforcement: true,
                max_total_retries_per_session: 3,
                max_failures_before_abort: 0,
                enable_fallback: true,
                enable_validation: true,
            },
            register_event_handlers: true,
            enable_progress_callbacks: true,
            event_channel_capacity: 1024,
            event_bus: Some(Arc::clone(&event_bus)),
        })
        .await
        .map_err(|e| format!("execution: {e}"))?;

    // ── 6. PlanningPipelineService ─────────────────────────────────────
    // Resolve API provider from env or default to Claude
    let provider_str = std::env::var("INPUT_PROVIDER").unwrap_or_else(|_| "anthropic".to_string());
    let provider = match provider_str.to_lowercase().as_str() {
        "openai" | "open_ai" => LlmProvider::OpenAI,
        "deepseek" => LlmProvider::DeepSeek,
        _ => LlmProvider::Anthropic,
    };

    let api_key = std::env::var("RIGORIX__LLM__API_KEY")
        .or_else(|_| match provider {
            LlmProvider::Anthropic => std::env::var("ANTHROPIC_API_KEY"),
            LlmProvider::OpenAI | LlmProvider::DeepSeek | LlmProvider::Custom => {
                std::env::var("OPENAI_API_KEY")
            }
        })
        .or_else(|_| std::env::var("INPUT_API_KEY"));

    let api_key =
        match api_key {
            Ok(k) => k,
            Err(_) => {
                tracing::warn!(
                    "No LLM API key found. Set ANTHROPIC_API_KEY, OPENAI_API_KEY, \
                 RIGORIX__LLM__API_KEY, or inputs.api-key. \
                 Falling back to mock classifier for planning."
                );
                // Build with mock classifier (no real LLM calls)

                // Use mock classifier and extractor for fallback
                let mock_classifier = Box::new(
                    rigorix_engine::planning::application::mock_classifier::MockClassifier::new(),
                )
                    as Box<dyn rigorix_engine::planning::domain::classification::Classifier>;
                let mock_extractor = Box::new(
                rigorix_engine::planning::application::mock_extractor::MockParameterExtractor::new()
            ) as Box<dyn rigorix_engine::planning::domain::extractor::ParameterExtractor>;
                let mock_template_service: Arc<dyn TemplateEngineService> =
                    Arc::new(TemplateEngineImpl::new());
                let planning = PlanningPipelineFactoryImpl::new()
                    .create_default(mock_classifier, mock_extractor, mock_template_service)
                    .await
                    .map_err(|e| format!("planning (mock): {e}"))?;

                let orchestrator = OrchestratorBuilderImpl::new(orch_domain_config)
                    .with_repo_root(repo_root.to_string())
                    .with_cancellation_service(Arc::from(cancellation))
                    .with_event_bus(Arc::clone(&event_bus))
                    .with_state_manager(Arc::clone(&state_manager))
                    .with_budget_service(Arc::from(budget))
                    .with_execution_service(Arc::from(execution))
                    .with_planning_pipeline(Arc::from(planning))
                    .build()
                    .await
                    .map_err(|e| format!("orchestrator build: {e}"))?;

                return Ok((Arc::from(orchestrator), event_bus));
            }
        };

    // Build classifier and template generator
    let api_base_url = std::env::var("INPUT_API_BASE_URL").unwrap_or_else(|_| match provider {
        LlmProvider::Anthropic => "https://api.anthropic.com/v1".into(),
        LlmProvider::OpenAI | LlmProvider::DeepSeek | LlmProvider::Custom => {
            "https://api.openai.com/v1".into()
        }
    });

    let model = std::env::var("INPUT_MODEL").unwrap_or_else(|_| match provider {
        LlmProvider::Anthropic => "claude-sonnet-4-20250514".into(),
        LlmProvider::OpenAI => "gpt-4o".into(),
        LlmProvider::DeepSeek => "deepseek-chat".into(),
        LlmProvider::Custom => "gpt-4o-mini".into(),
    });

    let max_tokens: u32 = std::env::var("INPUT_MAX_TOKENS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4096);

    let temperature: f64 = std::env::var("INPUT_TEMPERATURE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.3);

    let classifier: Box<dyn rigorix_engine::planning::domain::classification::Classifier> =
        match provider {
            LlmProvider::Anthropic => {
                let url = format!("{}/messages", api_base_url.trim_end_matches('/'));
                Box::new(ClaudeClassifier::new(
                    api_key.clone(),
                    Some(ClaudeClassifierConfig {
                        api_url: url,
                        model: model.clone(),
                        max_tokens,
                        temperature,
                        timeout_secs: 120,
                        requests_per_second: 10,
                    }),
                ))
            }
            _ => {
                let url = format!("{}/chat/completions", api_base_url.trim_end_matches('/'));
                Box::new(OpenaiClassifier::new(
                    api_key.clone(),
                    Some(OpenaiClassifierConfig {
                        api_url: url,
                        model: model.clone(),
                        max_tokens,
                        temperature,
                        timeout_secs: 120,
                        requests_per_second: 10,
                    }),
                ))
            }
        };

    let extractor_api_url = match provider {
        LlmProvider::Anthropic => format!("{}/messages", api_base_url.trim_end_matches('/')),
        _ => format!("{}/chat/completions", api_base_url.trim_end_matches('/')),
    };
    let extractor_provider = match provider {
        LlmProvider::Anthropic => ExtractorProvider::Anthropic,
        _ => ExtractorProvider::OpenAI,
    };
    let extractor = Box::new(LlmParameterExtractor::new(
        api_key.clone(),
        Some(LlmExtractorConfig {
            api_url: extractor_api_url,
            model: model.clone(),
            max_tokens,
            timeout_secs: 120,
            temperature,
            provider: extractor_provider,
        }),
    ))
        as Box<dyn rigorix_engine::planning::domain::extractor::ParameterExtractor>;

    let template_service: Arc<dyn TemplateEngineService> = Arc::new(TemplateEngineImpl::new());

    // Load existing templates from .rigorix/templates/ directory
    let tpl_dir = PathBuf::from(repo_root).join(".rigorix/templates");
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

    let generator_api_url = match provider {
        LlmProvider::Anthropic => format!("{}/messages", api_base_url.trim_end_matches('/')),
        _ => format!("{}/chat/completions", api_base_url.trim_end_matches('/')),
    };
    let generator_config = Some(ClaudeGeneratorConfig {
        api_url: generator_api_url,
        model: model.clone(),
        max_tokens,
        timeout_secs: 120,
        temperature,
        max_retries: 1,
    });
    let generator: Option<Box<dyn TemplateGenerator>> = match provider {
        LlmProvider::Anthropic => Some(Box::new(ClaudeTemplateGenerator::new(
            api_key.clone(),
            generator_config.clone(),
        ))),
        _ => Some(Box::new(OpenaiTemplateGenerator::new(
            api_key.clone(),
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
        .map_err(|e| format!("planning: {e}"))?;

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
    let mut builder = OrchestratorBuilderImpl::new(orch_domain_config)
        .with_repo_root(repo_root.to_string())
        .with_cancellation_service(Arc::from(cancellation))
        .with_event_bus(Arc::clone(&event_bus))
        .with_state_manager(Arc::clone(&state_manager))
        .with_budget_service(Arc::from(budget))
        .with_execution_service(Arc::from(execution))
        .with_planning_pipeline(Arc::from(planning));

    // Apply budget limits if provided
    if let (Some(calls), Some(tokens)) = (max_llm_calls, max_llm_tokens) {
        builder = builder.with_llm_budget(calls, tokens);
    }

    // Attach audit service (best-effort)
    builder = builder.with_audit_service(audit);

    let orchestrator = builder
        .build()
        .await
        .map_err(|e| format!("orchestrator build: {e}"))?;

    Ok((Arc::from(orchestrator), event_bus))
}

// ---------------------------------------------------------------------------
// GitHub Actions input parsing helpers
// ---------------------------------------------------------------------------

/// Read a GitHub Action input from environment (INPUT_<NAME>).
fn read_input(name: &str) -> Option<String> {
    let env_name = format!("INPUT_{}", name.to_uppercase().replace('-', "_"));
    std::env::var(&env_name).ok().filter(|v| !v.is_empty())
}

/// Read GITHUB_WORKSPACE.
fn read_workspace() -> String {
    std::env::var("GITHUB_WORKSPACE").unwrap_or_else(|_| ".".to_string())
}

/// Read GITHUB_EVENT_NAME.
fn read_event_name() -> String {
    std::env::var("GITHUB_EVENT_NAME").unwrap_or_else(|_| "unknown".to_string())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    // 1. Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    tracing::info!("Rigorix GitHub Action v{}", env!("CARGO_PKG_VERSION"));

    // 2. Read all action inputs
    let input_mode = read_input("mode").unwrap_or_else(|| "auto".to_string());
    let input_intent = read_input("intent");
    let _permission_mode = read_input("permission-mode");
    let fail_on_violation = read_input("fail-on-violation")
        .map(|v| v == "true")
        .unwrap_or(false);
    let fail_on_action_error = read_input("fail-on-action-error")
        .map(|v| v == "true")
        .unwrap_or(false);
    let max_llm_calls: Option<u32> = read_input("max-llm-calls").and_then(|v| v.parse().ok());
    let max_llm_tokens: Option<u64> = read_input("max-llm-tokens").and_then(|v| v.parse().ok());
    let _max_validation_iterations: u32 = read_input("max-validation-iterations")
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    let _post_pr_comment = read_input("post-pr-comment")
        .map(|v| v == "true")
        .unwrap_or(true);
    let _profile = read_input("profile");
    let repo_root = read_workspace();
    let event_name = read_event_name();

    // 3. Create cancellation token
    let cancellation_token = CancellationToken::new();
    let _ct_child = cancellation_token.child_token();

    // Install signal handler for graceful shutdown
    let ct_for_signal = cancellation_token.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        tracing::warn!("Received Ctrl+C, initiating graceful shutdown...");
        ct_for_signal.cancel();
    });

    // 4. Build the engine orchestrator
    tracing::info!("Building engine orchestrator (repo_root: {repo_root})");
    let (orchestrator, _event_bus) =
        match build_action_orchestrator(&repo_root, max_llm_calls, max_llm_tokens).await {
            Ok(services) => services,
            Err(e) => {
                tracing::error!("Failed to build orchestrator: {e}");
                write_annotation(
                    "error",
                    &WorkflowAnnotation {
                        level: AnnotationLevel::Error,
                        message: format!("Failed to initialize Rigorix engine: {e}"),
                        file: None,
                        line: None,
                        column: None,
                        title: Some("Orchestrator Build Failed".to_string()),
                    },
                );
                set_github_output("status", "failed").await;
                std::process::exit(1);
            }
        };

    // 5. Build ActionContext from environment
    // Use the ContextBuilder from the action_entrypoint module
    let context_repository =
        Arc::new(rigorix_actions::action_entrypoint::infrastructure::ContextRepositoryImpl);
    let context_builder = ContextBuilderImpl::new(context_repository);

    // Build the action context
    let context = match context_builder
        .build(
            rigorix_actions::action_entrypoint::application::dto::BuildContextInput {
                env_override: None,
                workspace_override: Some(repo_root.clone()),
                event_name_override: Some(event_name.clone()),
                event_path_override: None,
                event_payload_override: None,
            },
        )
        .await
    {
        Ok(output) => output.context,
        Err(e) => {
            tracing::warn!("ContextBuilder failed ({e}), falling back to manual context");
            // Fallback: build a minimal context manually
            let event = match event_name.as_str() {
                "workflow_dispatch" => GitHubEvent::WorkflowDispatch {
                    ref_name: read_event_ref_name(),
                },
                "pull_request" | "pull_request_target" => GitHubEvent::PullRequest {
                    pr_number: 0,
                    action: "opened".to_string(),
                    title: String::new(),
                    base_branch: String::new(),
                    head_branch: String::new(),
                    head_sha: String::new(),
                },
                "issue_comment" => GitHubEvent::IssueComment {
                    issue_number: 0,
                    comment_body: String::new(),
                    commenter: String::new(),
                },
                _ => GitHubEvent::Unknown {
                    event_name: event_name.clone(),
                },
            };

            ActionContext::new(
                repo_root.clone(),
                event,
                ActionMode::Status,
                read_github_token(),
            )
        }
    };

    // 6. Resolve execution mode
    let mode_resolver = ModeResolverImpl;
    let resolved_mode = mode_resolver
        .resolve(ResolveModeInput {
            input_mode: Some(input_mode),
            event_name,
            event_payload: None,
            input_intent,
        })
        .await
        .unwrap_or_else(|_| {
            rigorix_actions::action_entrypoint::application::dto::ResolveModeOutput {
                mode: ActionMode::Status,
                source: "fallback".to_string(),
                unambiguous: false,
                warnings: vec!["Failed to resolve mode, defaulting to Status".to_string()],
            }
        });

    let final_context = context.with_mode(resolved_mode.mode.clone());

    // 7. Dispatch via ActionRouter
    let router = ActionRouterImpl::new(orchestrator, None);
    let dispatch_result = router
        .dispatch(DispatchInput {
            context: final_context,
            timeout_secs: Some(600), // 10 minute timeout
            force: false,
        })
        .await;

    // 8. Write all outputs
    match dispatch_result {
        Ok(output) => {
            let duration_ms = output.duration_ms;
            let status = output.output.status.clone();
            let summary_text = output.output.summary.clone();
            let execution_id = output.output.execution_id.clone();

            // Write annotations
            for annotation in &output.output.annotations {
                let level_str = match annotation.level {
                    AnnotationLevel::Error => "error",
                    AnnotationLevel::Warning => "warning",
                    AnnotationLevel::Notice => "notice",
                };
                write_annotation(level_str, annotation);
            }

            // Write step summary
            let step_summary = build_step_summary(
                &resolved_mode.mode,
                &status,
                &summary_text,
                execution_id.as_deref(),
                duration_ms,
            );
            write_step_summary(&step_summary).await;

            // Set output variables
            set_github_output("execution_id", execution_id.as_deref().unwrap_or("")).await;
            set_github_output("status", &format!("{status:?}")).await;
            set_github_output("mode_used", resolved_mode.mode.as_str()).await;

            for (key, value) in &output.output.output_variables {
                set_github_output(key, value).await;
            }

            match status {
                DispatchStatus::Success => {
                    tracing::info!("Action completed successfully: {summary_text}");
                    std::process::exit(0);
                }
                DispatchStatus::Warning => {
                    tracing::warn!("Action completed with warnings: {summary_text}");
                    if fail_on_action_error {
                        std::process::exit(1);
                    }
                    std::process::exit(0);
                }
                DispatchStatus::Failure => {
                    tracing::error!("Action failed: {summary_text}");
                    if fail_on_violation || fail_on_action_error {
                        std::process::exit(1);
                    }
                    std::process::exit(0); // fail-open by default
                }
                DispatchStatus::Skipped => {
                    tracing::info!("Action skipped: {summary_text}");
                    std::process::exit(0);
                }
            }
        }
        Err(e) => {
            tracing::error!("Dispatch error: {e}");

            write_annotation(
                "error",
                &WorkflowAnnotation {
                    level: AnnotationLevel::Error,
                    message: format!("Rigorix dispatch failed: {e}"),
                    file: None,
                    line: None,
                    column: None,
                    title: Some("Dispatch Error".to_string()),
                },
            );

            let summary = format!(
                "# ❌ Rigorix — Error\n\n**Dispatch failed**: {e}\n\n---\nCheck workflow logs for details."
            );
            write_step_summary(&summary).await;
            set_github_output("status", "error").await;

            if fail_on_action_error {
                std::process::exit(1);
            }
            std::process::exit(0); // fail-open by default
        }
    }
}

/// Read the GITHUB_REF_NAME for workflow_dispatch events.
fn read_event_ref_name() -> String {
    std::env::var("GITHUB_REF_NAME").unwrap_or_else(|_| "main".to_string())
}

/// Read the GitHub token from environment.
fn read_github_token() -> Option<String> {
    std::env::var("GITHUB_TOKEN")
        .ok()
        .or_else(|| std::env::var("INPUT_GITHUB_TOKEN").ok())
}
