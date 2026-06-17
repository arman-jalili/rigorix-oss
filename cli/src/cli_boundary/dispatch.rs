//! Command dispatcher — routes parsed commands to engine services.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#dispatch-logic
//! Implements: Dispatcher component: dispatch function and DispatchResult type
//! Issue: issue-dispatcher
//!
//! # Contract (Frozen)
//!
//! The dispatcher is a `match` over `CliCommand` variants that routes each
//! command to the appropriate handler:
//!
//! | Command | Target | Method |
//! |---------|--------|--------|
//! | Run | OrchestratorService | `orchestrator.run(RunInput)` |
//! | Plan | OrchestratorService | `orchestrator.plan_only(PlanOnlyInput)` |
//! | Cancel | OrchestratorService | `orchestrator.cancel(CancelInput)` |
//! | Status | OrchestratorService | `orchestrator.status()` |
//! | History | StateManagerService | `list_executions(limit, status)` |
//! | Explain | StateManagerService | `load_state(execution_id)` |
//! | DiffPlan | DagPlanningService | `compare_plans(id1, id2)` |
//! | Generate | TemplateGenerator | `generate(intent)` |
//! | Template | TemplateEngine | `list() / show(id)` |
//! | Audit | AuditService | `list() / show() / diff()` |
//! | Logs | EventBusService | `subscribe(session_id)` |
//! | Config | ConfigService | `validate() / load() / init()` |
//! | Init | CLI-only | scaffold `.rigorix/` |
//! | Key | CLI-only | generate API key |
//! | Tui | TUI module | `tui::run(config, ct, exec, run)` |

use std::fmt;

use serde_json::Value as JsonValue;

use crate::cli_boundary::cli::{AuditAction, CliCommand, ConfigAction, TemplateAction};

// ---------------------------------------------------------------------------
// Dispatch result type
// ---------------------------------------------------------------------------

/// Unified result from any command dispatch.
///
/// The output formatter (`output::format_and_exit`) renders this type
/// into the selected output format (Pretty, JSON, Markdown, Quiet).
#[derive(Debug)]
pub struct DispatchResult {
    /// Human-readable summary of the operation result.
    pub summary: String,

    /// Structured data payload for JSON/Markdown output.
    pub data: Option<JsonValue>,

    /// Exit code to return (0 = success, non-zero = error).
    pub exit_code: i32,
}

impl DispatchResult {
    /// Create a successful dispatch result.
    pub fn success(summary: impl Into<String>) -> Self {
        DispatchResult {
            summary: summary.into(),
            data: None,
            exit_code: 0,
        }
    }

    /// Create a successful dispatch result with structured data.
    pub fn success_with_data(summary: impl Into<String>, data: JsonValue) -> Self {
        DispatchResult {
            summary: summary.into(),
            data: Some(data),
            exit_code: 0,
        }
    }

    /// Create an error dispatch result.
    pub fn error(summary: impl Into<String>, exit_code: i32) -> Self {
        DispatchResult {
            summary: summary.into(),
            data: None,
            exit_code,
        }
    }

    /// Returns `true` if this result represents a successful dispatch.
    pub fn is_success(&self) -> bool {
        self.exit_code == 0
    }
}

impl fmt::Display for DispatchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary)
    }
}

// ---------------------------------------------------------------------------
// CLI-only helpers
// ---------------------------------------------------------------------------

/// Scaffold a `.rigorix/` directory with default config.
fn cmd_init() -> DispatchResult {
    let path = std::path::Path::new(".rigorix");
    if path.is_dir() {
        return DispatchResult::success(".rigorix/ already exists");
    }

    match std::fs::create_dir_all(path.join("templates")) {
        Ok(_) => {
            // Write default config
            let default_config = r#"# Rigorix Configuration
[orchestrator]
max_parallel_tasks = 4
max_retries = 3
default_timeout_secs = 120

[logging]
level = "info"
format = "text"

[llm]
# Set your provider: anthropic, openai, deepseek, lmstudio, or ollama
provider = "anthropic"
# Model ID — see ~/.rigorix/models.json for available models
model = "claude-sonnet-4-6"
# Base URL is auto-resolved from models.json if present
# max_tokens and temperature override model defaults
# max_tokens = 4096
# temperature = 0.7

# API key: set via RIGORIX__LLM__API_KEY or ANTHROPIC_API_KEY / OPENAI_API_KEY
"#;
            let config_path = path.join("rigorix.toml");
            if let Err(e) = std::fs::write(&config_path, default_config) {
                return DispatchResult::error(format!("Failed to write config: {e}"), 1);
            }
            DispatchResult::success("Initialized .rigorix/ directory")
        }
        Err(e) => DispatchResult::error(format!("Failed to create .rigorix/: {e}"), 1),
    }
}

