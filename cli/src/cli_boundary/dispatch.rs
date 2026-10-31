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

// #1083: approved process-spawning sink — fixed-argv `git remote/config` reads.
// See `.pi/architecture/modules/tool-system.md#process-spawning-sinks`.
#![allow(clippy::disallowed_types)]

use std::fmt;

use serde_json::Value as JsonValue;

use crate::cli_boundary::cli::{ApproveAction, CliCommand, ConfigAction, LedgerAction};
use crate::cli_boundary::orchestrator::CliServices;
use rigorix_engine::audit::application::projection::{
    ProjectAuthorityFacts, verify_authority_fact_bundle,
};
use rigorix_engine::audit::domain::authority_fact::AuthorityFactBundle;

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

/// `rigorix doctor` — print the seam inventory (#1103).
///
/// Exit code: 0 when nothing is misconfigured (unarmed is fine), 1 when a seam
/// was requested and cannot be honoured, because that seam will refuse runs.
async fn cmd_doctor() -> DispatchResult {
    let repo_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let report = crate::cli_boundary::doctor::probe(&repo_root).await;
    let rendered = report.render();
    if report.exit_code() == 0 {
        DispatchResult::success(rendered)
    } else {
        DispatchResult::error(rendered, report.exit_code())
    }
}

/// `rigorix approve ledger migrate` — the only path that adopts a legacy
/// bare-map consumption ledger (#1247).
///
/// Runs before service bootstrap: the loader refuses a bare map, so building
/// services first would make the migration command unusable in exactly the
/// state it exists to repair.
async fn cmd_approve_ledger_migrate(
    config: &crate::cli_boundary::config::CliConfig,
) -> DispatchResult {
    use rigorix_engine::approval::infrastructure::repository::LedgerMigration;
    let repo_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let signing_key = config
        .engine_config
        .as_ref()
        .and_then(|engine| engine.audit_hmac_key.as_deref());
    match rigorix_engine::execution_engine::application::factory::ApprovalBindingSetup::migrate_consumption_ledger(
        &repo_root,
        signing_key,
    ) {
        Ok(Some(LedgerMigration::Migrated { markers })) => DispatchResult::success(format!(
            "adopted {markers} marker(s) from the legacy consumption ledger; wrote the sealed form"
        )),
        Ok(Some(LedgerMigration::AlreadySealed { markers })) => DispatchResult::success(format!(
            "consumption ledger is already sealed; {markers} marker(s) verify"
        )),
        Ok(None) => DispatchResult::error(
            "approval binding is not configured: export RIGORIX_HMAC_KEY (or configure \
             audit_hmac_key) before migrating the consumption ledger"
                .to_string(),
            1,
        ),
        Err(error) => DispatchResult::error(error.to_string(), 1),
    }
}

/// Scaffold `.rigorix/` and **always** report the seam inventory (#1101).
///
/// The scaffold itself is idempotent: an existing directory is left alone. The
/// advisory is not — it runs on every invocation, because a repository
/// initialised by an older version must still learn what this one knows. The
/// previous `if path.is_dir() { return ... }` made the gap permanent.
async fn cmd_init() -> DispatchResult {
    let repo_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let path = std::path::Path::new(".rigorix");
    // Keyed on the config file, not the directory: `rigorix doctor` (and any
    // engine seam construction) may create `.rigorix/` as a side effect, and an
    // empty directory must not be mistaken for an initialised workspace.
    let existed = path.join("rigorix.toml").is_file();

    if !existed {
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

# ── Enforcement seams — ALL UNARMED AS WRITTEN (#1101) ──────────────────────
# Nothing below is active. Each line names the file or variable that arms a
# seam; uncommenting a file path is not enough on its own — read the comment.
#
#   precondition gate   .rigorix/preconditions.toml     (argv checks, ADR-017)
#                       re-reads authoritative state before dispatch; a
#                       malformed file arms a refuse-everything gate
#   sequence policy     .rigorix/sequence-policy.toml   (promote/prerequisite rules)
#   capacity ledger     .rigorix/capacity.toml          (contested limits, ADR-018)
#   approval binding    RIGORIX_APPROVAL_BINDING=1 + RIGORIX_HMAC_KEY=<secret>
#                       binding without a key is a startup error, not a warning
#   history anchor      RIGORIX_ANCHOR_URL + RIGORIX_ANCHOR_PUBLIC_KEY
#   envelope signing    RIGORIX_HMAC_KEY=<secret>       (run `rigorix key`)
#
# `rigorix doctor` prints which of these are armed in this repository.
"#;
                let config_path = path.join("rigorix.toml");
                if let Err(e) = std::fs::write(&config_path, default_config) {
                    return DispatchResult::error(format!("Failed to write config: {e}"), 1);
                }
            }
            Err(e) => return DispatchResult::error(format!("Failed to create .rigorix/: {e}"), 1),
        }
    }

    let report = crate::cli_boundary::doctor::probe(&repo_root).await;
    let headline = if existed {
        ".rigorix/rigorix.toml already exists — scaffold left unchanged"
    } else {
        "Initialized .rigorix/ directory"
    };
    DispatchResult::success(format!(
        "{headline}\n\n{}\nNext: `rigorix doctor` lists every seam above; \\\n`rigorix init` writes unarmed configuration only.",
        report.render()
    ))
}

