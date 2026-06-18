//! OrchestratorSpawner — background orchestrator task management.
//!
//! @canonical .pi/architecture/modules/tui.md#orchestrator-spawner
//! Implements: Contract Freeze — OrchestratorSpawner component
//! Issue: issue-tui-contract-freeze
//!
//! # Contract (Frozen)
//!
//! Spawns the engine orchestrator in a `tokio::spawn`'d background task.
//! Builds the same `OrchestratorBuilder` as `cli_boundary`. Manages
//! lifecycle — start, graceful cancel, immediate abort — keeping UI
//! responsive.

use std::sync::Mutex;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::cli_boundary::config::CliConfig;
use crate::cli_boundary::error::CliError;

use super::event_bridge::TuiCommand;

// ---------------------------------------------------------------------------
// Orchestrator state
// ---------------------------------------------------------------------------

/// Current state of the spawned orchestrator task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrchestratorTaskState {
    /// No orchestrator running.
    Idle,
    /// Orchestrator is running in a background task.
    Running,
    /// Orchestrator completed successfully.
    Completed,
    /// Orchestrator failed.
    Failed,
    /// Orchestrator was cancelled.
    Cancelled,
}

// ---------------------------------------------------------------------------
// OrchestratorSpawner trait
// ---------------------------------------------------------------------------

/// Manages the lifecycle of a background orchestrator task.
///
/// The spawner is responsible for:
/// 1. Building the `OrchestratorService` from config
/// 2. Spawning it in a `tokio::spawn`'d task
/// 3. Providing a `CancellationToken` for cancellation
/// 4. Tracking task state (running, completed, failed, cancelled)
/// 5. Exposing the EventBus for subscription
#[async_trait::async_trait]
pub trait OrchestratorSpawner: Send + Sync {
    /// Spawn a new orchestrator run in a background task.
    ///
    /// Builds the orchestrator service from config, subscribes the
    /// EventBridge, and spawns the run. Returns immediately —
    /// the orchestrator runs in the background.
    ///
    /// # Errors
    ///
    /// Returns `CliError` if the orchestrator builder fails or config
    /// is invalid.
    async fn spawn_run(&self, config: CliConfig, intent: String) -> Result<(), CliError>;

    /// Spawn a plan-only operation in a background task.
    async fn spawn_plan_only(&self, config: CliConfig, intent: String) -> Result<(), CliError>;

    /// Request graceful cancellation of the running orchestrator.
    async fn cancel_graceful(&self) -> Result<(), CliError>;

    /// Request immediate abort of the running orchestrator.
    async fn cancel_immediate(&self) -> Result<(), CliError>;

    /// Get the current task state.
    fn task_state(&self) -> OrchestratorTaskState;

    /// Get a sender for TUI commands (reverse channel).
    fn command_sender(&self) -> Option<mpsc::Sender<TuiCommand>>;
}

// ---------------------------------------------------------------------------
// OrchestratorSpawnerImpl — concrete implementation
// ---------------------------------------------------------------------------

use super::VmCommand;
use super::view_model::ExecutionPhase;

/// Concrete implementation of [`OrchestratorSpawner`].
///
/// Stores a `Sender<VmCommand>` so background tasks can send results
/// (nodes, template ID, LLM metrics, errors) back to the TUI event loop.
pub struct OrchestratorSpawnerImpl {
    cancellation_token: CancellationToken,
    state: Mutex<OrchestratorTaskState>,
    vm_tx: mpsc::Sender<VmCommand>,
    tui_tx: mpsc::Sender<TuiCommand>,
}

#[allow(dead_code)]
impl OrchestratorSpawnerImpl {
    /// Create a new spawner.
    ///
    /// `vm_tx` is the channel for sending results (nodes, template ID,
    /// metrics, errors) back to the TUI event loop. `tui_tx` is the
    /// reverse channel for TUI → orchestrator commands (cancel, retry).
    pub(crate) fn new(
        cancellation_token: CancellationToken,
        vm_tx: mpsc::Sender<VmCommand>,
        tui_tx: mpsc::Sender<TuiCommand>,
    ) -> Self {
        Self {
            cancellation_token,
            state: Mutex::new(OrchestratorTaskState::Idle),
            vm_tx,
            tui_tx,
        }
    }

    /// Set the task state.
    fn set_state(&self, new_state: OrchestratorTaskState) {
        if let Ok(mut state) = self.state.lock() {
            *state = new_state;
        }
    }
}