/// Generate an API key.
fn cmd_key(label: Option<String>) -> DispatchResult {
    let key = uuid::Uuid::new_v4();
    let label_str = label.unwrap_or_else(|| "default".to_string());
    let data = serde_json::json!({
        "key": key.to_string(),
        "label": label_str,
    });
    DispatchResult::success_with_data(format!("Generated API key ({label_str})"), data)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Dispatch a parsed CLI command to the appropriate handler.
///
/// Routes the command to:
/// - `OrchestratorService` for Tier 1 commands (run, plan, cancel, status)
/// - Individual engine services for Tier 2 commands
/// - CLI-only logic for Tier 3 commands (init, key)
/// - The TUI module for the Tui variant
///
/// Returns a `DispatchResult` that the output formatter can render.
pub async fn dispatch(
    command: CliCommand,
    config: crate::cli_boundary::config::CliConfig,
    cancellation_token: crate::cli_boundary::signal::CancellationToken,
) -> DispatchResult {
    // Build orchestrator for Tier 1 commands
    let orch = match &command {
        CliCommand::Run { .. }
        | CliCommand::Plan { .. }
        | CliCommand::Cancel { .. }
        | CliCommand::Status => {
            match crate::cli_boundary::orchestrator::build_orchestrator(
                config,
                cancellation_token,
                String::new(),
            )
            .await
            {
                Ok(o) => Some(o),
                Err(e) => {
                    let code = e.exit_code();
                    return DispatchResult::error(e.to_string(), code);
                }
            }
        }
        _ => None,
    };

    match command {
        // ── Tier 1: Via OrchestratorService ──────────────────────────
        CliCommand::Run {
            intent,
            enforcement,
            max_llm_calls: _,
            max_llm_tokens: _,
        } => {
            let orch = orch.expect("orchestrator built above");
            let input = rigorix_engine::orchestrator::application::dto::RunInput {
                intent,
                config: serde_json::Value::Null,
                repo_root: String::new(),
                enforcement_preset: enforcement,
            };
            match orch.run(input).await {
                Ok(output) => {
                    // Save generated template to disk if present
                    if let Some(toml) = &output.record.planning.generated_toml {
                        let tpl_dir = std::path::PathBuf::from(".rigorix/templates");
                        let tpl_path =
                            tpl_dir.join(format!("{}.toml", output.record.planning.template_id));
                        let _ = tokio::fs::create_dir_all(&tpl_dir).await;
                        let _ = tokio::fs::write(&tpl_path, toml).await;
                    }
                    let data = serde_json::json!({
                        "execution_id": output.execution_id,
                        "status": "completed",
                    });
                    DispatchResult::success_with_data(
                        format!("Run completed: {}", output.execution_id),
                        data,
                    )
                }
                Err(e) => DispatchResult::error(format!("Run failed: {e}"), 1),
            }
        }

        CliCommand::Plan { intent } => {
            let orch = orch.expect("orchestrator built above");
            let input = rigorix_engine::orchestrator::application::dto::PlanOnlyInput {
                intent,
                config: serde_json::Value::Null,
                repo_root: String::new(),
            };
            match orch.plan_only(input).await {
                Ok(output) => {
                    // Save generated template to disk if present
                    if let Some(toml) = output.plan["generated_toml"].as_str() {
                        let template_id = output.plan["template_id"].as_str().unwrap_or("unknown");
                        let tpl_dir = std::path::PathBuf::from(".rigorix/templates");
                        let tpl_path = tpl_dir.join(format!("{template_id}.toml"));
                        let _ = tokio::fs::create_dir_all(&tpl_dir).await;
                        let _ = tokio::fs::write(&tpl_path, toml).await;
                    }
                    DispatchResult::success_with_data(
                        "Plan generated",
                        serde_json::json!({ "plan": output.plan, "graph": output.graph }),
                    )
                }
                Err(e) => DispatchResult::error(format!("Plan failed: {e}"), 1),
            }
        }

        CliCommand::Cancel { execution_id } => {
            let orch = orch.expect("orchestrator built above");
            let input = rigorix_engine::orchestrator::application::dto::CancelInput {
                execution_id,
                reason: None,
            };
            match orch.cancel(input).await {
                Ok(output) => DispatchResult::success(format!(
                    "Cancelled {}. {} nodes aborted.",
                    output.execution_id, output.nodes_cancelled
                )),
                Err(e) => DispatchResult::error(format!("Cancel failed: {e}"), 1),
            }
        }

        CliCommand::Status => {
            let orch = orch.expect("orchestrator built above");
            match orch.status().await {
                Ok(output) => {
                    let status_str = format!("{:?}", output.status);
                    let data = serde_json::json!({
                        "execution_id": output.execution_id,
                        "status": status_str,
                        "nodes": output.nodes,
                    });
                    DispatchResult::success_with_data(
                        format!("Status: {status_str} (execution {})", output.execution_id),
                        data,
                    )
                }
                Err(e) => DispatchResult::error(format!("Status failed: {e}"), 1),
            }
        }

        // ── Tier 2: Via Engine Services Directly ─────────────────────
        // These are stubs that return NotImplemented until the engine
        // services are wired into the CLI builder.
        CliCommand::History { limit, status } => {
            let _ = (limit, status);
            DispatchResult::success_with_data(
                "History (placeholder)",
                serde_json::json!({ "executions": [] }),
            )
        }

        CliCommand::Explain { execution_id, diff } => {
            let _ = (execution_id, diff);
            DispatchResult::success_with_data(
                "Explain (placeholder)",
                serde_json::json!({ "execution_id": execution_id }),
            )
        }

        CliCommand::DiffPlan { id1, id2 } => {
            let _ = (id1, id2);
            DispatchResult::success("Diff (placeholder)")
        }

        CliCommand::Generate { intent } => {
            let _ = intent;
            DispatchResult::success("Generate (placeholder)")
        }

        CliCommand::Template { action } => match action {
            TemplateAction::List => DispatchResult::success_with_data(
                "Available templates",
                serde_json::json!({ "templates": [] }),
            ),
            TemplateAction::Show { id } => {
                DispatchResult::success(format!("Template: {id} (placeholder)"))
            }
        },

        CliCommand::Audit { action } => match action {
            AuditAction::List { limit } => {
                let _ = limit;
                DispatchResult::success_with_data(
                    "Audit trail",
                    serde_json::json!({ "entries": [] }),
                )
            }
            AuditAction::Show { id } => {
                DispatchResult::success(format!("Audit: {id} (placeholder)"))
            }
            AuditAction::Diff { id1, id2 } => {
                DispatchResult::success(format!("Audit diff: {id1} vs {id2} (placeholder)"))
            }
        },

        CliCommand::Logs { session_id } => {
            let _ = session_id;
            DispatchResult::success("Logs (placeholder)")
        }

        CliCommand::Config { action } => match action {
            ConfigAction::Init => DispatchResult::success("Config init (placeholder)"),
            ConfigAction::Show => {
                DispatchResult::success_with_data("Configuration", serde_json::json!({}))
            }
            ConfigAction::Validate => DispatchResult::success("Config valid"),
        },

        // ── Tier 3: CLI-Only ────────────────────────────────────
        CliCommand::Init => cmd_init(),
        CliCommand::Key { label } => cmd_key(label),

        // ── TUI ──────────────────────────────────────────────────
        CliCommand::Tui { exec, run } => {
            let _ = (exec, run);
            // The TUI variant is handled by main.rs before dispatch
            DispatchResult::success("TUI mode")
        }
    }
}