/// Produce the key the engine actually consumes (#1102).
///
/// This used to print a random UUID labelled "Generated API key" and return it in
/// a field called `key`. Nothing reads that value; 26 call sites read
/// `RIGORIX_HMAC_KEY`. A user who ran the command had configured nothing and
/// would find out later as an unverifiable envelope — the harder failure to
/// diagnose. It now emits the key the readers want, and names the variable it has
/// to be set as.
async fn cmd_key(label: Option<String>) -> DispatchResult {
    use rand::RngCore;

    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let key: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let label = label.unwrap_or_else(|| "default".to_string());

    let data = serde_json::json!({
        "env_var": "RIGORIX_HMAC_KEY",
        "key": key,
        "label": label,
        "algorithm": "HMAC-SHA256, 32-byte key, hex-encoded",
    });

    DispatchResult::success_with_data(
        format!(
            "Generated HMAC signing key ({label}) — this is the key the engine reads as \
             RIGORIX_HMAC_KEY.\n\n  export RIGORIX_HMAC_KEY={key}\n\n\
             It is printed once and not stored: put it in your secret manager or shell \
             profile. Without it, audit envelopes are unsigned and `rigorix doctor` \
             reports `envelope signing: unarmed`."
        ),
        data,
    )
}

// ---------------------------------------------------------------------------
// Tier 1 handlers — via OrchestratorService
// ---------------------------------------------------------------------------

async fn handle_run(
    orch: &dyn rigorix_engine::orchestrator::application::OrchestratorService,
    intent: String,
) -> DispatchResult {
    let input = rigorix_engine::orchestrator::application::dto::RunInput {
        intent,
        config: serde_json::Value::Null,
        repo_root: std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        repository: detect_git_repository(),
        author: detect_git_author(),
        // Identity attestation arrives via the MCP auth module (ADR-012);
        // the CLI run path is legacy self-asserted author.
        identity: None,
        // Reserved for the per-run override (#1156): the CLI flag is refused,
        // so this is always None until that override path exists.
        enforcement_preset: None,
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

/// Detect git repository in "owner/repo" format from the origin remote.
fn detect_git_repository() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        parse_repo_from_url(&url)
    } else {
        None
    }
}

/// Detect git author email from local config.
fn detect_git_author() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["config", "user.email"])
        .output()
        .ok()?;
    if output.status.success() {
        let email = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if email.is_empty() { None } else { Some(email) }
    } else {
        None
    }
}

