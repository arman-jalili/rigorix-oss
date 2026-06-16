//! Binary entry point for the rigorix CLI.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: CLI binary entry point with config, tracing, signals, dispatch
//! Issue: #237
//!
//! Per ADR-002 (CLI/engine split), this is a thin wrapper:
//! 1. Parse CLI args via clap
//! 2. Load and merge configuration
//! 3. Initialize tracing
//! 4. Install signal handlers
//! 5. Dispatch command → render output → exit
//!
//! # Exit Codes
//! - 0: Success
//! - 1: General error / engine error
//! - 2: Configuration error (missing config, parse error, missing API key)
//! - 3: Invalid command or arguments
//! - 130: Cancelled by user (Ctrl+C)
//! - 137: Killed / timeout
//!
//! # Contract (Frozen)
//! - Entry point only — no business logic
//! - All errors are caught and formatted before exit
//! - Tracing is initialized as early as possible

use std::process;

use clap::Parser;
use tracing::info;

use rigorix::cancellation::infrastructure::signal::SignalHandler;
use rigorix::cancellation::infrastructure::signal_impl::SignalHandlerImpl;
use rigorix::cli_boundary::domain::error::CliError;
use rigorix::cli_boundary::infrastructure::output::LogFormatter;
use rigorix::cli_boundary::infrastructure::output_impl::LogFormatterImpl;
use rigorix::cli_boundary::interfaces::cli::{CliArgs, CliCommand, GlobalOptions};
use rigorix::configuration::domain::config::{
    CliConfig, ColorMode, LogFormat, LogLevel, OutputFormat,
};
use rigorix::configuration::infrastructure::config::CliConfigLoader;
use rigorix::configuration::infrastructure::config_impl::{
    CliConfigLoaderImpl, build_engine_cli_overrides, validate_api_key_for_command,
};
use rigorix_engine::configuration::application::ConfigService;

#[tokio::main]
async fn main() {
    // Parse CLI args
    let args = CliArgs::parse();

    // Build CLI config from global flags
    let cli_overrides = parse_global_options(&args.global_opts);

    // Load config (file + env + overrides)
    let config = match load_config(&cli_overrides).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(e.exit_code());
        }
    };

    // Validate config for the specific command before proceeding
    let command_name = command_name(&args.command);
    if let Some(err) = validate_api_key_for_command(&config, command_name) {
        eprintln!("{}", err);
        process::exit(err.exit_code());
    }

    // Bridge CLI config to engine's ConfigService
    match init_engine_config(&config).await {
        Ok(engine_config) => {
            info!(
                "Engine config loaded — sources: {:?}",
                engine_config.sources_used
            );
        }
        Err(e) => {
            eprintln!("{}", e);
            process::exit(e.exit_code());
        }
    }

    // Initialize tracing AFTER config is loaded (so we respect RIGORIX_LOG)
    rigorix::observability::infrastructure::tracing::init_tracing(
        config.log_level,
        config.log_format,
    );
    info!("rigorix CLI starting");

    // Install signal handler
    let signal_handler = SignalHandlerImpl::new();
    let _signal_rx = match signal_handler.install().await {
        Ok(rx) => rx,
        Err(e) => {
            eprintln!("Failed to install signal handler: {}", e);
            process::exit(e.exit_code());
        }
    };

    // Initialize template command handler
    let template_handler =
        match rigorix::templates::infrastructure::template_handler::TemplateCommandHandler::new(
            config.clone(),
        )
        .await
        {
            Ok(handler) => handler,
            Err(e) => {
                eprintln!("Failed to initialize template system: {}", e);
                process::exit(e.exit_code());
            }
        };

    // Dispatch command
    let formatter = LogFormatterImpl::new(config.output_format);
    let result = dispatch_command(args.command, &config, &formatter, &template_handler).await;

    // Handle result
    match result {
        Ok(output) => {
            println!("{}", output);
            process::exit(0);
        }
        Err(e) => {
            let formatted = formatter.format_error(&e).await;
            eprintln!("{}", formatted);
            process::exit(e.exit_code());
        }
    }
}

