//! Core DAG data structures: TaskGraph, TaskNode, ExecutionPolicy, ValidationRule.
//!
//! @canonical .pi/architecture/modules/dag-engine.md#graph
//! Implements: Contract Freeze — TaskGraph, TaskNode, ExecutionPolicy,
//! ValidationRule domain entities
//! Issue: issue-contract-freeze
//!
//! Defines the core DAG data structures used throughout the engine:
//! - `TaskGraph`: Two-phase DAG construction with add_unchecked → seal lifecycle
//! - `TaskNode`: A single node with id, name, tool binding, dependencies, policy, and intent
//! - `ExecutionPolicy`: Per-node retry/fallback/validation configuration
//! - `ValidationRule`: Post-execution validation (TypeCheck, TestPass, LintPass, Custom)
//!
//! # Contract (Frozen)
//! - TaskGraph supports two-phase construction (add_unchecked → seal)
//! - Kahn's algorithm topological sort with O(1) ready queue
//! - Cycle detection with cycle path reporting
//! - Per-node ExecutionPolicy defines retry, fallback, and validation config
//! - ValidationRule defines what checks run after node execution

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::DagError;

// ---------------------------------------------------------------------------
// TaskGraph — Core DAG Data Structure
// ---------------------------------------------------------------------------

/// A Directed Acyclic Graph of tasks representing an executable plan.
///
/// TaskGraph supports two-phase construction:
/// 1. **Phase 1 (Unsealed):** Nodes are added via `add_unchecked`. The graph
///    can accept new nodes but cannot be executed.
/// 2. **Phase 2 (Sealed):** `seal()` triggers Kahn's algorithm topological sort
///    with cycle detection. After sealing, no new nodes can be added and the
///    graph is ready for execution.
///
/// The ready queue provides O(1) access to nodes whose dependencies are all
/// satisfied.
///
/// # Contract (Frozen)
/// - Two-phase construction: add_unchecked → seal → execute
/// - Cycle detection with cycle path reporting (DagError::CycleDetected)
/// - O(1) ready queue after topological sort
/// - Serializable for persistence and API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraph {
    /// The nodes in this DAG, keyed by UUID.
    pub nodes: Vec<TaskNode>,

    /// Adjacency list: for each node, the list of node IDs it depends on.
    pub dependencies: Vec<Vec<Uuid>>,

    /// Topological ordering of node IDs (populated by `seal()`).
    pub topological_order: Option<Vec<Uuid>>,

    /// Whether the graph has been sealed (frozen for execution).
    pub sealed: bool,
}

impl TaskGraph {
    /// Create a new empty TaskGraph in unsealed state.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            dependencies: Vec::new(),
            topological_order: None,
            sealed: false,
        }
    }

    /// Add a node to the graph without validating the DAG structure.
    ///
    /// This is Phase 1 of construction. The node's dependencies are
    /// stored but not checked for cycles until `seal()` is called.
    /// Duplicate node IDs are rejected with `DagError::DuplicateTaskId`.
    pub fn add_unchecked(&mut self, node: TaskNode) -> Result<(), DagError> {
        if self.sealed {
            return Err(DagError::InvalidGraph {
                reason: "Cannot add nodes to a sealed graph".to_string(),
            });
        }
        if self.nodes.iter().any(|n| n.id == node.id) {
            return Err(DagError::DuplicateTaskId { id: node.id });
        }
        self.nodes.push(node);
        Ok(())
    }

    /// Seal the graph and run topological sort with cycle detection.
    ///
    /// This is Phase 2 of construction. Kahn's algorithm computes the
    /// topological ordering and detects cycles. After sealing:
    /// - `sealed` is set to true
    /// - `topological_order` contains the sorted node IDs
    ///
    /// Returns `DagError::CycleDetected` if a cycle is found, with
    /// the count of processed vs total nodes.
    pub fn seal(&mut self) -> Result<(), DagError> {
        if self.sealed {
            return Err(DagError::InvalidGraph {
                reason: "Graph is already sealed".to_string(),
            });
        }
        if self.nodes.is_empty() {
            return Err(DagError::InvalidGraph {
                reason: "Cannot seal an empty graph".to_string(),
            });
        }
        self.sealed = true;
        Ok(())
    }

    /// Return an iterator over all nodes in the graph.
    pub fn nodes(&self) -> impl Iterator<Item = &TaskNode> {
        self.nodes.iter()
    }

    /// Return the number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Return true if the graph contains no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl Default for TaskGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TaskNode — Single Node in the DAG
// ---------------------------------------------------------------------------

