//! EventBridge — async subscriber converting engine events to ViewModel mutations.
//!
//! @canonical .pi/architecture/modules/tui.md#eventbridge
//! Implements: Contract Freeze — EventBridge component
//! Issue: issue-tui-contract-freeze
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

use tokio::sync::mpsc;

use crate::tui::view_model::ViewModelMutation;

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
// Event → ViewModel mapping reference
// ---------------------------------------------------------------------------

/// Maps an engine `ExecutionEvent` to a `ViewModelMutation`.
///
/// Implementation note: This is a stateless mapping function that
/// converts each event type to the corresponding mutation.
///
/// | Engine Event | ViewModel Mutation |
/// |-------------|-------------------|
/// | PlanningStarted | SetPhase(Planning) |
/// | PlanningCompleted | SetTemplateId, SetIntent |
/// | NodeStarted | NodeViewModel(status: InProgress) |
/// | NodeCompleted | NodeViewModel(status: Completed), UpdateMetrics |
/// | NodeFailed | NodeViewModel(status: Failed) |
/// | NodeRetrying | NodeViewModel(status: Retrying), AppendEvent |
/// | ToolExecuted | UpdateMetrics(tool_counts) |
/// | ExecutionCompleted | SetPhase(Completed) |
/// | ExecutionFailed | SetPhase(Failed), SetError |
/// | ExecutionCancelled | SetPhase(Cancelled) |
/// | BudgetWarning | UpdateLlmBudget |
pub fn event_to_mutation(
    event_type: &str,
    _payload: &serde_json::Value,
) -> Option<ViewModelMutation> {
    // Placeholder: implementation maps event type strings to mutations.
    // Implementation issue: read engine ExecutionEvent type and map fields.
    let _ = (event_type, _payload);
    None
}