/// Parse global CLI options into a CliConfig for the config loader.
fn parse_global_options(opts: &GlobalOptions) -> CliConfig {
    CliConfig {
        output_format: match opts.output_format.as_str() {
            "json" => OutputFormat::Json,
            "quiet" => OutputFormat::Quiet,
            _ => OutputFormat::Pretty,
        },
        color: match opts.color.as_str() {
            "always" => ColorMode::Always,
            "never" => ColorMode::Never,
            _ => ColorMode::Auto,
        },
        log_level: match opts.log_level.as_str() {
            "trace" => LogLevel::Trace,
            "debug" => LogLevel::Debug,
            "warn" => LogLevel::Warn,
            "error" => LogLevel::Error,
            _ => LogLevel::Info,
        },
        log_format: match opts.log_format.as_str() {
            "json" => LogFormat::Json,
            _ => LogFormat::Pretty,
        },
        config_path: opts.config_path.clone(),
        ..CliConfig::default()
    }
}

/// Return a string name for the command (for validation purposes).
fn command_name(command: &CliCommand) -> &'static str {
    match command {
        CliCommand::Run { .. } => "run",
        CliCommand::Plan { .. } => "plan",
        CliCommand::Init { .. } => "init",
        CliCommand::Generate { .. } => "generate",
        CliCommand::History(_) => "history",
        CliCommand::Logs { .. } => "logs",
        CliCommand::Audit(_) => "audit",
        CliCommand::Template(_) => "template",
    }
}

/// Initialize the engine's `ConfigService` with CLI-side values.
///
/// Bridges the CLI configuration to the engine's configuration pipeline so that
/// CLI flags, env vars, and the config file are all available to engine services.
async fn init_engine_config(
    cli_config: &CliConfig,
) -> Result<rigorix_engine::configuration::application::dto::LoadConfigOutput, CliError> {
    let engine_service =
        rigorix_engine::configuration::application::config_service_impl::ConfigServiceImpl::default(
        );

    let cli_overrides = build_engine_cli_overrides(cli_config);

    let input = rigorix_engine::configuration::application::dto::LoadConfigInput {
        config_path: cli_config.config_path.clone(),
        cli_overrides: Some(cli_overrides),
        ..Default::default()
    };

    engine_service
        .load(input)
        .await
        .map_err(|e| CliError::Engine(rigorix_engine::error::CoreOrchestratorError::from(e)))
}

/// Load and merge CLI configuration.
async fn load_config(cli_overrides: &CliConfig) -> Result<CliConfig, CliError> {
    let loader = CliConfigLoaderImpl::new();

    if let Some(ref config_path) = cli_overrides.config_path {
        loader
            .load_from_path(config_path, cli_overrides.clone())
            .await
    } else {
        loader.load(cli_overrides.clone()).await
    }
}

