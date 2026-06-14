//! Service interfaces (use cases) for the State Persistence bounded context.
//!
//! @canonical .pi/architecture/modules/state-persistence.md
//! Implements: Contract Freeze — StateManagerService and GraphManagerService traits
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for state persistence:
//! saving/loading execution state, managing per-node state, and persisting
//! execution graphs for TUI history. All methods are async and return domain
//! error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;
use uuid::Uuid;

use crate::state_persistence::domain::StateError;

use super::dto::{
    ExecutionSummary, ListExecutionsInput, ListExecutionsOutput, LoadStateInput, LoadStateOutput,
    NodeStateChangedInput, NodeStateChangedOutput, SaveStateInput, SaveStateOutput,
};

/// Central state persistence service that manages execution state on disk.
///
/// The StateManagerService sits between the Orchestrator (which drives
/// execution) and the filesystem/persistence layer. Every state change
/// during execution passes through this service, which:
///
/// 1. Saves execution state at each phase (Pending → Running → Completed/Failed)
/// 2. Loads execution state for recovery or inspection
/// 3. Tracks per-node state transitions (Pending → InProgress → Completed/Failed/Skipped)
/// 4. Lists available executions for TUI or CLI history
///
/// # Persistence Pattern
///
/// All state writes use atomic write-rename for crash safety:
/// 1. Serialise state to `{execution_id}.json.tmp`
/// 2. `fs::rename` to `{execution_id}.json`
///
/// On POSIX, `rename(2)` is atomic — a power failure during write leaves
/// the original file intact.
///
/// # Cancellation Integration
///
/// The state manager cooperates with the Cancellation module:
/// - When an execution is cancelled, the final state is saved with
///   `ExecutionStatus::Cancelled` before the orchestrator returns
/// - Partial state on disk is always valid (atomic writes guarantee this)
#[async_trait]
pub trait StateManagerService: Send + Sync {
    /// Save the current execution state to persistent storage.
    ///
    /// Uses atomic write-rename: writes to a `.tmp` file, then atomically
    /// renames to the final path. If a hard link or filesystem error occurs,
    /// returns `StateError::IoError`.
    ///
    /// # Performance
    /// Expected to complete in < 10ms for typical state sizes.
    async fn save_state(&self, input: SaveStateInput) -> Result<SaveStateOutput, StateError>;

    /// Load an execution state from persistent storage.
    ///
    /// Reads and deserialises the state file for the given execution ID.
    /// If the file does not exist, returns `StateError::StateNotFound`.
    /// If the file is corrupted, returns `StateError::CorruptedState`.
    async fn load_state(&self, input: LoadStateInput) -> Result<LoadStateOutput, StateError>;

    /// Update the state of a single node within an execution.
    ///
    /// Convenience method that:
    /// 1. Loads the current state
    /// 2. Applies the node state change
    /// 3. Saves the updated state
    ///
    /// This is an atomic operation from the caller's perspective —
    /// either all three steps succeed or none persist.
    async fn update_node_state(
        &self,
        input: NodeStateChangedInput,
    ) -> Result<NodeStateChangedOutput, StateError>;

    /// List all available executions.
    ///
    /// Scans the state directory for state files and returns a summary
    /// of each execution including status and timing.
    ///
    /// If the directory does not exist, returns an empty list (not an error).
    /// Corrupted state files are skipped and logged via events.
    async fn list_executions(
        &self,
        input: ListExecutionsInput,
    ) -> Result<ListExecutionsOutput, StateError>;

    /// Delete an execution state from persistent storage.
    ///
    /// Removes the state file for the given execution ID.
    /// If the file does not exist, returns `Ok(())` (idempotent).
    async fn delete_state(&self, execution_id: Uuid) -> Result<(), StateError>;
}

/// Graph persistence service that manages ExecutionGraph records.
///
/// The GraphManagerService provides CRUD operations on persisted
/// execution graphs for TUI "view past execution" mode.
///
/// # Data Flow
/// 1. After an execution completes, the orchestrator builds an
///    `ExecutionGraph` from the final state + drained events
/// 2. `save_graph` persists it via this service
/// 3. TUI queries graphs via `list_graphs`, `get_graph`
/// 4. Old graphs are cleaned up via `delete_graph`
#[async_trait]
pub trait GraphManagerService: Send + Sync {
    /// Persist an execution graph.
    ///
    /// Saves the graph to a dedicated graph store (separate from state files
    /// since graphs are larger and less frequently accessed).
    async fn save_graph(&self, graph: &ExecutionSummary) -> Result<(), StateError>;

    /// Load an execution graph by its graph ID.
    async fn load_graph(&self, graph_id: Uuid) -> Result<ExecutionSummary, StateError>;

    /// List all available execution graphs, ordered by most recent first.
    async fn list_graphs(&self, limit: u32) -> Result<ListExecutionsOutput, StateError>;

    /// Delete an execution graph.
    async fn delete_graph(&self, graph_id: Uuid) -> Result<(), StateError>;
}
