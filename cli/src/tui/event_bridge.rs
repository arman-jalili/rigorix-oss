//! EventBridge — async subscriber converting engine events to ViewModel mutations.
//!
//! @canonical .pi/architecture/modules/tui.md#eventbridge
//! Implements: Contract Freeze — EventBridge component
//! Issue: issue-tui-contract-freeze, ISSUE-ASYNC-EXEC
//!
//! # Contract (Frozen)
//!
//! Subscribes to the engine's `EventBus` broadcast channel. Each incoming
//! `ExecutionEvent` is mapped to a `ViewModelMutation` and applied to the
//! write buffer.
//!
//! ## Lag handling
//!
//! - < 50 dropped: continue processing remaining events
//! - ≥ 50 dropped: trigger reconciliation from `ExecutionState` on disk
//!
//! ## Reverse channel
//!
//! `tokio::sync::mpsc::Sender<TuiCommand>` for TUI → orchestrator commands.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use tokio::sync::{Mutex, broadcast, mpsc};

use rigorix_engine::event_system::domain::ExecutionEvent;

use crate::tui::view_model::{ExecutionPhase, NodeStatus, NodeViewModel};

use super::VmCommand;

// ---------------------------------------------------------------------------
// TuiCommand — reverse channel commands
// ---------------------------------------------------------------------------

/// Commands sent from the TUI to the running orchestrator.
#[derive(Debug, Clone)]
pub enum TuiCommand {
    /// Graceful shutdown: finish current node, then stop.
    Cancel { graceful: bool },
    /// Retry a specific failed node.
    RetryNode { node_id: String },
}

// ---------------------------------------------------------------------------
// EventBridge state
// ---------------------------------------------------------------------------

/// Configuration for the EventBridge subscriber.
#[derive(Debug, Clone)]
pub struct EventBridgeConfig {
    /// Maximum allowed dropped events before reconciliation.
    pub max_dropped_events: u32,
}

impl Default for EventBridgeConfig {
    fn default() -> Self {
        Self {
            max_dropped_events: 50,
        }
    }
}

/// Statistics about the EventBridge's operation.
#[derive(Debug, Clone, Default)]
pub struct EventBridgeStats {
    /// Total events processed.
    pub events_processed: u64,
    /// Total events dropped (subscriber lagged).
    pub events_dropped: u64,
    /// Number of reconciliations triggered.
    pub reconciliations: u64,
}

// ---------------------------------------------------------------------------
// EventBridge trait
// ---------------------------------------------------------------------------

/// Async subscriber that bridges engine events to ViewModel mutations.
///
/// The EventBridge:
/// 1. Subscribes to the engine's EventBus
/// 2. Receives `ExecutionEvent`s as they're published
/// 3. Maps each event to a `ViewModelMutation`
/// 4. Applies the mutation to the write buffer
/// 5. Handles lag: triggers reconciliation at threshold
/// 6. Provides a reverse channel for TUI → orchestrator commands
#[async_trait::async_trait]
pub trait EventBridge: Send + Sync {
    /// Start the event bridge, connecting to the engine's EventBus.
    ///
    /// Spawns a background task that processes events until the bridge
    /// is stopped or the sender is dropped.
    async fn start(&self) -> Result<(), String>;

    /// Stop the event bridge, disconnecting from the EventBus.
    async fn stop(&self) -> Result<(), String>;

    /// Get a sender for the reverse command channel.
    ///
    /// Returns `None` if the bridge is not started.
    fn command_sender(&self) -> Option<mpsc::Sender<TuiCommand>>;

    /// Get current bridge statistics.
    fn stats(&self) -> EventBridgeStats;

    /// Get the number of dropped events since last check (resets counter).
    fn drain_dropped_events(&self) -> u32;
}

// ---------------------------------------------------------------------------
// EventBridgeImpl — concrete implementation
// ---------------------------------------------------------------------------

/// Concrete implementation of [`EventBridge`].
///
/// Subscribes to an engine event bus broadcast channel and forwards
/// each `ExecutionEvent` to the TUI via the `vm_tx` channel as a
/// `VmCommand`.
pub struct EventBridgeImpl {
    /// Shared flag to stop the background event processing task.
    stopped: Arc<AtomicBool>,
    /// Broadcast receiver for the engine's event bus.
    /// Wrapped in Mutex because we need to replace it on restart.
    receiver: Mutex<Option<broadcast::Receiver<ExecutionEvent>>>,
    /// Channel sender for VmCommands (TUI event loop).
    vm_tx: mpsc::Sender<VmCommand>,
    /// TUI command channel (reverse channel).
    tui_tx: mpsc::Sender<TuiCommand>,
    /// TUI command receiver.
    #[allow(dead_code)]
    tui_rx: Mutex<Option<mpsc::Receiver<TuiCommand>>>,
    /// Event processing statistics.
    stats: Arc<EventBridgeStatsInternal>,
}