#[async_trait::async_trait]
impl OrchestratorSpawner for OrchestratorSpawnerImpl {
    async fn spawn_run(&self, config: CliConfig, intent: String) -> Result<(), CliError> {
        self.set_state(OrchestratorTaskState::Running);
        let tx = self.vm_tx.clone();
        let ct = self.cancellation_token.clone();

        tokio::spawn(async move {
            let repo_root = config.repo_root.clone();
            match crate::cli_boundary::orchestrator::build_orchestrator(
                config,
                ct,
                repo_root.clone(),
            )
            .await
            {
                Ok((orch, _svc)) => {
                    let input = rigorix_engine::orchestrator::application::dto::RunInput {
                        intent,
                        config: serde_json::Value::Null,
                        repo_root,
                        enforcement_preset: None,
                    };
                    match orch.run(input).await {
                        Ok(output) => {
                            let _ = tx.send(VmCommand::ExecutionId(output.execution_id)).await;
                            let _ = tx.send(VmCommand::Phase(ExecutionPhase::Completed)).await;
                        }
                        Err(e) => {
                            let _ = tx
                                .send(VmCommand::Error(format!("Run failed: {e}")))
                                .await;
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(VmCommand::Error(e.to_string())).await;
                }
            }
        });
        Ok(())
    }

    async fn spawn_plan_only(&self, config: CliConfig, intent: String) -> Result<(), CliError> {
        self.set_state(OrchestratorTaskState::Running);
        let tx = self.vm_tx.clone();
        let ct = self.cancellation_token.clone();

        tokio::spawn(async move {
            let repo_root = config.repo_root.clone();
            match crate::cli_boundary::orchestrator::build_orchestrator(
                config,
                ct,
                repo_root.clone(),
            )
            .await
            {
                Ok((orch, _svc)) => {
                    let input = rigorix_engine::orchestrator::application::dto::PlanOnlyInput {
                        intent,
                        config: serde_json::Value::Null,
                        repo_root,
                    };
                    match orch.plan_only(input).await {
                        Ok(output) => {
                            let exec_id = output.plan["execution_id"]
                                .as_str()
                                .and_then(|s| s.parse().ok());
                            if let Some(id) = exec_id {
                                let _ = tx.send(VmCommand::ExecutionId(id)).await;
                            }
                            if let Some(toml) = output.plan["generated_toml"].as_str() {
                                let tid =
                                    output.plan["template_id"].as_str().unwrap_or("unknown");
                                let tpl_dir = std::path::PathBuf::from(".rigorix/templates");
                                let tpl_path = tpl_dir.join(format!("{tid}.toml"));
                                let _ = tokio::fs::create_dir_all(&tpl_dir).await;
                                let _ = tokio::fs::write(&tpl_path, toml).await;
                                let _ = tx.send(VmCommand::TemplateId(tid.to_string())).await;
                            }
                            if let Some(calls) = output.plan["llm_calls_used"].as_u64() {
                                let _ = tx.send(VmCommand::LlmCalls(calls)).await;
                            }
                            if let Some(tokens) = output.plan["llm_tokens_used"].as_u64() {
                                let _ = tx.send(VmCommand::Tokens(tokens)).await;
                            }
                            // Parse graph nodes via the shared helper
                            let nodes = super::parse_graph_nodes(&output.graph);
                            if !nodes.is_empty() {
                                let _ = tx.send(VmCommand::SetNodes(nodes)).await;
                            }
                            let _ = tx
                                .send(VmCommand::Phase(ExecutionPhase::Completed))
                                .await;
                        }
                        Err(e) => {
                            let _ = tx
                                .send(VmCommand::Error(format!("Plan failed: {e}")))
                                .await;
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(VmCommand::Error(e.to_string())).await;
                }
            }
        });
        Ok(())
    }

    async fn cancel_graceful(&self) -> Result<(), CliError> {
        self.cancellation_token.cancel();
        self.set_state(OrchestratorTaskState::Cancelled);
        Ok(())
    }

    async fn cancel_immediate(&self) -> Result<(), CliError> {
        self.cancellation_token.cancel();
        self.set_state(OrchestratorTaskState::Cancelled);
        Ok(())
    }

    fn task_state(&self) -> OrchestratorTaskState {
        self.state.lock().map(|s| *s).unwrap_or(OrchestratorTaskState::Failed)
    }

    fn command_sender(&self) -> Option<mpsc::Sender<TuiCommand>> {
        Some(self.tui_tx.clone())
    }
}
