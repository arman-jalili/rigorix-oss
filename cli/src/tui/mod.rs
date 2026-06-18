//! Terminal UI module — interactive TUI (ratatui).
//! @canonical .pi/architecture/modules/tui.md

pub mod command_bar;
pub mod event_bridge;
pub mod input;
pub mod orchestrator_spawner;
pub mod plan_review;
pub mod view_model;
pub mod views;
pub mod widgets;

use std::time::Duration;

use ratatui::Frame;
use ratatui::Terminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use tokio_util::sync::CancellationToken;

use crate::cli_boundary::config::CliConfig;

use self::command_bar::CommandBarState;
use self::input::keymap;
use self::input::{InputFocus, KeyAction};
use self::event_bridge::event_to_vm_command;
use self::view_model::{ActiveView, ExecutionPhase, NodeStatus, NodeViewModel, TuiViewModel};
use self::widgets::{LayoutMode, WidgetContext, cmd_bar, status_bar};

/// Commands sent from the background orchestrator task to the TUI event loop.
#[allow(clippy::enum_variant_names, dead_code)]
#[derive(Debug)]
pub(crate) enum VmCommand {
    /// Set the execution phase.
    Phase(ExecutionPhase),
    /// Set execution ID.
    ExecutionId(uuid::Uuid),
    /// Set template ID.
    TemplateId(String),
    /// Set LLM calls metric.
    LlmCalls(u64),
    /// Set LLM tokens metric.
    Tokens(u64),
    /// Populate DAG nodes from plan graph JSON.
    SetNodes(Vec<view_model::NodeViewModel>),
    /// Set an error message and phase to Failed.
    Error(String),
    /// Set or clear the copy-to-file message.
    CopyMessage(Option<String>),
    /// Plan completed (from background task).
    PlanCompleted(Box<rigorix_engine::orchestrator::application::dto::PlanOnlyOutput>),
    /// Plan failed (from background task).
    PlanFailed(String),
    /// Run completed (from background task).
    RunCompleted(Box<rigorix_engine::orchestrator::application::dto::RunOutput>),
    /// Run failed (from background task).
    RunFailed(String),
}

/// Run the interactive TUI.
pub async fn run(
    config: CliConfig,
    cancellation_token: CancellationToken,
    exec: Option<uuid::Uuid>,
    run: Option<String>,
) {
    let _ = (exec, run);
    let (vm_tx, mut vm_rx) = tokio::sync::mpsc::channel::<VmCommand>(64);

    let _ = ratatui::crossterm::terminal::enable_raw_mode();
    let _ = ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::terminal::EnterAlternateScreen,
        ratatui::crossterm::event::EnableMouseCapture,
    );
    let mut terminal =
        match Terminal::new(ratatui::backend::CrosstermBackend::new(std::io::stdout())) {
            Ok(t) => t,
            Err(e) => {
                restore_terminal();
                eprintln!("Failed to initialise terminal: {e}");
                return;
            }
        };

    let mut vm = TuiViewModel::default();
    let mut command_bar = CommandBarState::default();
    let mut input_focus = InputFocus::CommandBar;
    let mut selected_node: Option<String> = None;

    // Build orchestrator once at startup (same as CLI dispatch)
    let repo_root = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let orch = match crate::cli_boundary::orchestrator::build_orchestrator(
        config.clone(),
        cancellation_token.clone(),
        repo_root,
    )
    .await
    {
        Ok((o, _svc)) => {
            let orch: std::sync::Arc<
                dyn rigorix_engine::orchestrator::application::service::OrchestratorService,
            > = o.into();
            orch
        }
        Err(e) => {
            restore_terminal();
            eprintln!("Failed to build orchestrator: {e}");
            return;
        }
    };

    let result = run_event_loop(
        &mut terminal,
        &mut vm,
        &mut command_bar,
        &mut input_focus,
        &mut selected_node,
        orch,
        config,
        vm_tx,
        &mut vm_rx,
    )
    .await;

    restore_terminal();
    if let Err(e) = result {
        eprintln!("TUI error: {e}");
    }
}

fn restore_terminal() {
    let _ = ratatui::crossterm::terminal::disable_raw_mode();
    let _ = ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::terminal::LeaveAlternateScreen,
        ratatui::crossterm::event::DisableMouseCapture,
    );
}

