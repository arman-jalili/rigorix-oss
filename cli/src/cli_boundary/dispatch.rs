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

use crate::cli_boundary::cli::{CliCommand, ConfigAction};

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
    // Build CLI services for Tier 2 commands (cheap, no LLM key needed).
    // Also used by Tier 3 (init, key).
    let services = match crate::cli_boundary::orchestrator::build_cli_services(config.clone()).await
    {
        Ok(s) => Some(s),
        Err(e) => {
            let code = e.exit_code();
            return DispatchResult::error(e.to_string(), code);
        }
    };

    // Build full orchestrator for commands that need LLM (Tier 1 + generate + diff-plan).
    // Also keep the services reference for template recovery on generate.
    let (orch, llm_services): (Option<_>, Option<_>) = match &command {
        CliCommand::Run { .. }
        | CliCommand::Plan { .. }
        | CliCommand::Cancel { .. }
        | CliCommand::Status
        | CliCommand::Generate { .. }
        | CliCommand::DiffPlan { .. } => {
            match crate::cli_boundary::orchestrator::build_orchestrator(
                config,
                cancellation_token,
                String::new(),
            )
            .await
            {
                Ok((o, svc)) => (Some(o), Some(svc)),
                Err(e) => {
                    let code = e.exit_code();
                    return DispatchResult::error(e.to_string(), code);
                }
            }
        }
        _ => (None, None),
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
                repo_root: std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
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

                    let status = format!("{:?}", output.record.status);
                    let task_results = &output.record.task_results;
                    use rigorix_engine::orchestrator::domain::record::TaskStatus;
                    let fail_count = task_results
                        .iter()
                        .filter(|t| matches!(t.status, TaskStatus::Failure))
                        .count();
                    let pass_count = task_results
                        .iter()
                        .filter(|t| matches!(t.status, TaskStatus::Success))
                        .count();
                    let skip_count = task_results
                        .iter()
                        .filter(|t| matches!(t.status, TaskStatus::Skipped))
                        .count();

                    let llm_calls = output.record.planning.llm_calls;
                    let llm_tokens = output.record.planning.total_tokens;
                    let template_id = &output.record.planning.template_id;
                    let node_order = &output.record.planning.node_order;

                    // Sort task results by topological DAG order
                    let mut indexed: Vec<_> = task_results.iter().collect();
                    indexed.sort_by_key(|t| {
                        node_order
                            .iter()
                            .position(|n| n == &t.node_name)
                            .unwrap_or(usize::MAX)
                    });

                    let mut summary = format!(
                        "Run: {} — {} failed, {} passed, {} skipped ({} total)\n",
                        status, fail_count, pass_count, skip_count, task_results.len()
                    );
                    summary.push_str(&format!(
                        "  Template: {} | LLM: {} calls, {} tokens\n",
                        template_id, llm_calls, llm_tokens
                    ));

                    for task in &indexed {
                        let icon = match task.status {
                            TaskStatus::Success => "  ✓",
                            TaskStatus::Failure => "  ✗",
                            _ => "  ○",
                        };
                        summary.push_str(&format!("\n{} {} — {:?}", icon, task.node_name, task.status));
                        if let Some(ref err) = task.error {
                            summary.push_str(&format!("\n     Error: {}", err));
                        }
                    }

                    DispatchResult::success(summary)
                }
                Err(e) => DispatchResult::error(format!("Run failed: {e}"), 1),
            }
        }

        CliCommand::Plan { intent } => {
            let intent_str = intent.clone();
            let orch = orch.expect("orchestrator built above");
            let input = rigorix_engine::orchestrator::application::dto::PlanOnlyInput {
                intent,
                config: serde_json::Value::Null,
                repo_root: std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
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

                    let tpl_id = output.plan["template_id"].as_str().unwrap_or("?");
                    let confidence = output.plan["confidence"].as_f64().unwrap_or(0.0);
                    let llm_calls = output.plan["llm_calls_used"].as_u64().unwrap_or(0);
                    let llm_tokens = output.plan["llm_tokens_used"].as_u64().unwrap_or(0);
                    let params = output.plan["parameters"]
                        .as_object()
                        .map(|m| {
                            m.iter()
                                .map(|(k, v)| format!("    ├── {}: {}", k, v))
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                        .unwrap_or_else(|| "    (none)".to_string());

                    // Build node list from graph
                    let nodes_arr = output.graph["nodes"].as_array();
                    let node_count = nodes_arr.map(|a| a.len()).unwrap_or(0);
                    let node_lines = nodes_arr
                        .map(|a| {
                            a.iter()
                                .map(|n| {
                                    let name = n["name"].as_str().unwrap_or("?");
                                    let deps = n["dependencies"]
                                        .as_array()
                                        .map(|d| {
                                            d.iter()
                                                .filter_map(|d| {
                                                    // Resolve dep names from IDs
                                                    a.iter().find_map(|n2| {
                                                        if n2["id"] == *d {
                                                            n2["name"].as_str().map(|s| s.to_string())
                                                        } else {
                                                            None
                                                        }
                                                    })
                                                })
                                                .collect::<Vec<_>>()
                                        })
                                        .unwrap_or_default();
                                    if deps.is_empty() {
                                        format!("    · {} (root)", name)
                                    } else {
                                        format!("    · {} ← [{}]", name, deps.join(", "))
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                        .unwrap_or_default();
                    let sealed = output.graph["sealed"]
                        .as_bool()
                        .unwrap_or(false);

                    let summary = format!(
                        "Plan: {} (confidence {:.0}%)\n  Template: {} | LLM: {} calls, {} tokens\n  Parameters:\n{}\n  Graph: {} node(s), sealed={}\n{}",
                        intent_str, confidence * 100.0, tpl_id, llm_calls, llm_tokens,
                        params, node_count, sealed, node_lines
                    );

                    DispatchResult::success(summary)
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

        // ── Tier 2: Via CliServices ───────────────────────────────────
        CliCommand::History { limit, status: _ } => {
            let svc = services.as_ref().expect("services built above");
            let input = rigorix_engine::state_persistence::application::dto::ListExecutionsInput {
                limit: limit.map(|n| n.min(100)),
                ..Default::default()
            };
            match svc.state_manager.list_executions(input).await {
                Ok(output) => {
                    let data = serde_json::json!({
                        "executions": output.executions,
                        "total_count": output.total_count,
                    });
                    DispatchResult::success_with_data(
                        format!("{} execution(s)", output.total_count),
                        data,
                    )
                }
                Err(e) => DispatchResult::error(format!("history: {e}"), 1),
            }
        }

        CliCommand::Explain {
            execution_id,
            diff: _,
        } => {
            let svc = services.as_ref().expect("services built above");
            let input = rigorix_engine::state_persistence::application::dto::LoadStateInput {
                execution_id,
            };
            match svc.state_manager.load_state(input).await {
                Ok(output) => {
                    let summary = format!(
                        "Execution {}: {:?} — {} node(s)",
                        execution_id, output.state.status, output.state.node_states.len()
                    );
                    let data = serde_json::json!({ "state": output.state });
                    DispatchResult::success_with_data(summary, data)
                }
                Err(e) => DispatchResult::error(format!("explain: {e}"), 1),
            }
        }

        CliCommand::DiffPlan { id1: _, id2: _ } => {
            DispatchResult::error(
                "diff-plan: requires DagPlanningService (not yet exposed via CliServices)",
                1,
            )
        }

        CliCommand::Generate { intent } => {
            let orch = orch.as_ref().expect("orchestrator built above");
            let input = rigorix_engine::orchestrator::application::dto::PlanOnlyInput {
                intent,
                config: serde_json::Value::Null,
                repo_root: std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
            };
            match orch.plan_only(input).await {
                Ok(output) => {
                    let tpl_id = output.plan["template_id"].as_str().unwrap_or("generated");
                    let toml_str = output.plan["generated_toml"].as_str().unwrap_or("");
                    if toml_str.is_empty() {
                        return DispatchResult::error(
                            "generate: LLM did not produce a template. Try a more specific intent.",
                            1,
                        );
                    }
                    let tpl_dir = std::path::PathBuf::from(".rigorix/templates");
                    let tpl_path = tpl_dir.join(format!("{tpl_id}.toml"));
                    let _ = tokio::fs::create_dir_all(&tpl_dir).await;
                    if let Err(e) = tokio::fs::write(&tpl_path, toml_str).await {
                        return DispatchResult::error(format!("generate: save failed: {e}"), 1);
                    }
                    let data = serde_json::json!({
                        "template_id": tpl_id,
                        "saved_to": tpl_path.to_string_lossy(),
                        "toml": toml_str,
                    });
                    DispatchResult::success_with_data(
                        format!("Generated template '{tpl_id}' → {}", tpl_path.display()),
                        data,
                    )
                }
                Err(e) => {
                    // The pipeline may have generated and registered a template
                    // before failing on graph generation (e.g. missing parameters).
                    let err_msg = e.to_string();
                    // Use the orchestrator's services — the template was registered there
                    let svc = llm_services.as_ref().or(services.as_ref())
                        .expect("services built above");
                    if let Ok(list) = svc.template_service.list_templates().await {
                        if let Some(summary) = list.templates.first() {
                            let tpl_id = summary.id.clone();
                            let tpl_dir = std::path::PathBuf::from(".rigorix/templates");
                            let tpl_path = tpl_dir.join(format!("{tpl_id}.toml"));
                            let _ = tokio::fs::create_dir_all(&tpl_dir).await;

                            // Try to get the full template and serialize to TOML
                            if let Some(full_template) =
                                svc.template_service.get_template_full(&tpl_id).await
                            {
                                if let Ok(toml_str) = toml::to_string_pretty(&full_template) {
                                    let _ = tokio::fs::write(&tpl_path, &toml_str).await;
                                }
                            }

                            let data = serde_json::json!({
                                "template_id": summary.id,
                                "name": summary.name,
                                "description": summary.description,
                                "param_count": summary.param_count,
                                "saved_to": tpl_path.to_string_lossy(),
                                "note": format!("graph generation incomplete: {err_msg}"),
                            });
                            return DispatchResult::success_with_data(
                                format!(
                                    "Generated template '{}' ({}) — {} param(s). Graph pending.",
                                    summary.id, summary.name, summary.param_count,
                                ),
                                data,
                            );
                        }
                    }
                    DispatchResult::error(format!("generate: {err_msg}"), 1)
                }
            }
        }

        CliCommand::Template { action } => {
            let svc = llm_services.as_ref().or(services.as_ref())
                .expect("services built above");
            match action {
                crate::cli_boundary::cli::TemplateAction::List => {
                    match svc.template_service.list_templates().await {
                        Ok(output) => {
                            let data =
                                serde_json::json!({ "templates": output.templates });
                            DispatchResult::success_with_data(
                                format!("{} template(s)", output.templates.len()),
                                data,
                            )
                        }
                        Err(e) => {
                            DispatchResult::error(format!("template list: {e}"), 1)
                        }
                    }
                }
                crate::cli_boundary::cli::TemplateAction::Show { id } => {
                    let input = rigorix_engine::templates::application::dto::GetTemplateInput {
                        template_id: id.clone(),
                    };
                    match svc.template_service.get_template(input).await {
                        Ok(Some(summary)) => {
                            let data = serde_json::json!({ "template": summary });
                            DispatchResult::success_with_data(
                                format!("Template: {id}"),
                                data,
                            )
                        }
                        Ok(None) => DispatchResult::error(
                            format!("template not found: {id}"),
                            1,
                        ),
                        Err(e) => {
                            DispatchResult::error(format!("template show: {e}"), 1)
                        }
                    }
                }
            }
        }

        CliCommand::Audit { action: _ } => {
            DispatchResult::error(
                "audit: AuditService does not yet expose list/show/diff queries",
                1,
            )
        }

        CliCommand::Logs { session_id: _ } => {
            DispatchResult::error(
                "logs: EventBusService not yet exposed via CliServices for log replay",
                1,
            )
        }

        CliCommand::Config { action } => {
            let svc = services.as_ref().expect("services built above");
            match action {
                ConfigAction::Init => cmd_init(),
                ConfigAction::Show => {
                    let data = serde_json::json!({ "config": svc.config });
                    DispatchResult::success_with_data("Current configuration", data)
                }
                ConfigAction::Validate => {
                    match svc.config.engine_config() {
                        Ok(ec) => DispatchResult::success(format!(
                            "Config valid: {} parallel tasks, LLM provider={:?}, model={}",
                            ec.orchestrator.max_parallel_tasks,
                            ec.llm.provider,
                            ec.llm.model,
                        )),
                        Err(e) => DispatchResult::error(format!("config invalid: {e}"), 1),
                    }
                }
            }
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
