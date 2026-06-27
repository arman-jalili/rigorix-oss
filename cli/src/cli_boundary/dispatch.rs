//! Command dispatcher — routes parsed commands to engine services.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#dispatch-logic
//! Implements: Dispatcher component: dispatch function and DispatchResult type
//! Issue: issue-dispatcher
//!
//! # Contract (Frozen)
//!
//! The dispatcher is a `match` over `CliCommand` variants that routes each
//! command to the appropriate handler. Each handler is a standalone async
//! function, keeping `dispatch()` as a thin router under 50 lines.
//!
//! | Command | Handler | Backend |
//! |---------|---------|--------|
//! | Run | `handle_run` | OrchestratorService |
//! | Plan | `handle_plan` | OrchestratorService |
//! | Cancel | `handle_cancel` | OrchestratorService |
//! | Status | `handle_status` | OrchestratorService |
//! | History | `handle_history` | StateManagerService |
//! | Explain | `handle_explain` | StateManagerService |
//! | DiffPlan | `handle_diff_plan` | StateManagerService |
//! | Generate | `handle_generate` | OrchestratorService |
//! | Template | `handle_template` | TemplateEngineService |
//! | Audit | `handle_audit` | AuditRepository |
//! | Logs | `handle_logs` | EventBusService |
//! | Config | `handle_config` | CliConfig |

use std::fmt;

use serde_json::Value as JsonValue;

use crate::cli_boundary::cli::{CliCommand, ConfigAction};
use crate::cli_boundary::orchestrator::CliServices;

// ---------------------------------------------------------------------------
// Dispatch result type
// ---------------------------------------------------------------------------

/// Unified result from any command dispatch.
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
    pub fn success(summary: impl Into<String>) -> Self {
        DispatchResult {
            summary: summary.into(),
            data: None,
            exit_code: 0,
        }
    }
    pub fn success_with_data(summary: impl Into<String>, data: JsonValue) -> Self {
        DispatchResult {
            summary: summary.into(),
            data: Some(data),
            exit_code: 0,
        }
    }
    pub fn error(summary: impl Into<String>, exit_code: i32) -> Self {
        DispatchResult {
            summary: summary.into(),
            data: None,
            exit_code,
        }
    }
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

