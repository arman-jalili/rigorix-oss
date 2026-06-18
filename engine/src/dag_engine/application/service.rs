//! Service interfaces (use cases) for the DAG Engine bounded context.
//!
//! @canonical .pi/architecture/modules/dag-engine.md
//! Implements: Contract Freeze — DagGraphService and DagPlanningService traits
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for DAG construction,
//! validation, and planning:
//! - `DagGraphService`: DAG construction, sealing, and lifecycle management
//! - `DagPlanningService`: Plan comparison and impact analysis for audit
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;
use uuid::Uuid;

use crate::dag_engine::domain::{DagError, TaskNode};

use super::dto::{
    AddNodeInput, AddNodeOutput, ComparePlansInput, ComparePlansOutput, ConstructGraphInput,
    ConstructGraphOutput, GetGraphInput, GetGraphOutput, GetNodeInput, GetNodeOutput,
    ListNodesInput, ListNodesOutput, SealGraphInput, SealGraphOutput,
};

/// Core DAG graph service for construction, validation, and lifecycle management.
///
/// The DagGraphService sits between the TemplateEngine (which produces
/// node lists) and the ParallelExecutor (which consumes sealed graphs).
/// It handles:
///
/// 1. Graph construction from a list of TaskNodes (add_unchecked)
/// 2. Graph sealing with topological sort and cycle detection
/// 3. Node querying and lifecycle tracking
/// 4. Graph state management (ready queue, status checks)
///
/// # Lifecycle
///
/// 1. `construct_graph` — Create a new TaskGraph with initial nodes
/// 2. `add_node` — Add nodes one at a time (Phase 1)
/// 3. `seal_graph` — Seal the graph and run topological sort (Phase 2)
/// 4. `get_graph` — Retrieve the current graph state
///
/// # Cancellation Integration
///
/// The graph service cooperates with the Cancellation module:
/// - If a graph is being constructed during cancellation, `seal_graph`
///   should check the cancellation signal before starting topological sort
/// - Graph construction can be interrupted without data corruption
///   since the graph is immutable until sealed
#[async_trait]
pub trait DagGraphService: Send + Sync {
    /// Construct a new TaskGraph from a list of nodes.
    ///
    /// Creates a new TaskGraph, adds all nodes via `add_unchecked`,
    /// and returns the unsealed graph. The caller must call `seal_graph`
    /// to finalize the graph for execution.
    async fn construct_graph(
        &self,
        input: ConstructGraphInput,
    ) -> Result<ConstructGraphOutput, DagError>;

    /// Add a single node to an existing unsealed graph.
    ///
    /// Appends a TaskNode to the graph. Returns an error if the graph
    /// has already been sealed or if a node with the same ID exists.
    async fn add_node(&self, input: AddNodeInput) -> Result<AddNodeOutput, DagError>;

    /// Seal a graph and run topological sort with cycle detection.
    ///
    /// Transitions the graph from Phase 1 (unsealed) to Phase 2 (sealed).
    /// Runs Kahn's algorithm for topological sorting and cycle detection.
    /// After sealing, no more nodes can be added.
    async fn seal_graph(&self, input: SealGraphInput) -> Result<SealGraphOutput, DagError>;

    /// Retrieve the current state of a TaskGraph.
    async fn get_graph(&self, input: GetGraphInput) -> Result<GetGraphOutput, DagError>;

    /// Get a specific node from a graph by its ID.
    async fn get_node(&self, input: GetNodeInput) -> Result<GetNodeOutput, DagError>;

    /// List all nodes in a graph, optionally filtered by status.
    async fn list_nodes(&self, input: ListNodesInput) -> Result<ListNodesOutput, DagError>;

    /// Mark a node as completed during execution.
    async fn mark_node_completed(&self, dag_id: Uuid, node_id: Uuid) -> Result<(), DagError>;

    /// Get the set of nodes whose dependencies are all satisfied
    /// (ready to execute).
    async fn get_ready_nodes(&self, dag_id: Uuid) -> Result<Vec<Uuid>, DagError>;

    /// Check if the graph has been sealed.
    async fn is_sealed(&self, dag_id: Uuid) -> Result<bool, DagError>;
}

/// DAG planning service for plan comparison and impact analysis.
///
/// The DagPlanningService provides plan diff computation and impact
/// level classification for audit trails and approval workflows.
/// Every plan change is recorded with its impact level for review.
#[async_trait]
pub trait DagPlanningService: Send + Sync {
    /// Compare two plans and compute a structured diff.
    ///
    /// Compares an old plan (set of nodes) against a new plan and
    /// produces a PlanDiff with added, removed, modified, and
    /// unchanged nodes, plus an auto-computed ImpactLevel.
    ///
    /// # Audit Integration
    ///
    /// The resulting PlanDiff is emitted as a DagEvent::PlanCompared
    /// for audit trail recording.
    async fn compare_plans(&self, input: ComparePlansInput)
        -> Result<ComparePlansOutput, DagError>;