/// Parse "owner/repo" from common git URL formats.
///
/// Handles: https://github.com/owner/repo.git, git@github.com:owner/repo.git,
/// ssh://git@github.com/owner/repo.git
fn parse_repo_from_url(url: &str) -> Option<String> {
    // Strip protocol prefix if present (https://, ssh://, etc.)
    let stripped = url
        .trim()
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("ssh://"))
        .unwrap_or(url);

    // Handle git@github.com:owner/repo.git format
    let path = if let Some(at_pos) = stripped.find('@') {
        // git@host:owner/repo.git → owner/repo.git
        if let Some(colon_pos) = stripped[at_pos..].find(':') {
            &stripped[at_pos + colon_pos + 1..]
        } else {
            stripped
        }
    } else {
        // Remove host portion: github.com/owner/repo.git → owner/repo.git
        if let Some(slash_pos) = stripped.find('/') {
            &stripped[slash_pos + 1..]
        } else {
            stripped
        }
    };

    // Strip trailing .git
    let path = path.strip_suffix(".git").unwrap_or(path);

    // Must contain exactly one slash: owner/repo
    if path.split('/').count() == 2 && !path.is_empty() && !path.starts_with('/') {
        Some(path.to_string())
    } else {
        None
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
        crate::cli_boundary::cli::AuditAction::Projection { subject, out } => {
            // A bundle must carry a complete, verifiable chain, so ask for the
            // whole store rather than the default 100-entry window.
            match svc.audit_repository.list(None, None, Some(u32::MAX)).await {
                Ok(envelopes) => {
                    let projection = ProjectAuthorityFacts::project(&envelopes, &subject);
                    let fact_count = projection.facts.len();
                    let bundle = AuthorityFactBundle {
                        projection,
                        envelopes,
                        subjects_without_record: subject,
                    };
                    match out {
                        Some(path) => match serde_json::to_string_pretty(&bundle) {
                            Ok(json) => match std::fs::write(&path, &json) {
                                Ok(()) => DispatchResult::success_with_data(
                                    format!(
                                        "wrote authority-fact bundle ({fact_count} fact(s)) to {}",
                                        path.display()
                                    ),
                                    serde_json::json!({
                                        "path": path.display().to_string(),
                                        "facts": fact_count,
                                    }),
                                ),
                                Err(e) => DispatchResult::error(format!("write bundle: {e}"), 1),
                            },
                            Err(e) => DispatchResult::error(format!("serialize bundle: {e}"), 1),
                        },
                        None => DispatchResult::success_with_data(
                            format!(
                                "authority-fact bundle ({fact_count} fact(s)) — verify by re-derivation with `audit verify-projection`"
                            ),
                            serde_json::to_value(&bundle).unwrap_or(JsonValue::Null),
                        ),
                    }
                }
                Err(e) => DispatchResult::error(format!("audit projection: {e}"), 1),
            }
        }
        crate::cli_boundary::cli::AuditAction::VerifyProjection { bundle: path } => {
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(e) => {
                    return DispatchResult::error(
                        format!("read bundle {}: {e}", path.display()),
                        1,
                    );
                }
            };
            let bundle: AuthorityFactBundle = match serde_json::from_str(&text) {
                Ok(bundle) => bundle,
                Err(e) => return DispatchResult::error(format!("parse bundle: {e}"), 1),
            };
            match verify_authority_fact_bundle(&bundle) {
                Ok(outcome) => DispatchResult::success_with_data(
                    format!(
                        "authority-fact bundle consistent by re-derivation ({} fact(s)); this proves \
                         the projection matches the supplied history, NOT that the history is \
                         authentic or externally anchored — provenance needs a separate trusted-anchor check",
                        bundle.projection.facts.len()
                    ),
                    serde_json::json!({
                        "verified": true,
                        "check": "chain_consistency",
                        "consistency": serde_json::to_value(outcome).unwrap_or_default(),
                        "provenance_established": false,
                        "facts": bundle.projection.facts.len(),
                        "projection_digest": bundle.projection.digest(),
                    }),
                ),
                Err(e) => DispatchResult::error(
                    format!("authority-fact bundle FAILED consistency verification: {e}"),
                    1,
                ),
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
        ConfigAction::Init => cmd_init().await,
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
    // A diagnostic must run when the rest of the CLI cannot (#1103).
    //
    // `build_cli_services` opens config, state and the approval store; with
    // `RIGORIX_APPROVAL_BINDING=1` and no key it fails closed and returns early —
    // which is precisely the state an operator needs `doctor` to explain. So both
    // commands are answered before any service is built. `init` is here for the
    // same reason: it is how a fresh repo gets a config at all.
    match &command {
        CliCommand::Doctor => return cmd_doctor().await,
        CliCommand::Init => return cmd_init().await,
        CliCommand::Approve {
            action:
                ApproveAction::Ledger {
                    action: LedgerAction::Migrate,
                },
        } => return cmd_approve_ledger_migrate(&config).await,
        _ => {}
    }

    // #1156: the flag is accepted by clap and was threaded into RunInput, but
    // nothing read it — the executor config is built once at startup. Refusing
    // loudly is the only option that does not let an operator believe the run
    // is hardened when it is not.
    if let CliCommand::Run {
        enforcement: Some(preset),
        ..
    } = &command
    {
        return DispatchResult::error(
            format!(
                concat!(
                    "`--enforcement {preset}` is not implemented: the executor config is built once at startup, ",
                    "so a per-run preset cannot be honoured. Set `[enforcement] preset` in rigorix.toml instead. ",
                    "The flag is refused rather than silently ignored."
                ),
                preset = preset
            ),
            2,
        );
    }

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

    // GAP-L-08: services is built unconditionally above (always Some or an
    // early error return). Bind it directly — a None here is an internal bug
    // reported as an error, never a panic on the dispatch path.
    let services = match services {
        Some(s) => s,
        None => {
            return DispatchResult::error(
                "internal error: CLI services unavailable".to_string(),
                1,
            );
        }
    };

    // Thin router — delegates to handler functions
    match command {
        CliCommand::Run { intent, .. } => {
            let Some(orch) = orch.as_deref() else {
                return DispatchResult::error(
                    "internal error: orchestrator not built for run".to_string(),
                    1,
                );
            };
            handle_run(orch, intent).await
        }
        CliCommand::Plan { intent } => {
            let Some(orch) = orch.as_deref() else {
                return DispatchResult::error(
                    "internal error: orchestrator not built for plan".to_string(),
                    1,
                );
            };
            let intent_clone = intent.clone();
            // Show plan output (handle_plan returns DispatchResult but doesn't print it)
            let plan_result = handle_plan(orch, intent).await;
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
                    return handle_run(orch, intent_clone).await;
                }
                // User declined — exit cleanly with no double-print
                return DispatchResult::success("");
            }
            plan_result
        }
        CliCommand::Cancel { execution_id } => {
            let Some(orch) = orch.as_deref() else {
                return DispatchResult::error(
                    "internal error: orchestrator not built for cancel".to_string(),
                    1,
                );
            };
            handle_cancel(orch, execution_id).await
        }
        CliCommand::Status => {
            let Some(orch) = orch.as_deref() else {
                return DispatchResult::error(
                    "internal error: orchestrator not built for status".to_string(),
                    1,
                );
            };
            handle_status(orch).await
        }
        CliCommand::History { limit, .. } => handle_history(&services, limit).await,
        CliCommand::Explain { execution_id, .. } => handle_explain(&services, execution_id).await,
        CliCommand::DiffPlan { id1, id2 } => handle_diff_plan(&services, id1, id2).await,
        CliCommand::Generate { intent } => {
            handle_generate(
                orch.as_deref(),
                llm_services.as_ref(),
                Some(&services),
                intent,
            )
            .await
        }
        CliCommand::Template { action } => {
            let svc = llm_services.as_ref().unwrap_or(&services);
            handle_template(svc, action).await
        }
        CliCommand::Audit { action } => handle_audit(&services, action).await,
        CliCommand::Logs { session_id } => handle_logs(&services, session_id).await,
        CliCommand::Config { action } => handle_config(&services, action).await,
        CliCommand::Key { label } => cmd_key(label).await,
        // Answered before service bootstrap (see the top of this function).
        CliCommand::Approve { .. } => {
            DispatchResult::error("internal: handled before dispatch".to_string(), 1)
        }
        CliCommand::Doctor | CliCommand::Init => {
            DispatchResult::error("internal: handled before dispatch".to_string(), 1)
        }
        CliCommand::Tui { .. } => DispatchResult::success("TUI mode"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_enforcement_flag_is_refused_instead_of_silently_ignored() {
        // #1156: if this starts returning 0, the flag became a no-op again.
        let command = CliCommand::Run {
            intent: "do the thing".to_string(),
            enforcement: Some("strict".to_string()),
            max_llm_calls: None,
            max_llm_tokens: None,
        };
        let result = dispatch(
            command,
            crate::cli_boundary::config::CliConfig::default(),
            crate::cli_boundary::signal::CancellationToken::new(),
        )
        .await;

        assert_eq!(result.exit_code, 2, "a refused flag is a usage error");
        assert!(
            result.summary.contains("not implemented"),
            "{}",
            result.summary
        );
        assert!(
            !result.summary.contains("  "),
            "the message renders with a gap: {:?}",
            result.summary
        );
        assert!(
            result.summary.contains("[enforcement] preset"),
            "the error must name the working alternative: {}",
            result.summary
        );
    }
}