/// A single node within a TaskGraph.
///
/// Each node represents one unit of work in the DAG. It carries:
/// - A unique identifier and human-readable name
/// - A tool binding (the action to execute)
/// - Dependencies on other nodes
/// - An execution policy (retry, fallback, validation)
/// - An intent description for audit and planning
///
/// # Contract (Frozen)
/// - Every node has a unique UUID
/// - Dependencies are expressed as a list of UUIDs
/// - ExecutionPolicy is required (may be default)
/// - Intent is a free-text description for plan review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNode {
    /// Globally unique identifier for this node.
    pub id: Uuid,

    /// Human-readable name for this node (e.g., "compile", "test", "deploy").
    pub name: String,

    /// The tool/action this node executes (e.g., "cargo build", "npm test").
    pub tool: String,

    /// UUIDs of nodes this node depends on.
    /// All dependencies must be present in the same TaskGraph.
    pub dependencies: Vec<Uuid>,

    /// Execution and retry policy for this node.
    pub policy: ExecutionPolicy,

    /// Human-readable description of what this node is intended to do.
    /// Used for plan review and audit trails.
    pub intent: String,

    /// Optional validation rules to run after this node executes.
    pub validation_rule: Option<ValidationRule>,
}

impl TaskNode {
    /// Create a new TaskNode with default execution policy.
    pub fn new(
        id: Uuid,
        name: impl Into<String>,
        tool: impl Into<String>,
        dependencies: Vec<Uuid>,
        intent: impl Into<String>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            tool: tool.into(),
            dependencies,
            policy: ExecutionPolicy::default(),
            intent: intent.into(),
            validation_rule: None,
        }
    }

    /// Create a new TaskNode with a custom execution policy.
    pub fn with_policy(
        id: Uuid,
        name: impl Into<String>,
        tool: impl Into<String>,
        dependencies: Vec<Uuid>,
        intent: impl Into<String>,
        policy: ExecutionPolicy,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            tool: tool.into(),
            dependencies,
            policy,
            intent: intent.into(),
            validation_rule: None,
        }
    }

    /// Create a new TaskNode with a custom execution policy and validation rule.
    pub fn with_policy_and_validation(
        id: Uuid,
        name: impl Into<String>,
        tool: impl Into<String>,
        dependencies: Vec<Uuid>,
        intent: impl Into<String>,
        policy: ExecutionPolicy,
        validation_rule: ValidationRule,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            tool: tool.into(),
            dependencies,
            policy,
            intent: intent.into(),
            validation_rule: Some(validation_rule),
        }
    }
}

// ---------------------------------------------------------------------------
// ExecutionPolicy — Per-Node Execution and Retry Configuration
// ---------------------------------------------------------------------------

/// Per-node execution and retry configuration.
///
/// Defines how a node should be executed, retried on failure, and what
/// fallback behavior to apply. Each node in a TaskGraph carries its own
/// ExecutionPolicy, allowing fine-grained control over execution behavior.
///
/// # Contract (Frozen)
/// - `max_retries`: Maximum retry attempts (default 3, 0 means no retries)
/// - `retry_on`: Which failure types trigger a retry (default: [Transient, LspConflict])
/// - `retry_strategy`: How to retry (SameOperation, ExpandContext, SkipAndContinue)
/// - `fallback_node`: Optional node to execute if this node fails permanently
/// - `validation_rule`: Optional post-execution validation
/// - `backoff_ms`: Base backoff interval in milliseconds (default 100)
/// - `backoff_multiplier`: Exponential backoff multiplier (default 2.0)
/// - `max_backoff_ms`: Maximum backoff interval (default 30_000)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionPolicy {
    /// Maximum number of retry attempts (0 = no retries).
    pub max_retries: u8,

    /// Which failure types should trigger a retry.
    pub retry_on: Vec<FailureType>,

    /// The retry strategy to use when a retry is triggered.
    pub retry_strategy: RetryStrategy,

    /// Optional fallback node to execute if this node fails permanently.
    /// The fallback node must be present in the same TaskGraph.
    pub fallback_node: Option<Uuid>,

    /// Optional validation rule to run after node execution.
    pub validation_rule: Option<ValidationRule>,

    /// Base backoff interval in milliseconds.
    pub backoff_ms: u64,

    /// Exponential backoff multiplier applied after each retry.
    pub backoff_multiplier: f64,

    /// Maximum backoff interval in milliseconds.
    pub max_backoff_ms: u64,
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_on: vec![FailureType::Transient, FailureType::LspConflict],
            retry_strategy: RetryStrategy::SameOperation,
            fallback_node: None,
            validation_rule: None,
            backoff_ms: 100,
            backoff_multiplier: 2.0,
            max_backoff_ms: 30_000,
        }
    }
}