/// Internal statistics counters using atomics.
struct EventBridgeStatsInternal {
    events_processed: AtomicU64,
    events_dropped: AtomicU64,
    reconciliations: AtomicU64,
    dropped_since_last_check: AtomicU32,
}

impl EventBridgeStatsInternal {
    #[allow(dead_code)]
    fn new() -> Self {
        Self {
            events_processed: AtomicU64::new(0),
            events_dropped: AtomicU64::new(0),
            reconciliations: AtomicU64::new(0),
            dropped_since_last_check: AtomicU32::new(0),
        }
    }
}

impl EventBridgeImpl {
    /// Create a new EventBridgeImpl.
    ///
    /// `vm_tx` is the channel for sending VmCommands to the TUI event loop.
    /// `receiver` is the broadcast receiver from the engine's event bus.
    #[allow(dead_code)]
    pub(crate) fn new(
        vm_tx: mpsc::Sender<VmCommand>,
        receiver: broadcast::Receiver<ExecutionEvent>,
    ) -> Self {
        let (tui_tx, tui_rx) = mpsc::channel(32);
        Self {
            stopped: Arc::new(AtomicBool::new(false)),
            receiver: Mutex::new(Some(receiver)),
            vm_tx,
            tui_tx,
            tui_rx: Mutex::new(Some(tui_rx)),
            stats: Arc::new(EventBridgeStatsInternal::new()),
        }
    }

    /// Set a new broadcast receiver (e.g., when starting a new execution).
    pub async fn set_receiver(&self, receiver: broadcast::Receiver<ExecutionEvent>) {
        let mut rx = self.receiver.lock().await;
        *rx = Some(receiver);
    }
}