#[allow(clippy::too_many_arguments)]
async fn run_event_loop(
    terminal: &mut Terminal<ratatui::prelude::CrosstermBackend<std::io::Stdout>>,
    vm: &mut TuiViewModel,
    command_bar: &mut CommandBarState,
    input_focus: &mut InputFocus,
    selected_node: &mut Option<String>,
    orch: std::sync::Arc<
        dyn rigorix_engine::orchestrator::application::service::OrchestratorService,
    >,
    _config: crate::cli_boundary::config::CliConfig,
    vm_tx: tokio::sync::mpsc::Sender<VmCommand>,
    vm_rx: &mut tokio::sync::mpsc::Receiver<VmCommand>,
) -> Result<(), String> {
    loop {
        terminal
            .draw(|frame| {
                let area = frame.area();
                let layout_mode = LayoutMode::from_size(area.width, area.height);
                let ctx = WidgetContext {
                    area,
                    layout_mode,
                    color_enabled: true,
                    detailed: layout_mode != LayoutMode::Compact,
                };
                draw_frame(
                    frame,
                    area,
                    &ctx,
                    vm,
                    command_bar,
                    input_focus,
                    selected_node,
                );
            })
            .map_err(|e| format!("Render error: {e}"))?;

        // Poll for commands from background tasks before rendering
        while let Ok(cmd) = vm_rx.try_recv() {
            apply_vm_command(vm, cmd);
        }

        if event::poll(Duration::from_millis(50)).map_err(|e| format!("Poll error: {e}"))?
            && let Event::Key(key) = event::read().map_err(|e| format!("Read error: {e}"))?
            && (key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat)
        {
            let action = keymap::map_key(key, *input_focus);
            if !handle_action(
                action,
                vm,
                command_bar,
                input_focus,
                selected_node,
                key,
                &vm_tx,
                &orch,
            )
            .await
            {
                break;
            }
        }
    }
    Ok(())
}