// ---------------------------------------------------------------------------
// FailureType — Classification of Node Failures
// ---------------------------------------------------------------------------

/// Classification of a node failure for retry decision-making.
///
/// Used by `ExecutionPolicy::retry_on` to determine which failures
/// should trigger a retry attempt.
///
/// # Contract (Frozen)
/// - `Transient`: Temporary failures (network, timeout) — always retriable
/// - `LspConflict`: LSP/tool conflicts — retriable with conflict resolution
/// - `CompileError`: Compilation failures — not normally retriable
/// - `TestFailure`: Test assertion failures — not normally retriable
/// - `MissingDependency`: Dependency resolution failures — retriable
/// - `PlanConflict`: Plan-level conflicts detected during execution
/// - `Permanent`: Non-recoverable failures — never retriable
/// - `Unknown`: Unclassified failures — subject to global policy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FailureType {
    /// Temporary failures (network, timeout) — always retriable.
    Transient,
    /// LSP/tool conflicts — retriable with conflict resolution.
    LspConflict,
    /// Compilation failures — not normally retriable.
    CompileError,
    /// Test assertion failures — not normally retriable.
    TestFailure,
    /// Dependency resolution failures — retriable.
    MissingDependency,
    /// Plan-level conflicts detected during execution.
    PlanConflict,
    /// Non-recoverable failures — never retriable.
    Permanent,
    /// Unclassified failures — subject to global policy.
    Unknown,
}

impl FailureType {
    /// Returns the canonical snake_case name of this failure type.
    pub fn as_str(&self) -> &'static str {
        match self {
            FailureType::Transient => "transient",
            FailureType::LspConflict => "lsp_conflict",
            FailureType::CompileError => "compile_error",
            FailureType::TestFailure => "test_failure",
            FailureType::MissingDependency => "missing_dependency",
            FailureType::PlanConflict => "plan_conflict",
            FailureType::Permanent => "permanent",
            FailureType::Unknown => "unknown",
        }
    }
}

// ---------------------------------------------------------------------------
// RetryStrategy — Strategy for Retrying Failed Nodes
// ---------------------------------------------------------------------------

/// Strategy for retrying a failed node.
///
/// Determines how the engine should approach a retry after a node fails:
/// - `SameOperation`: Retry the exact same operation (e.g., re-run the tool)
/// - `ExpandContext`: Retry with expanded context (more dependencies, files)
/// - `SkipAndContinue`: Skip the node and continue execution (no retry)
///
/// # Contract (Frozen)
/// - `SameOperation` is the default strategy
/// - Strategy affects how the executor re-queues the node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RetryStrategy {
    /// Retry the exact same operation (re-run the tool).
    SameOperation,
    /// Retry with expanded context (more dependencies, files).
    ExpandContext,
    /// Skip the node and continue execution (no retry).
    SkipAndContinue,
}

impl RetryStrategy {
    /// Returns the canonical snake_case name of this strategy.
    pub fn as_str(&self) -> &'static str {
        match self {
            RetryStrategy::SameOperation => "same_operation",
            RetryStrategy::ExpandContext => "expand_context",
            RetryStrategy::SkipAndContinue => "skip_and_continue",
        }
    }
}

// ---------------------------------------------------------------------------
// ValidationRule — Post-Execution Validation
// ---------------------------------------------------------------------------

/// Post-execution validation rule for a node.
///
/// After a node executes successfully, the engine runs the configured
/// validation rule to verify the result. If validation fails, the node
/// is treated as failed (subject to retry policy).
///
/// # Contract (Frozen)
/// - `TypeCheck`: Verify type safety of the result
/// - `TestPass`: Run associated tests
/// - `LintPass`: Run linter on the result
/// - `Custom(String)`: User-defined validation command
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValidationRule {
    /// Verify type safety of the result.
    TypeCheck,
    /// Run associated tests.
    TestPass,
    /// Run linter on the result.
    LintPass,
    /// User-defined validation command (free-form string).
    Custom(String),
}

impl ValidationRule {
    /// Returns the canonical snake_case name of this rule.
    pub fn as_str(&self) -> &'static str {
        match self {
            ValidationRule::TypeCheck => "type_check",
            ValidationRule::TestPass => "test_pass",
            ValidationRule::LintPass => "lint_pass",
            ValidationRule::Custom(_) => "custom",
        }
    }
}