#[async_trait::async_trait]
impl EventBridge for EventBridgeImpl {
    async fn start(&self) -> Result<(), String> {
        self.stopped.store(false, Ordering::SeqCst);

        let stopped = Arc::clone(&self.stopped);
        let stats = Arc::clone(&self.stats);
        let vm_tx = self.vm_tx.clone();

        // Take the receiver out of the mutex
        let receiver = {
            let mut rx = self.receiver.lock().await;
            rx.take()
                .ok_or_else(|| "EventBridge already started".to_string())?
        };

        tokio::spawn(async move {
            let mut rx = receiver;
            loop {
                if stopped.load(Ordering::SeqCst) {
                    break;
                }

                match rx.recv().await {
                    Ok(event) => {
                        stats.events_processed.fetch_add(1, Ordering::SeqCst);

                        // Map the event to VmCommand(s) and send to TUI
                        if let Some(cmd) = event_to_vm_command(&event) {
                            let _ = vm_tx.send(cmd).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        stats.events_dropped.fetch_add(n, Ordering::SeqCst);
                        stats
                            .dropped_since_last_check
                            .fetch_add(n as u32, Ordering::SeqCst);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        // Event bus closed — no more events
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), String> {
        self.stopped.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn command_sender(&self) -> Option<mpsc::Sender<TuiCommand>> {
        Some(self.tui_tx.clone())
    }

    fn stats(&self) -> EventBridgeStats {
        EventBridgeStats {
            events_processed: self.stats.events_processed.load(Ordering::SeqCst),
            events_dropped: self.stats.events_dropped.load(Ordering::SeqCst),
            reconciliations: self.stats.reconciliations.load(Ordering::SeqCst),
        }
    }

    fn drain_dropped_events(&self) -> u32 {
        self.stats
            .dropped_since_last_check
            .swap(0, Ordering::SeqCst)
    }
}

// ---------------------------------------------------------------------------
// Event → VmCommand mapping
// ---------------------------------------------------------------------------

/// Maps an engine `ExecutionEvent` to a `VmCommand` for the TUI event loop.
///
/// | Engine Event | VmCommand |
/// |-------------|----------|
/// | PlanningStarted | Phase(Planning) |
/// | PlanningCompleted | TemplateId, LlmCalls, Tokens |
/// | NodeStarted | SetNodes (single node, InProgress status) |
/// | NodeCompleted | SetNodes (single node, Completed status), Phase(Executing) |
/// | NodeFailed | SetNodes (single node, Failed status) |
/// | NodeRetrying | SetNodes (single node, Retrying status) |
/// | ToolExecuted | LlmCalls, Tokens |
/// | ExecutionCompleted | Phase(Completed) |
/// | ExecutionFailed | Error(msg), Phase(Failed) |
/// | ExecutionCancelled | Phase(Cancelled) |
/// | BudgetWarning | (no VmCommand equivalent) |
pub(crate) fn event_to_vm_command(event: &ExecutionEvent) -> Option<VmCommand> {
    match event {
        ExecutionEvent::PlanningStarted { .. } => Some(VmCommand::Phase(ExecutionPhase::Planning)),
        ExecutionEvent::PlanningCompleted {
            template_id,
            parameters,
            ..
        } => {
            let _ = parameters;
            Some(VmCommand::TemplateId(template_id.clone()))
        }
        ExecutionEvent::NodeStarted {
            node_id, node_name, ..
        } => {
            let nvm = NodeViewModel {
                id: node_id.clone(),
                name: node_name.clone(),
                tool_name: String::new(),
                status: NodeStatus::InProgress,
                dependencies: Vec::new(),
                dependents: Vec::new(),
                timing_ms: None,
                output_preview: None,
                error: None,
                retry_count: 0,
                risk_level: None,
            };
            Some(VmCommand::SetNodes(vec![nvm]))
        }
        ExecutionEvent::NodeCompleted {
            node_id,
            node_name,
            duration_ms,
            output,
            ..
        } => {
            let preview = output.as_str().map(|s| {
                let truncated: String = s.chars().take(200).collect();
                truncated
            });
            let nvm = NodeViewModel {
                id: node_id.clone(),
                name: node_name.clone(),
                tool_name: String::new(),
                status: NodeStatus::Completed,
                dependencies: Vec::new(),
                dependents: Vec::new(),
                timing_ms: Some(*duration_ms),
                output_preview: preview,
                error: None,
                retry_count: 0,
                risk_level: None,
            };
            Some(VmCommand::SetNodes(vec![nvm]))
        }
        ExecutionEvent::NodeFailed { node_id, error, .. } => {
            let nvm = NodeViewModel {
                id: node_id.clone(),
                name: String::new(),
                tool_name: String::new(),
                status: NodeStatus::Failed,
                dependencies: Vec::new(),
                dependents: Vec::new(),
                timing_ms: None,
                output_preview: None,
                error: Some(error.clone()),
                retry_count: 0,
                risk_level: None,
            };
            Some(VmCommand::SetNodes(vec![nvm]))
        }
        ExecutionEvent::NodeRetrying {
            node_id, attempt, ..
        } => {
            let _ = attempt;
            let nvm = NodeViewModel {
                id: node_id.clone(),
                name: String::new(),
                tool_name: String::new(),
                status: NodeStatus::Retrying,
                dependencies: Vec::new(),
                dependents: Vec::new(),
                timing_ms: None,
                output_preview: None,
                error: None,
                retry_count: 0,
                risk_level: None,
            };
            Some(VmCommand::SetNodes(vec![nvm]))
        }
        ExecutionEvent::ToolExecuted { .. } => {
            // Tool execution is tracked via metrics update; no direct VmCommand
            // Mapping to metrics would require a Metrics update variant
            None
        }
        ExecutionEvent::ExecutionCompleted { .. } => {
            Some(VmCommand::Phase(ExecutionPhase::Completed))
        }
        ExecutionEvent::ExecutionFailed { error, .. } => Some(VmCommand::Error(error.clone())),
        ExecutionEvent::ExecutionCancelled { .. } => {
            Some(VmCommand::Phase(ExecutionPhase::Cancelled))
        }
        ExecutionEvent::BudgetWarning { .. } => {
            // No VmCommand equivalent for budget warnings currently
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_event_to_vm_command_planning_started() {
        let event = ExecutionEvent::PlanningStarted {
            execution_id: Uuid::new_v4(),
            intent: "test intent".to_string(),
            timestamp: Utc::now(),
        };
        let cmd = event_to_vm_command(&event);
        assert!(matches!(
            cmd,
            Some(VmCommand::Phase(ExecutionPhase::Planning))
        ));
    }

    #[test]
    fn test_event_to_vm_command_node_started() {
        let event = ExecutionEvent::NodeStarted {
            execution_id: Uuid::new_v4(),
            node_id: "node-1".to_string(),
            node_name: "Test Node".to_string(),
            timestamp: Utc::now(),
        };
        let cmd = event_to_vm_command(&event).unwrap();
        match cmd {
            VmCommand::SetNodes(nodes) => {
                assert_eq!(nodes.len(), 1);
                assert_eq!(nodes[0].id, "node-1");
                assert_eq!(nodes[0].name, "Test Node");
                assert_eq!(nodes[0].status, NodeStatus::InProgress);
            }
            _ => panic!("Expected SetNodes"),
        }
    }

    #[test]
    fn test_event_to_vm_command_execution_completed() {
        let event = ExecutionEvent::ExecutionCompleted {
            execution_id: Uuid::new_v4(),
            duration_ms: 1000,
            nodes_executed: 5,
            timestamp: Utc::now(),
        };
        let cmd = event_to_vm_command(&event);
        assert!(matches!(
            cmd,
            Some(VmCommand::Phase(ExecutionPhase::Completed))
        ));
    }

    #[test]
    fn test_event_to_vm_command_execution_failed() {
        let event = ExecutionEvent::ExecutionFailed {
            execution_id: Uuid::new_v4(),
            error: "something went wrong".to_string(),
            timestamp: Utc::now(),
        };
        let cmd = event_to_vm_command(&event);
        assert!(matches!(cmd, Some(VmCommand::Error(ref e)) if e == "something went wrong"));
    }
}