    /// Compute the impact level of a set of proposed changes.
    ///
    /// Given an existing plan and a proposed modification, compute
    /// the impact level without producing a full PlanDiff.
    /// Useful for quick policy decisions.
    async fn compute_impact(
        &self,
        old_nodes: Vec<TaskNode>,
        new_nodes: Vec<TaskNode>,
    ) -> Result<ImpactLevelResult, DagError>;
}

/// Result of an impact computation.
#[derive(Debug, Clone)]
pub struct ImpactLevelResult {
    /// The computed impact level.
    pub impact_level: crate::dag_engine::domain::ImpactLevel,
    /// Human-readable summary of why this impact level was assigned.
    pub summary: String,
}

/// Policy evaluation service for retry, backoff, and fallback decisions.
///
/// The ExecutionPolicyService evaluates ExecutionPolicy configurations
/// during node execution. It answers questions like:
/// - Should this node be retried given the failure type?
/// - How long should we wait before retrying?
/// - Is there a fallback node to execute?
/// - Is the policy configuration valid?
///
/// # Contract (Frozen)
/// - Retry decisions are purely based on policy + failure type
/// - Backoff uses exponential: backoff_ms * multiplier^attempt
/// - Fallback is only considered after max_retries exhausted
/// - All policy validation must pass for execution to proceed
#[async_trait]
pub trait ExecutionPolicyService: Send + Sync {
    /// Determine if a node should be retried after a failure.
    ///
    /// Checks:
    /// 1. Is the failure type in the policy's `retry_on` list?
    /// 2. Have we exhausted `max_retries`?
    ///
    /// Returns a RetryDecision with the outcome and reason.
    async fn should_retry(&self, input: ShouldRetryInput) -> Result<RetryDecision, DagError>;

    /// Compute the backoff delay before retrying.
    ///
    /// Uses exponential backoff:
    /// `delay = min(backoff_ms * multiplier^attempt, max_backoff_ms)`
    async fn compute_backoff(
        &self,
        input: ComputeBackoffInput,
    ) -> Result<ComputeBackoffOutput, DagError>;

    /// Validate an ExecutionPolicy configuration.
    ///
    /// Checks:
    /// - max_retries <= 255 (always true for u8, checked for semantics)
    /// - backoff_ms > 0
    /// - backoff_multiplier >= 1.0
    /// - max_backoff_ms >= backoff_ms
    async fn validate_policy(
        &self,
        input: ValidatePolicyInput,
    ) -> Result<ValidatePolicyOutput, DagError>;
}

// ---------------------------------------------------------------------------
// ExecutionPolicyService DTOs
// ---------------------------------------------------------------------------

/// Input for a retry decision.
#[derive(Debug, Clone)]
pub struct ShouldRetryInput {
    /// The execution policy governing this node.
    pub policy: crate::dag_engine::domain::ExecutionPolicy,
    /// The type of failure that occurred.
    pub failure_type: crate::failure_classification::domain::FailureType,
    /// The number of retries already attempted.
    pub retries_attempted: u8,
}

/// Decision about whether to retry a failed node.
#[derive(Debug, Clone)]
pub enum RetryDecision {
    /// Retry the node with the specified strategy.
    Retry {
        /// The retry strategy to apply.
        strategy: crate::failure_classification::domain::RetryStrategy,
        /// Next attempt number (1-indexed).
        attempt: u8,
        /// Reason for the retry decision.
        reason: String,
    },
    /// Do not retry; consider fallback or permanent failure.
    NoRetry {
        /// Reason the node will not be retried.
        reason: String,
        /// If true, attempt to execute the fallback node (if configured).
        use_fallback: bool,
    },
}

/// Input for computing backoff delay.
#[derive(Debug, Clone)]
pub struct ComputeBackoffInput {
    /// The execution policy containing backoff configuration.
    pub policy: crate::dag_engine::domain::ExecutionPolicy,
    /// The current retry attempt number (1-indexed).
    pub attempt: u8,
}

/// Output from computing backoff delay.
#[derive(Debug, Clone)]
pub struct ComputeBackoffOutput {
    /// The backoff delay in milliseconds.
    pub delay_ms: u64,
    /// The effective multiplier used.
    pub multiplier: f64,
    /// Human-readable explanation of the computation.
    pub explanation: String,
}

/// Input for validating an ExecutionPolicy.
#[derive(Debug, Clone)]
pub struct ValidatePolicyInput {
    /// The execution policy to validate.
    pub policy: crate::dag_engine::domain::ExecutionPolicy,
}

/// Output from validating an ExecutionPolicy.
#[derive(Debug, Clone)]
pub struct ValidatePolicyOutput {
    /// Whether the policy is valid.
    pub is_valid: bool,
    /// List of validation errors (empty if valid).
    pub errors: Vec<String>,
    /// List of validation warnings.
    pub warnings: Vec<String>,
}