/// Apply a VmCommand to the ViewModel.
fn apply_vm_command(vm: &mut TuiViewModel, cmd: VmCommand) {
    match cmd {
        VmCommand::Phase(phase) => vm.phase = phase,
        VmCommand::ExecutionId(id) => vm.execution_id = Some(id),
        VmCommand::TemplateId(tid) => vm.template_id = Some(tid),
        VmCommand::LlmCalls(n) => vm.metrics.llm_calls = n,
        VmCommand::Tokens(n) => vm.metrics.tokens = n,
        VmCommand::CopyMessage(msg) => vm.copy_message = msg,
        VmCommand::SetNodes(nodes) => {
            for n in nodes {
                // Try to find existing node by id first, then by name as fallback
                let target_id = if vm.nodes.contains_key(&n.id) {
                    n.id.clone()
                } else if let Some((existing_id, _)) =
                    vm.nodes.iter().find(|(_, v)| v.name == n.name && !n.name.is_empty())
                {
                    existing_id.clone()
                } else {
                    n.id.clone()
                };

                vm.nodes
                    .entry(target_id)
                    .and_modify(|existing| {
                        // Preserve existing name/tool/risk if incoming is empty
                        if !n.name.is_empty() {
                            existing.name = n.name.clone();
                        }
                        if !n.tool_name.is_empty() {
                            existing.tool_name = n.tool_name.clone();
                        }
                        existing.status = n.status;
                        if let Some(ms) = n.timing_ms {
                            existing.timing_ms = Some(ms);
                        }
                        if n.output_preview.is_some() {
                            existing.output_preview = n.output_preview.clone();
                        }
                        if n.error.is_some() {
                            existing.error = n.error.clone();
                        }
                        existing.retry_count = n.retry_count;
                    })
                    .or_insert_with(|| n);
            }
        }
        VmCommand::Error(err) => {
            vm.error = Some(err);
            vm.phase = ExecutionPhase::Failed;
        }
        VmCommand::PlanCompleted(output) => {
            // Extract template metadata
            if let Some(toml) = output.plan["generated_toml"].as_str() {
                let tid = output.plan["template_id"].as_str().unwrap_or("unknown");
                let tpl_dir = std::path::PathBuf::from(".rigorix/templates");
                let tpl_path = tpl_dir.join(format!("{tid}.toml"));
                let _ = std::fs::create_dir_all(&tpl_dir);
                let _ = std::fs::write(&tpl_path, toml);
                vm.template_id = Some(tid.to_string());
            }
            if let Some(calls) = output.plan["llm_calls_used"].as_u64() {
                vm.metrics.llm_calls = calls;
            }
            if let Some(tokens) = output.plan["llm_tokens_used"].as_u64() {
                vm.metrics.tokens = tokens;
            }
            let nodes = parse_graph_nodes(&output.graph);
            if !nodes.is_empty() {
                vm.nodes.clear();
                for n in nodes {
                    vm.nodes.insert(n.id.clone(), n);
                }
            }
            vm.phase = ExecutionPhase::Completed;
            vm.active_view = ActiveView::Plan;
        }
        VmCommand::PlanFailed(err) => {
            vm.error = Some(err);
            vm.phase = ExecutionPhase::Failed;
        }
        VmCommand::RunCompleted(output) => {
            vm.execution_id = Some(output.execution_id);
            // Update node statuses from task results
            for task in &output.record.task_results {
                if let Some(node) = vm.nodes.get_mut(&task.node_id) {
                    node.status = match task.status {
                        rigorix_engine::orchestrator::domain::record::TaskStatus::Success => {
                            view_model::NodeStatus::Completed
                        }
                        rigorix_engine::orchestrator::domain::record::TaskStatus::Failure => {
                            view_model::NodeStatus::Failed
                        }
                        rigorix_engine::orchestrator::domain::record::TaskStatus::Skipped => {
                            view_model::NodeStatus::Skipped
                        }
                        rigorix_engine::orchestrator::domain::record::TaskStatus::Cancelled => {
                            view_model::NodeStatus::Skipped
                        }
                        rigorix_engine::orchestrator::domain::record::TaskStatus::Pending => {
                            view_model::NodeStatus::Pending
                        }
                    };
                    node.output_preview = task.output.clone();
                    node.timing_ms = Some(task.duration_ms);
                    node.retry_count = task.retry_attempts;
                    node.error = task.error.clone();
                }
            }
            // Update metrics
            vm.metrics.nodes_total = vm.nodes.len() as u32;
            vm.metrics.nodes_completed = vm
                .nodes
                .values()
                .filter(|n| n.status == view_model::NodeStatus::Completed)
                .count() as u32;
            vm.phase = ExecutionPhase::Completed;
            vm.active_view = ActiveView::Dashboard;
        }
        VmCommand::RunFailed(err) => {
            vm.error = Some(err);
            vm.phase = ExecutionPhase::Failed;
        }
    }
}

/// Render the current view as plain text (for copy-to-file).
pub(crate) fn render_view_as_text(vm: &TuiViewModel) -> String {
    let mut out = String::new();
    let phase = format!("{:?}", vm.phase);
    out.push_str(&format!(
        "Rigorix — View: {:?} | Phase: {}\n",
        vm.active_view, phase
    ));
    out.push_str(&format!(
        "Execution: {} | Template: {}\n",
        vm.execution_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "-".to_string()),
        vm.template_id.as_deref().unwrap_or("-"),
    ));
    out.push_str(&format!(
        "LLM: {} calls, {} tokens | Intent: {}\n",
        vm.metrics.llm_calls,
        vm.metrics.tokens,
        vm.intent.as_deref().unwrap_or("-"),
    ));
    if let Some(err) = &vm.error {
        out.push_str(&format!("Error: {}\n", err));
    }
    out.push('\n');

    let mut nodes: Vec<&NodeViewModel> = vm.nodes.values().collect();
    nodes.sort_by(|a, b| {
        let order = |s: NodeStatus| -> u8 {
            match s {
                NodeStatus::Failed => 0,
                NodeStatus::InProgress => 1,
                NodeStatus::Pending => 2,
                _ => 3,
            }
        };
        order(a.status).cmp(&order(b.status))
    });

    out.push_str(&format!("Nodes ({} total):\n", nodes.len()));
    for n in &nodes {
        let icon = match n.status {
            NodeStatus::Completed => "✓",
            NodeStatus::InProgress => "▶",
            NodeStatus::Failed => "✗",
            NodeStatus::Retrying => "↻",
            NodeStatus::Pending => "·",
            NodeStatus::Skipped => "–",
        };
        let deps = if n.dependencies.is_empty() {
            "(root)".to_string()
        } else {
            format!("← [{}]", n.dependencies.join(", "))
        };
        out.push_str(&format!("  {} {} {} {}\n", icon, n.name, deps, n.tool_name));
        if let Some(ms) = n.timing_ms {
            out.push_str(&format!("     {}ms\n", ms));
        }
        if let Some(ref err) = n.error {
            let truncated: String = err.lines().take(3).collect::<Vec<_>>().join("\n     ");
            out.push_str(&format!("     Error: {}\n", truncated));
        }
    }

    if !vm.event_log.is_empty() {
        out.push_str(&format!("\nEvents ({}):\n", vm.event_log.len()));
        for entry in &vm.event_log {
            out.push_str(&format!(
                "  {} [{}] {}\n",
                entry.timestamp_ms, entry.event_type, entry.summary
            ));
        }
    }

    out
}