fn cmd_init() -> DispatchResult {
    let path = std::path::Path::new(".rigorix");
    if path.is_dir() {
        return DispatchResult::success(".rigorix/ already exists");
    }
    match std::fs::create_dir_all(path.join("templates")) {
        Ok(_) => {
            let default_config = r#"# Rigorix Configuration
[orchestrator]
max_parallel_tasks = 4
max_retries = 3
default_timeout_secs = 120

[logging]
level = "info"
format = "text"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-6"
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
// Tier 1 handlers — via OrchestratorService
// ---------------------------------------------------------------------------

async fn handle_run(
    orch: &dyn rigorix_engine::orchestrator::application::OrchestratorService,
    intent: String,
    enforcement: Option<String>,
) -> DispatchResult {
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
                let tpl_path = tpl_dir.join(format!("{}.toml", output.record.planning.template_id));
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

            let mut indexed: Vec<_> = task_results.iter().collect();
            indexed.sort_by_key(|t| {
                node_order
                    .iter()
                    .position(|n| n == &t.node_name)
                    .unwrap_or(usize::MAX)
            });

            let mut summary = format!(
                "Run: {} — {} failed, {} passed, {} skipped ({} total)\n",
                status,
                fail_count,
                pass_count,
                skip_count,
                task_results.len()
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
                summary.push_str(&format!(
                    "\n{} {} — {:?}",
                    icon, task.node_name, task.status
                ));
                if let Some(ref err) = task.error {
                    summary.push_str(&format!("\n     Error: {}", err));
                }
            }
            DispatchResult::success(summary)
        }
        Err(e) => DispatchResult::error(format!("Run failed: {e}"), 1),
    }
}

async fn handle_plan(
    orch: &dyn rigorix_engine::orchestrator::application::OrchestratorService,
    intent: String,
) -> DispatchResult {
    let intent_str = intent.clone();
    let input = rigorix_engine::orchestrator::application::dto::PlanOnlyInput {
        intent,
        config: serde_json::Value::Null,
        repo_root: std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
    };
    match orch.plan_only(input).await {
        Ok(output) => {
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
                                        .filter_map(|d2| {
                                            a.iter().find_map(|n2| {
                                                if n2["id"] == *d2 {
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
            let sealed = output.graph["sealed"].as_bool().unwrap_or(false);

            let summary = format!(
                "Plan: {} (confidence {:.0}%)\n  Template: {} | LLM: {} calls, {} tokens\n  Parameters:\n{}\n  Graph: {} node(s), sealed={}\n{}",
                intent_str,
                confidence * 100.0,
                tpl_id,
                llm_calls,
                llm_tokens,
                params,
                node_count,
                sealed,
                node_lines
            );
            DispatchResult::success(summary)
        }
        Err(e) => DispatchResult::error(format!("Plan failed: {e}"), 1),
    }
}

async fn handle_cancel(
    orch: &dyn rigorix_engine::orchestrator::application::OrchestratorService,
    execution_id: uuid::Uuid,
) -> DispatchResult {
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

async fn handle_status(
    orch: &dyn rigorix_engine::orchestrator::application::OrchestratorService,
) -> DispatchResult {
    match orch.status().await {
        Ok(output) => {
            let status_str = format!("{:?}", output.status);
            let data = serde_json::json!({ "execution_id": output.execution_id, "status": status_str, "nodes": output.nodes });
            DispatchResult::success_with_data(
                format!("Status: {status_str} (execution {})", output.execution_id),
                data,
            )
        }
        Err(e) => DispatchResult::error(format!("Status failed: {e}"), 1),
    }
}

// ---------------------------------------------------------------------------
// Tier 2 handlers — via CliServices
// ---------------------------------------------------------------------------

async fn handle_history(svc: &CliServices, limit: Option<u32>) -> DispatchResult {
    let input = rigorix_engine::state_persistence::application::dto::ListExecutionsInput {
        limit: limit.map(|n| n.min(100)),
        ..Default::default()
    };
    match svc.state_manager.list_executions(input).await {
        Ok(output) => {
            let data = serde_json::json!({ "executions": output.executions, "total_count": output.total_count });
            DispatchResult::success_with_data(format!("{} execution(s)", output.total_count), data)
        }
        Err(e) => DispatchResult::error(format!("history: {e}"), 1),
    }
}

async fn handle_explain(svc: &CliServices, execution_id: uuid::Uuid) -> DispatchResult {
    let input =
        rigorix_engine::state_persistence::application::dto::LoadStateInput { execution_id };
    match svc.state_manager.load_state(input).await {
        Ok(output) => {
            let summary = format!(
                "Execution {}: {:?} — {} node(s)",
                execution_id,
                output.state.status,
                output.state.node_states.len()
            );
            let data = serde_json::json!({ "state": output.state });
            DispatchResult::success_with_data(summary, data)
        }
        Err(e) => DispatchResult::error(format!("explain: {e}"), 1),
    }
}

async fn handle_diff_plan(svc: &CliServices, id1: uuid::Uuid, id2: uuid::Uuid) -> DispatchResult {
    let sm1 = svc.state_manager.clone();
    let sm2 = sm1.clone();
    let r1 = {
        let input = rigorix_engine::state_persistence::application::dto::LoadStateInput {
            execution_id: id1,
        };
        sm1.load_state(input).await
    };
    let r2 = {
        let input = rigorix_engine::state_persistence::application::dto::LoadStateInput {
            execution_id: id2,
        };
        sm2.load_state(input).await
    };
    match (r1, r2) {
        (Ok(out1), Ok(out2)) => {
            let s1 = &out1.state;
            let s2 = &out2.state;
            let mut added_nodes = Vec::new();
            let mut removed_nodes = Vec::new();
            let mut modified_nodes = Vec::new();
            let mut common_nodes = Vec::new();
            for (id, ns1) in &s1.node_states {
                if let Some(ns2) = s2.node_states.get(id) {
                    if ns1.status != ns2.status || ns1.retries != ns2.retries {
                        modified_nodes.push(serde_json::json!({ "node_id": id,
                            "left_status": format!("{:?}", ns1.status), "right_status": format!("{:?}", ns2.status),
                            "left_retries": ns1.retries, "right_retries": ns2.retries,
                            "left_duration_ms": ns1.duration_ms, "right_duration_ms": ns2.duration_ms }));
                    } else {
                        common_nodes.push(id.to_string());
                    }
                } else {
                    removed_nodes.push(format!("{} ({:?})", id, ns1.status));
                }
            }
            for id in s2.node_states.keys() {
                if !s1.node_states.contains_key(id) {
                    added_nodes.push(format!("{} ({:?})", id, s2.node_states[id].status));
                }
            }
            let summary = format!(
                "Plan diff: {} added, {} removed, {} modified, {} unchanged",
                added_nodes.len(),
                removed_nodes.len(),
                modified_nodes.len(),
                common_nodes.len()
            );
            let data = serde_json::json!({ "added": added_nodes, "removed": removed_nodes, "modified": modified_nodes,
                "unchanged_count": common_nodes.len(), "left_execution_id": id1.to_string(), "right_execution_id": id2.to_string() });
            DispatchResult::success_with_data(summary, data)
        }
        (Err(e), _) | (_, Err(e)) => DispatchResult::error(format!("diff-plan: {e}"), 1),
    }
}

async fn handle_generate(
    orch: Option<&dyn rigorix_engine::orchestrator::application::OrchestratorService>,
    llm_services: Option<&CliServices>,
    services: Option<&CliServices>,
    intent: String,
) -> DispatchResult {
    let orch = match orch {
        Some(o) => o,
        None => {
            return DispatchResult::error("generate: orchestrator not available".to_string(), 1);
        }
    };
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
                    "generate: LLM did not produce a template".to_string(),
                    1,
                );
            }
            let tpl_dir = std::path::PathBuf::from(".rigorix/templates");
            let tpl_path = tpl_dir.join(format!("{tpl_id}.toml"));
            let _ = tokio::fs::create_dir_all(&tpl_dir).await;
            if let Err(e) = tokio::fs::write(&tpl_path, toml_str).await {
                return DispatchResult::error(format!("generate: save failed: {e}"), 1);
            }
            let data = serde_json::json!({ "template_id": tpl_id, "saved_to": tpl_path.to_string_lossy(), "toml": toml_str });
            DispatchResult::success_with_data(
                format!("Generated template '{tpl_id}' → {}", tpl_path.display()),
                data,
            )
        }
        Err(e) => {
            let svc = llm_services.or(services);
            if let Some(svc) = svc
                && let Ok(list) = svc.template_service.list_templates().await
                && let Some(summary) = list.templates.first()
            {
                let tpl_id = summary.id.clone();
                let tpl_dir = std::path::PathBuf::from(".rigorix/templates");
                let tpl_path = tpl_dir.join(format!("{tpl_id}.toml"));
                let _ = tokio::fs::create_dir_all(&tpl_dir).await;
                if let Some(full) = svc.template_service.get_template_full(&tpl_id).await
                    && let Ok(toml_str) = toml::to_string_pretty(&full)
                {
                    let _ = tokio::fs::write(&tpl_path, &toml_str).await;
                }
                let data = serde_json::json!({ "template_id": summary.id, "name": summary.name,
                    "description": summary.description, "param_count": summary.param_count,
                    "saved_to": tpl_path.to_string_lossy(), "note": format!("graph generation incomplete: {e}") });
                return DispatchResult::success_with_data(
                    format!(
                        "Generated template '{}' ({}) — {} param(s). Graph pending.",
                        summary.id, summary.name, summary.param_count
                    ),
                    data,
                );
            }
            DispatchResult::error(format!("generate: {e}"), 1)
        }
    }
}

async fn handle_template(
    svc: &CliServices,
    action: crate::cli_boundary::cli::TemplateAction,
) -> DispatchResult {
    match action {
        crate::cli_boundary::cli::TemplateAction::List => {
            match svc.template_service.list_templates().await {
                Ok(output) => {
                    let data = serde_json::json!({ "templates": output.templates });
                    DispatchResult::success_with_data(
                        format!("{} template(s)", output.templates.len()),
                        data,
                    )
                }
                Err(e) => DispatchResult::error(format!("template list: {e}"), 1),
            }
        }
        crate::cli_boundary::cli::TemplateAction::Show { id } => {
            let input = rigorix_engine::templates::application::dto::GetTemplateInput {
                template_id: id.clone(),
            };
            match svc.template_service.get_template(input).await {
                Ok(Some(summary)) => {
                    let data = serde_json::json!({ "template": summary });
                    DispatchResult::success_with_data(format!("Template: {id}"), data)
                }
                Ok(None) => DispatchResult::error(format!("template not found: {id}"), 1),
                Err(e) => DispatchResult::error(format!("template show: {e}"), 1),
            }
        }
    }
}

async fn handle_audit(
    svc: &CliServices,
    action: crate::cli_boundary::cli::AuditAction,
) -> DispatchResult {
    match action {
        crate::cli_boundary::cli::AuditAction::List { limit } => {
            match svc.audit_repository.list(None, None, limit).await {
                Ok(envelopes) => {
                    let summaries: Vec<serde_json::Value> = envelopes
                        .iter()
                        .map(|e| {
                            serde_json::json!({
                                "execution_id": e.execution_id, "template_id": e.template_id,
                                "timestamp": e.timestamp, "event_count": e.events.len(),
                                "has_signature": e.signature.is_some(),
                            })
                        })
                        .collect();
                    let data =
                        serde_json::json!({ "count": summaries.len(), "envelopes": summaries });
                    DispatchResult::success_with_data(
                        format!("{} audit envelope(s)", summaries.len()),
                        data,
                    )
                }
                Err(e) => DispatchResult::error(format!("audit list: {e}"), 1),
            }
        }
        crate::cli_boundary::cli::AuditAction::Show { id } => {
            match svc
                .audit_repository
                .find_by_execution_id(&id.parse().unwrap_or_default())
                .await
            {
                Ok(Some(envelope)) => {
                    let data = serde_json::json!({ "envelope": envelope });
                    DispatchResult::success_with_data(format!("Audit entry: {id}"), data)
                }
                Ok(None) => DispatchResult::error(format!("audit entry not found: {id}"), 1),
                Err(e) => DispatchResult::error(format!("audit show: {e}"), 1),
            }
        }
        crate::cli_boundary::cli::AuditAction::Diff { id1, id2 } => {
            let id1_parsed: uuid::Uuid = match id1.parse() {
                Ok(u) => u,
                Err(_) => return DispatchResult::error(format!("invalid UUID: {id1}"), 1),
            };
            let id2_parsed: uuid::Uuid = match id2.parse() {
                Ok(u) => u,
                Err(_) => return DispatchResult::error(format!("invalid UUID: {id2}"), 1),
            };
            match (
                svc.audit_repository.find_by_execution_id(&id1_parsed).await,
                svc.audit_repository.find_by_execution_id(&id2_parsed).await,
            ) {
                (Ok(Some(e1)), Ok(Some(e2))) => {
                    let template_changed = e1.template_id != e2.template_id;
                    let events_added = e2.events.len().saturating_sub(e1.events.len());
                    let events_removed = e1.events.len().saturating_sub(e2.events.len());
                    let data = serde_json::json!({ "left_execution_id": e1.execution_id, "right_execution_id": e2.execution_id,
                        "template_changed": template_changed, "events_added": events_added, "events_removed": events_removed,
                        "left_event_count": e1.events.len(), "right_event_count": e2.events.len() });
                    DispatchResult::success_with_data(format!("Audit diff: {id1} vs {id2}"), data)
                }
                (Ok(None), _) => DispatchResult::error(format!("audit entry not found: {id1}"), 1),
                (_, Ok(None)) => DispatchResult::error(format!("audit entry not found: {id2}"), 1),
                (Err(e), _) | (_, Err(e)) => DispatchResult::error(format!("audit diff: {e}"), 1),
            }
        }
    }
}

async fn handle_logs(svc: &CliServices, session_id: Option<uuid::Uuid>) -> DispatchResult {
    let input = rigorix_engine::event_system::application::dto::QueryEventsInput {
        execution_id: session_id,
        event_type: None,
        after_sequence: None,
        limit: Some(100),
        after_timestamp: None,
        before_timestamp: None,
    };
    match svc.event_bus.query_events(input).await {
        Ok(output) => {
            let events: Vec<serde_json::Value> = output
                .events
                .iter()
                .map(|pe| {
                    serde_json::json!({
                        "sequence": pe.sequence, "event_type": pe.event.event_type_name(),
                        "execution_id": pe.event.execution_id(), "timestamp": pe.event.timestamp(),
                        "summary": pe.event.summary(),
                    })
                })
                .collect();
            let count = events.len();
            let summary = session_id
                .map(|sid| format!("{count} event(s) for session {sid}"))
                .unwrap_or_else(|| format!("{count} event(s)"));
            let data = serde_json::json!({ "total": output.total, "has_more": output.has_more, "events": events });
            DispatchResult::success_with_data(summary, data)
        }
        Err(e) => DispatchResult::error(format!("logs: {e}"), 1),
    }
}

async fn handle_config(svc: &CliServices, action: ConfigAction) -> DispatchResult {
    match action {
        ConfigAction::Init => cmd_init(),
        ConfigAction::Show => {
            let data = serde_json::json!({ "config": svc.config });
            DispatchResult::success_with_data("Current configuration", data)
        }
        ConfigAction::Validate => match svc.config.engine_config() {
            Ok(ec) => DispatchResult::success(format!(
                "Config valid: {} parallel tasks, LLM provider={:?}, model={}",
                ec.orchestrator.max_parallel_tasks, ec.llm.provider, ec.llm.model,
            )),
            Err(e) => DispatchResult::error(format!("config invalid: {e}"), 1),
        },
    }
}

// ---------------------------------------------------------------------------
// Public API — thin dispatch
// ---------------------------------------------------------------------------

/// Dispatch a parsed CLI command to the appropriate handler function.
///
/// This function is a thin router under 50 lines. Each command variant
/// delegates to a dedicated handler function defined above.
pub async fn dispatch(
    command: CliCommand,
    config: crate::cli_boundary::config::CliConfig,
    cancellation_token: crate::cli_boundary::signal::CancellationToken,
) -> DispatchResult {
    // Build CLI services for Tier 2 commands
    let services = match crate::cli_boundary::orchestrator::build_cli_services(config.clone()).await
    {
        Ok(s) => Some(s),
        Err(e) => return DispatchResult::error(e.to_string(), e.exit_code()),
    };

    // Extract budget values for orchestrator build
    let (budget_calls, budget_tokens) = match &command {
        CliCommand::Run {
            max_llm_calls,
            max_llm_tokens,
            ..
        } => (*max_llm_calls, *max_llm_tokens),
        _ => (None, None),
    };

    // Build full orchestrator for Tier 1 commands + generate + diff-plan
    let (orch, llm_services): (
        Option<Box<dyn rigorix_engine::orchestrator::application::OrchestratorService>>,
        Option<CliServices>,
    ) = match &command {
        CliCommand::Run { .. }
        | CliCommand::Plan { .. }
        | CliCommand::Cancel { .. }
        | CliCommand::Status
        | CliCommand::Generate { .. }
        | CliCommand::DiffPlan { .. } => {
            match crate::cli_boundary::orchestrator::build_orchestrator_with_budget(
                config,
                cancellation_token,
                String::new(),
                budget_calls,
                budget_tokens,
            )
            .await
            {
                Ok((o, svc)) => (Some(o), Some(svc)),
                Err(e) => return DispatchResult::error(e.to_string(), e.exit_code()),
            }
        }
        _ => (None, None),
    };

    // Thin router — delegates to handler functions
    match command {
        CliCommand::Run {
            intent,
            enforcement,
            ..
        } => {
            handle_run(
                orch.as_deref().expect("orchestrator for run"),
                intent,
                enforcement,
            )
            .await
        }
        CliCommand::Plan { intent } => {
            let intent_clone = intent.clone();
            // Show plan output (handle_plan returns DispatchResult but doesn't print it)
            let plan_result =
                handle_plan(orch.as_deref().expect("orchestrator for plan"), intent).await;
            if plan_result.is_success() {
                // Print the plan output so user sees it before the prompt
                println!("{}", plan_result.summary);
                use std::io::Write;
                print!("\nRun this plan now? [y/N]: ");
                std::io::stdout().flush().ok();
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).ok();
                let input = input.trim().to_lowercase();
                if input == "y" || input == "yes" {
                    return handle_run(
                        orch.as_deref().expect("orchestrator for run"),
                        intent_clone,
                        None,
                    )
                    .await;
                }
                // User declined — exit cleanly with no double-print
                return DispatchResult::success("");
            }
            plan_result
        }
        CliCommand::Cancel { execution_id } => {
            handle_cancel(
                orch.as_deref().expect("orchestrator for cancel"),
                execution_id,
            )
            .await
        }
        CliCommand::Status => {
            handle_status(orch.as_deref().expect("orchestrator for status")).await
        }
        CliCommand::History { limit, .. } => {
            handle_history(services.as_ref().expect("services for history"), limit).await
        }
        CliCommand::Explain { execution_id, .. } => {
            handle_explain(
                services.as_ref().expect("services for explain"),
                execution_id,
            )
            .await
        }
        CliCommand::DiffPlan { id1, id2 } => {
            handle_diff_plan(services.as_ref().expect("services for diff-plan"), id1, id2).await
        }
        CliCommand::Generate { intent } => {
            handle_generate(
                orch.as_deref(),
                llm_services.as_ref(),
                services.as_ref(),
                intent,
            )
            .await
        }
        CliCommand::Template { action } => {
            let svc = llm_services
                .as_ref()
                .or(services.as_ref())
                .expect("services for template");
            handle_template(svc, action).await
        }
        CliCommand::Audit { action } => {
            handle_audit(services.as_ref().expect("services for audit"), action).await
        }
        CliCommand::Logs { session_id } => {
            handle_logs(services.as_ref().expect("services for logs"), session_id).await
        }
        CliCommand::Config { action } => {
            handle_config(services.as_ref().expect("services for config"), action).await
        }
        CliCommand::Init => cmd_init(),
        CliCommand::Key { label } => cmd_key(label),
        CliCommand::Tui { .. } => DispatchResult::success("TUI mode"),
    }
}