/// Dispatch a CLI command to the appropriate handler.
async fn dispatch_command(
    command: CliCommand,
    _config: &CliConfig,
    formatter: &LogFormatterImpl,
    template_handler: &rigorix::templates::infrastructure::template_handler::TemplateCommandHandler,
) -> Result<String, CliError> {
    match command {
        CliCommand::Run {
            intent,
            dry_run,
            skip_confirmations,
            skip_budget_check,
        } => {
            info!("Command: run — intent: {}, dry_run: {}", intent, dry_run);

            let _input = rigorix::cli_boundary::application::dto::RunInput {
                intent,
                dry_run,
                skip_confirmations,
                skip_budget_check,
            };

            // Placeholder: return a stub response until engine integration is wired
            let output = rigorix::cli_boundary::application::dto::RunOutput {
                session_id: "pending".into(),
                outcome: rigorix::cli_boundary::domain::event::SessionOutcome::Completed,
                summary: rigorix::cli_boundary::application::dto::ExecutionSummary {
                    total_nodes: 0,
                    completed: 0,
                    failed: 0,
                    skipped: 0,
                    total_duration_ms: 0,
                },
            };

            formatter.format_run(&output).await
        }

        CliCommand::Plan { intent } => {
            info!("Command: plan — intent: {}", intent);

            let _input = rigorix::cli_boundary::application::dto::PlanInput { intent };

            // Placeholder
            let output = rigorix::cli_boundary::application::dto::PlanOutput {
                template_id: "none".into(),
                template_name: "No template matched".into(),
                confidence: 0.0,
                nodes: vec![],
                total_estimated_tokens: 0,
                total_estimated_calls: 0,
                budget_exceeded: false,
                is_valid: false,
            };

            formatter.format_plan(&output).await
        }

        CliCommand::Init {
            path,
            non_interactive,
            api_key,
            enforcement_preset,
        } => {
            info!("Command: init — path: {}", path);

            let input = rigorix::cli_boundary::application::dto::InitInput {
                target_path: path,
                interactive: !non_interactive,
                api_key,
                enforcement_preset,
            };

            Ok(format!(
                "rigorix init — target: {}, interactive: {}",
                input.target_path, input.interactive
            ))
        }

        CliCommand::Generate {
            intent,
            stdout,
            dry_run,
        } => {
            info!("Command: generate — intent: {}", intent);

            let input = rigorix::cli_boundary::application::dto::GenerateInput {
                intent,
                stdout,
                dry_run,
            };

            Ok(format!(
                "rigorix generate — intent: {}, stdout: {}, dry_run: {}",
                input.intent, input.stdout, input.dry_run
            ))
        }

        CliCommand::History(history_cmd) => {
            info!("Command: history");
            match history_cmd {
                rigorix::cli_boundary::interfaces::cli::HistoryCommands::List {
                    limit: _,
                    status: _,
                } => {
                    let output = rigorix::cli_boundary::application::dto::HistoryListOutput {
                        sessions: vec![],
                        total: 0,
                    };
                    formatter.format_history_list(&output).await
                }
                rigorix::cli_boundary::interfaces::cli::HistoryCommands::Show { session_id } => {
                    let output = rigorix::cli_boundary::application::dto::HistoryShowOutput {
                        session: rigorix::cli_boundary::application::dto::SessionSummary {
                            session_id,
                            command: String::new(),
                            template_id: None,
                            outcome:
                                rigorix::cli_boundary::domain::event::SessionOutcome::Completed,
                            duration_ms: 0,
                            timestamp: String::new(),
                        },
                        nodes: vec![],
                    };
                    formatter.format_history_show(&output).await
                }
            }
        }

        CliCommand::Logs {
            session_id,
            event_type,
            node_id,
            severity,
            follow,
            limit,
        } => {
            info!("Command: logs — follow: {}", follow);
            let _input = rigorix::cli_boundary::application::dto::LogsInput {
                session_id,
                event_type,
                node_id,
                min_severity: severity,
                follow,
                limit,
            };

            let output = rigorix::cli_boundary::application::dto::LogsOutput {
                entries: vec![],
                total: 0,
            };
            formatter.format_logs(&output).await
        }

        CliCommand::Audit(audit_cmd) => {
            info!("Command: audit");
            match audit_cmd {
                rigorix::cli_boundary::interfaces::cli::AuditCommands::List { limit: _ } => {
                    let output = rigorix::cli_boundary::application::dto::AuditListOutput {
                        audits: vec![],
                        total: 0,
                    };
                    formatter.format_audit_list(&output).await
                }
                rigorix::cli_boundary::interfaces::cli::AuditCommands::Show { audit_id } => {
                    let output = rigorix::cli_boundary::application::dto::AuditShowOutput {
                        audit: rigorix::cli_boundary::application::dto::AuditSummary {
                            audit_id,
                            session_id: String::new(),
                            planning_hash: String::new(),
                            timestamp: String::new(),
                        },
                        events: vec![],
                    };
                    formatter.format_audit_show(&output).await
                }
                rigorix::cli_boundary::interfaces::cli::AuditCommands::Diff {
                    audit_id_1,
                    audit_id_2,
                } => {
                    let output = rigorix::cli_boundary::application::dto::AuditDiffOutput {
                        identical: true,
                        planning_hash_1: audit_id_1,
                        planning_hash_2: audit_id_2,
                        diff_description: "No diff available (engine integration pending)".into(),
                    };
                    formatter.format_audit_diff(&output).await
                }
            }
        }

        CliCommand::Template(template_cmd) => {
            info!("Command: template");
            match template_cmd {
                rigorix::cli_boundary::interfaces::cli::TemplateCommands::List => {
                    match template_handler.list().await {
                        Ok(output) => formatter.format_template_list(&output).await,
                        Err(e) => Err(e),
                    }
                }
                rigorix::cli_boundary::interfaces::cli::TemplateCommands::Show { template_id } => {
                    match template_handler.show(&template_id).await {
                        Ok(output) => formatter.format_template_show(&output).await,
                        Err(e) => Err(e),
                    }
                }
            }
        }
    }
}