/// Copy text to system clipboard using the platform's clipboard command.
/// Returns true if the clipboard command succeeded.
fn copy_to_clipboard(text: &str) -> bool {
    // macOS: pbcopy, Linux: xclip or xsel, Windows: clip
    let cmd = if cfg!(target_os = "macos") {
        "pbcopy"
    } else if cfg!(target_os = "linux") {
        // Try xclip first, fall back to xsel
        "xclip"
    } else if cfg!(target_os = "windows") {
        "clip"
    } else {
        return false;
    };

    std::process::Command::new(cmd)
        .arg(if cmd == "xclip" { "-selection" } else { "" })
        .arg(if cmd == "xclip" { "clipboard" } else { "" })
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text.as_bytes())?;
            }
            child.wait()?;
            Ok(())
        })
        .is_ok()
}

/// Parse the graph JSON from a plan output into NodeViewModel items.
pub(crate) fn parse_graph_nodes(graph: &serde_json::Value) -> Vec<view_model::NodeViewModel> {
    let mut nodes = Vec::new();
    let mut name_to_id: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    // First pass: parse raw nodes and build name→id map
    if let Some(raw_nodes) = graph.get("nodes").and_then(|n| n.as_array()) {
        // If nodes have UUID ids, try to resolve dependency names
        for raw in raw_nodes {
            let id = raw["id"].as_str().unwrap_or("").to_string();
            let name = raw["name"].as_str().unwrap_or(&id).to_string();
            name_to_id.insert(name.clone(), id.clone());
        }

        for raw in raw_nodes {
            let id = raw["id"].as_str().unwrap_or("").to_string();
            let name = raw["name"].as_str().unwrap_or(&id).to_string();
            let tool_name = raw["tool"].as_str().unwrap_or("").to_string();

            // Resolve dependency UUIDs to names using the name→id map, reversed
            let dep_ids: Vec<String> = raw["dependencies"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            // Resolve dep IDs to human-readable names
            let dep_names: Vec<String> = dep_ids
                .iter()
                .map(|dep_id| {
                    // Find the node name for this dependency ID
                    name_to_id
                        .iter()
                        .find(|(_n, nid)| *nid == dep_id)
                        .map(|(n, _)| n.clone())
                        .unwrap_or_else(|| dep_id.clone())
                })
                .collect();

            nodes.push(view_model::NodeViewModel {
                id: id.clone(),
                name,
                tool_name,
                status: view_model::NodeStatus::Pending,
                dependencies: dep_names.clone(),
                dependents: Vec::new(),
                timing_ms: None,
                output_preview: None,
                error: None,
                retry_count: 0,
                risk_level: None,
            });
        }

        // Second pass: compute dependents (reverse of dependencies)
        // Use indices to avoid simultaneous mutable borrows
        for i in 0..nodes.len() {
            let dep_names: Vec<String> = nodes[i].dependencies.clone();
            let my_name = nodes[i].name.clone();
            for dep_name in &dep_names {
                if let Some(dep_node) = nodes.iter_mut().find(|dn| dn.name == *dep_name) {
                    if !dep_node.dependents.contains(&my_name) {
                        dep_node.dependents.push(my_name.clone());
                    }
                }
            }
        }
    }
    nodes
}

#[allow(clippy::too_many_arguments)]
async fn handle_action(
    action: KeyAction,
    vm: &mut TuiViewModel,
    command_bar: &mut CommandBarState,
    input_focus: &mut InputFocus,
    selected_node: &mut Option<String>,
    key: event::KeyEvent,
    vm_tx: &tokio::sync::mpsc::Sender<VmCommand>,
    orch: &std::sync::Arc<
        dyn rigorix_engine::orchestrator::application::service::OrchestratorService,
    >,
) -> bool {
    match action {
        KeyAction::Quit => return false,
        KeyAction::FocusCommandBar => {
            *input_focus = InputFocus::CommandBar;
            command_bar.focused = true;
        }
        KeyAction::BlurCommandBar => {
            command_bar.focused = false;
            *input_focus = match vm.active_view {
                ActiveView::Dashboard => InputFocus::Dashboard,
                ActiveView::Plan => InputFocus::PlanReview,
                ActiveView::Events => InputFocus::Events,
                _ => InputFocus::View(vm.active_view),
            };
        }
        KeyAction::ExecuteCommand => {
            if let Some(parsed) = command_bar.parse() {
                match parsed {
                    command_bar::CommandBarInput::Intent(intent) => {
                        vm.intent = Some(intent.clone());
                        vm.phase = ExecutionPhase::Planning;
                        vm.active_view = ActiveView::Plan;
                        *input_focus = InputFocus::PlanReview;
                        command_bar.focused = false;
                        // Spawn plan_only in background — UI stays responsive
                        let orch = orch.clone();
                        let tx = vm_tx.clone();
                        let repo_root = std::env::current_dir()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        tokio::spawn(async move {
                            let input =
                                rigorix_engine::orchestrator::application::dto::PlanOnlyInput {
                                    intent: intent.clone(),
                                    config: serde_json::Value::Null,
                                    repo_root: repo_root.clone(),
                                };
                            match orch.plan_only(input).await {
                                Ok(output) => {
                                    let _ =
                                        tx.send(VmCommand::PlanCompleted(Box::new(output))).await;
                                }
                                Err(e) => {
                                    let err_msg = e.to_string();
                                    // Retry once on missing-parameter failures —
                                    // LLM stochasticity may produce a different template
                                    if err_msg.contains("Missing required parameter") {
                                        let input2 = rigorix_engine::orchestrator::application::dto::PlanOnlyInput {
                                            intent,
                                            config: serde_json::Value::Null,
                                            repo_root,
                                        };
                                        match orch.plan_only(input2).await {
                                            Ok(output) => {
                                                let _ = tx
                                                    .send(VmCommand::PlanCompleted(Box::new(
                                                        output,
                                                    )))
                                                    .await;
                                            }
                                            Err(e2) => {
                                                let _ = tx
                                                    .send(VmCommand::PlanFailed(format!(
                                                        "Retried; {e2}"
                                                    )))
                                                    .await;
                                            }
                                        }
                                    } else {
                                        let _ = tx.send(VmCommand::PlanFailed(err_msg)).await;
                                    }
                                }
                            }
                        });
                    }
                    command_bar::CommandBarInput::SlashCommand(cmd) => match cmd.as_str() {
                        "history" => vm.active_view = ActiveView::History,
                        "templates" => vm.active_view = ActiveView::Templates,
                        "audit" | "events" => vm.active_view = ActiveView::Events,
                        "nodes" => vm.active_view = ActiveView::Nodes,
                        "settings" => vm.active_view = ActiveView::Settings,
                        _ => {}
                    },
                    command_bar::CommandBarInput::ColonCommand(cmd) => match cmd.as_str() {
                        "q" => return false,
                        "cancel" | "cancel!" => vm.phase = ExecutionPhase::Cancelled,
                        _ => {}
                    },
                }
                command_bar.submit();
            }
        }
        KeyAction::NextView | KeyAction::PrevView => {
            let cycle = [
                ActiveView::Dashboard,
                ActiveView::Plan,
                ActiveView::Nodes,
                ActiveView::Events,
                ActiveView::Templates,
                ActiveView::History,
            ];
            let idx = cycle.iter().position(|v| *v == vm.active_view).unwrap_or(0);
            let len = cycle.len();
            let dir: i32 = if matches!(action, KeyAction::NextView) {
                1
            } else {
                -1
            };
            vm.active_view = cycle[((idx as i32 + dir + len as i32) as usize) % len];
            // Sync input_focus to the new view
            *input_focus = match vm.active_view {
                ActiveView::Dashboard => InputFocus::Dashboard,
                ActiveView::Plan => InputFocus::PlanReview,
                ActiveView::Events => InputFocus::Events,
                _ => InputFocus::View(vm.active_view),
            };
        }
        KeyAction::RunPlan => {
            vm.phase = ExecutionPhase::Executing;
            vm.active_view = ActiveView::Dashboard;
            *input_focus = InputFocus::Dashboard;
            vm.error = None;
            // Mark all nodes as Pending and reset metrics
            for node in vm.nodes.values_mut() {
                node.status = view_model::NodeStatus::Pending;
                node.timing_ms = None;
                node.output_preview = None;
                node.error = None;
            }
            vm.metrics.nodes_total = vm.nodes.len() as u32;
            vm.metrics.nodes_completed = 0;
            let intent = vm.intent.clone().unwrap_or_default();
            let orch = orch.clone();
            let tx = vm_tx.clone();
            let repo_root = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            // Subscribe to the event bus for real-time progress updates
            let event_bus_rx = orch.event_bus().subscribe_receiver();

            // Spawn the event bridge task: reads events -> sends VmCommands
            let bridge_tx = tx.clone();
            tokio::spawn(async move {
                let mut rx = event_bus_rx;
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            if let Some(cmd) = event_to_vm_command(&event) {
                                let _ = bridge_tx.send(cmd).await;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(dropped = n, "EventBridge lagged");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });

            // Spawn the run task
            tokio::spawn(async move {
                let input = rigorix_engine::orchestrator::application::dto::RunInput {
                    intent,
                    config: serde_json::Value::Null,
                    repo_root,
                    enforcement_preset: None,
                };
                match orch.run(input).await {
                    Ok(output) => {
                        let _ = tx.send(VmCommand::RunCompleted(Box::new(output))).await;
                    }
                    Err(e) => {
                        let _ = tx
                            .send(VmCommand::RunFailed(format!("Run failed: {e}")))
                            .await;
                    }
                }
            });
        }
        KeyAction::PlanOnly => {
            vm.active_view = ActiveView::Plan;
            *input_focus = InputFocus::PlanReview;
            if vm.nodes.is_empty() {
                // Re-trigger plan in background if no nodes yet
                let intent = vm.intent.clone().unwrap_or_default();
                if !intent.is_empty() {
                    vm.phase = ExecutionPhase::Planning;
                    let orch = orch.clone();
                    let tx = vm_tx.clone();
                    let repo_root = std::env::current_dir()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    tokio::spawn(async move {
                        let input = rigorix_engine::orchestrator::application::dto::PlanOnlyInput {
                            intent,
                            config: serde_json::Value::Null,
                            repo_root,
                        };
                        match orch.plan_only(input).await {
                            Ok(output) => {
                                let _ = tx.send(VmCommand::PlanCompleted(Box::new(output))).await;
                            }
                            Err(e) => {
                                let _ = tx.send(VmCommand::PlanFailed(e.to_string())).await;
                            }
                        }
                    });
                }
            }
        }
        KeyAction::GenerateTemplate => {
            vm.active_view = ActiveView::Plan;
            *input_focus = InputFocus::PlanReview;
            // Same as PlanOnly — triggers plan in background
            let intent = vm.intent.clone().unwrap_or_default();
            if !intent.is_empty() {
                vm.phase = ExecutionPhase::Planning;
                let orch = orch.clone();
                let tx = vm_tx.clone();
                let repo_root = std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                tokio::spawn(async move {
                    let input = rigorix_engine::orchestrator::application::dto::PlanOnlyInput {
                        intent,
                        config: serde_json::Value::Null,
                        repo_root,
                    };
                    match orch.plan_only(input).await {
                        Ok(output) => {
                            let _ = tx.send(VmCommand::PlanCompleted(Box::new(output))).await;
                        }
                        Err(e) => {
                            let _ = tx.send(VmCommand::PlanFailed(e.to_string())).await;
                        }
                    }
                });
            }
        }
        KeyAction::SelectNext | KeyAction::SelectPrev => {
            let ids: Vec<String> = vm.nodes.keys().cloned().collect();
            if !ids.is_empty() {
                let idx = selected_node
                    .as_ref()
                    .and_then(|s| ids.iter().position(|id| id == s))
                    .unwrap_or(0);
                let dir: usize = if matches!(action, KeyAction::SelectNext) {
                    1
                } else {
                    ids.len() - 1
                };
                *selected_node = Some(ids[(idx + dir) % ids.len()].clone());
            }
        }
        KeyAction::ToggleExpand
        | KeyAction::ShowOutput
        | KeyAction::Scroll(_)
        | KeyAction::ShowHelp
        | KeyAction::Search
        | KeyAction::FilterEvents(_) => {}
        KeyAction::ShowDetail => {
            if selected_node.is_some() {
                vm.active_view = ActiveView::Nodes;
            }
        }
        KeyAction::CopyToClipboard => {
            let content = render_view_as_text(&vm);
            // Write to temp file AND pipe to system clipboard
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let tmp = std::env::temp_dir().join(format!("rigorix-view-{}.txt", now));
            let path = tmp.to_string_lossy().to_string();

            // Write to temp file (always works)
            let write_ok = std::fs::write(&tmp, &content).is_ok();

            // Try clipboard using system command (non-blocking)
            let clipboard_ok = copy_to_clipboard(&content);

            if write_ok {
                let msg = if clipboard_ok {
                    format!("Copied to clipboard (also at {})", path)
                } else {
                    format!("Copied to {}", path)
                };
                vm.copy_message = Some(msg);
                let clear_tx = vm_tx.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    let _ = clear_tx.send(VmCommand::CopyMessage(None)).await;
                });
            } else {
                vm.error = Some(format!("Failed to copy view content"));
            }
        }
        KeyAction::CancelGraceful | KeyAction::CancelImmediate => {
            vm.phase = ExecutionPhase::Cancelled;
        }
        KeyAction::None => {
            if *input_focus == InputFocus::CommandBar
                && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            {
                handle_command_bar_key(command_bar, key);
            }
        }
    }
    true
}

fn handle_command_bar_key(state: &mut CommandBarState, key: event::KeyEvent) {
    use event::KeyCode;
    match key.code {
        KeyCode::Char(c) if !key.modifiers.contains(event::KeyModifiers::CONTROL) => {
            state.text.insert(state.cursor, c);
            state.cursor += 1;
        }
        KeyCode::Backspace => {
            if state.cursor > 0 {
                state.cursor -= 1;
                state.text.remove(state.cursor);
            }
        }
        KeyCode::Delete => {
            if state.cursor < state.text.len() {
                state.text.remove(state.cursor);
            }
        }
        KeyCode::Left => state.cursor = state.cursor.saturating_sub(1),
        KeyCode::Right => state.cursor = (state.cursor + 1).min(state.text.len()),
        KeyCode::Up => state.history_up(),
        KeyCode::Down => state.history_down(),
        KeyCode::Home => state.cursor = 0,
        KeyCode::End => state.cursor = state.text.len(),
        _ => {}
    }
}

fn draw_frame(
    frame: &mut Frame<'_>,
    area: Rect,
    ctx: &WidgetContext,
    vm: &TuiViewModel,
    command_bar: &CommandBarState,
    input_focus: &InputFocus,
    selected_node: &Option<String>,
) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .split(area);
    status_bar::render(frame, chunks[0], vm);
    render_main_content(frame, chunks[1], ctx, vm, selected_node);
    cmd_bar::render(
        frame,
        chunks[2],
        command_bar,
        *input_focus == InputFocus::CommandBar,
    );
}

fn render_main_content(
    frame: &mut Frame<'_>,
    area: Rect,
    ctx: &WidgetContext,
    vm: &TuiViewModel,
    selected_node: &Option<String>,
) {
    match vm.active_view {
        ActiveView::Dashboard => views::dashboard::render(frame, area, ctx, vm, selected_node),
        ActiveView::Plan => views::plan::render(frame, area, vm),
        ActiveView::History => views::history::render(frame, area, vm),
        ActiveView::Events => views::events::render(frame, area, vm),
        ActiveView::Nodes => views::nodes::render(frame, area, vm, selected_node),
        ActiveView::Settings => views::settings::render(frame, area),
        ActiveView::Templates => views::templates::render(frame, area, vm),
        ActiveView::Clarification => views::clarification::render(frame, area),
        ActiveView::Diff => views::diff::render(frame, area, vm),
    }
}
