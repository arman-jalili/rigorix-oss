//! Core DAG data structures: TaskGraph, TaskNode, ExecutionPolicy, ValidationRule.
//!
//! @canonical .pi/architecture/modules/dag-engine.md#graph
//! Implements: TaskGraph — TaskGraph, TaskNode, ExecutionPolicy, ValidationRule
//! Issue: issue-taskgraph
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
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

use super::error::DagError;
use crate::failure_classification::domain::{FailureType, RetryStrategy};

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
    /// The nodes in this DAG.
    pub nodes: Vec<TaskNode>,

    /// O(1) UUID → position index. Rebuilt on deserialization.
    /// Not serialized — derived from `nodes` on load.
    #[serde(skip)]
    node_index: HashMap<Uuid, usize>,

    /// Topological ordering of node IDs (populated by `seal()`).
    pub topological_order: Option<Vec<Uuid>>,

    /// Whether the graph has been sealed (frozen for execution).
    pub sealed: bool,

    /// Internal execution tracking.
    ///
    /// These fields are populated after `seal()` and used during execution.
    /// They are not serialized for persistence (rebuilt on load).
    #[serde(skip)]
    pub execution_state: ExecutionState,
}

/// Execution tracking state for a TaskGraph.
///
/// Maintained in-memory during graph execution. Rebuilt from scratch
/// when a graph is loaded from storage.
#[derive(Debug, Clone, Default)]
pub struct ExecutionState {
    /// In-degree for each node: number of unresolved dependencies.
    /// A node is ready when its in-degree reaches 0.
    pub in_degree: HashMap<Uuid, usize>,

    /// Forward adjacency: for each node, the set of nodes that depend on it.
    /// Built from the reverse of each TaskNode's `dependencies`.
    pub dependents: HashMap<Uuid, Vec<Uuid>>,

    /// Set of node IDs that have been marked as completed.
    pub completed: HashSet<Uuid>,

    /// Queue of node IDs whose in-degree is 0 (ready to execute).
    pub ready_queue: VecDeque<Uuid>,

    /// Whether the topological sort has been computed.
    pub sorted: bool,
}

impl TaskGraph {
    /// Create a new empty TaskGraph in unsealed state.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            node_index: HashMap::new(),
            topological_order: None,
            sealed: false,
            execution_state: ExecutionState::default(),
        }
    }

    /// Add a node to the graph without validating the DAG structure.
    ///
    /// This is Phase 1 of construction. The node's dependencies are
    /// stored but not checked for cycles until `seal()` is called.
    /// Duplicate node IDs are rejected with `DagError::DuplicateTaskId`.
    ///
    /// # Errors
    /// - `DagError::InvalidGraph` if the graph is already sealed
    /// - `DagError::DuplicateTaskId` if a node with the same ID exists
    pub fn add_unchecked(&mut self, node: TaskNode) -> Result<(), DagError> {
        if self.sealed {
            return Err(DagError::InvalidGraph {
                reason: "Cannot add nodes to a sealed graph".to_string(),
            });
        }
        if self.node_index.contains_key(&node.id) {
            return Err(DagError::DuplicateTaskId { id: node.id });
        }
        let idx = self.nodes.len();
        self.node_index.insert(node.id, idx);
        self.nodes.push(node);
        Ok(())
    }

    /// Rebuild the UUID → position index from the node list.
    ///
    /// Called automatically by `seal()` to restore the index after
    /// deserialization (where `node_index` is `#[serde(skip)]`).
    fn rebuild_index_if_needed(&mut self) {
        if self.node_index.len() == self.nodes.len() && !self.nodes.is_empty() {
            return; // Index is already in sync
        }
        self.node_index.clear();
        for (i, node) in self.nodes.iter().enumerate() {
            self.node_index.insert(node.id, i);
        }
    }

    /// Seal the graph and run topological sort with cycle detection.
    ///
    /// This is Phase 2 of construction. Kahn's algorithm computes the
    /// topological ordering and detects cycles. After sealing:
    /// - `sealed` is set to true
    /// - `topological_order` contains the sorted node IDs
    /// - `execution_state` is initialised with in-degrees and dependents
    ///
    /// # Errors
    /// - `DagError::InvalidGraph` if already sealed, empty, or has
    ///   invalid dependency references
    /// - `DagError::CycleDetected` if a cycle is found, with
    ///   the count of processed vs total nodes
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

        // Ensure the node index is populated (may be empty after deserialization)
        self.rebuild_index_if_needed();

        // Validate all dependency references
        self.validate_dependencies()?;

        // Build initial in-degree from node dependencies (before Kahn's reduction)
        let mut initial_in_degree: HashMap<Uuid, usize> = HashMap::new();
        let mut dependents: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for node in &self.nodes {
            initial_in_degree.insert(node.id, node.dependencies.len());
            for dep_id in &node.dependencies {
                dependents.entry(*dep_id).or_default().push(node.id);
            }
        }

        // Run Kahn's algorithm
        let topo_order = self.kahns_algorithm(&initial_in_degree, &dependents)?;

        // Build the ready queue from nodes with initial in_degree == 0
        let mut ready_queue = VecDeque::new();
        for node in &self.nodes {
            if *initial_in_degree.get(&node.id).unwrap_or(&0) == 0 {
                ready_queue.push_back(node.id);
            }
        }

        self.topological_order = Some(topo_order);
        self.execution_state = ExecutionState {
            in_degree: initial_in_degree,
            dependents,
            completed: HashSet::new(),
            ready_queue,
            sorted: true,
        };
        self.sealed = true;
        Ok(())
    }

    /// Validate that all dependency references point to existing nodes.
    fn validate_dependencies(&self) -> Result<(), DagError> {
        let node_ids: HashSet<Uuid> = self.nodes.iter().map(|n| n.id).collect();
        let mut missing = Vec::new();

        for node in &self.nodes {
            for dep_id in &node.dependencies {
                if !node_ids.contains(dep_id) {
                    missing.push(*dep_id);
                }
            }
        }

        if !missing.is_empty() {
            return Err(DagError::DependencyNotFound { missing });
        }
        Ok(())
    }

    /// Run Kahn's algorithm for topological sorting with cycle detection.
    ///
    /// Takes pre-computed in_degree and dependents maps and processes
    /// them through Kahn's algorithm. Returns the topological ordering.
    ///
    /// # Algorithm
    /// 1. Start with nodes that have in-degree 0
    /// 2. Process each node: decrease in-degree of its dependents
    /// 3. If a dependent's in-degree reaches 0, add to process queue
    /// 4. If not all nodes are processed, a cycle exists
    fn kahns_algorithm(
        &self,
        initial_in_degree: &HashMap<Uuid, usize>,
        dependents: &HashMap<Uuid, Vec<Uuid>>,
    ) -> Result<Vec<Uuid>, DagError> {
        // Clone in_degree since Kahn's algorithm mutates it
        let mut in_degree = initial_in_degree.clone();

        let mut queue: VecDeque<Uuid> = VecDeque::new();
        let mut topo_order: Vec<Uuid> = Vec::with_capacity(self.nodes.len());

        // Start with nodes that have no dependencies (in_degree == 0)
        for node in &self.nodes {
            if *in_degree.get(&node.id).unwrap_or(&0) == 0 {
                queue.push_back(node.id);
            }
        }

        while let Some(node_id) = queue.pop_front() {
            topo_order.push(node_id);

            if let Some(deps) = dependents.get(&node_id) {
                for dep_id in deps {
                    if let Some(degree) = in_degree.get_mut(dep_id) {
                        *degree = degree.saturating_sub(1);
                        if *degree == 0 {
                            queue.push_back(*dep_id);
                        }
                    }
                }
            }
        }

        let processed = topo_order.len();
        let total = self.nodes.len();

        if processed < total {
            return Err(DagError::CycleDetected {
                found: processed,
                total,
            });
        }

        Ok(topo_order)
    }

    /// Mark a node as completed and update the ready queue.
    ///
    /// When a node completes, its dependents may have their in-degree
    /// reduced to 0, making them ready for execution.
    ///
    /// # Errors
    /// - `DagError::TaskNotFound` if the node ID does not exist
    /// - `DagError::InvalidGraph` if the graph has not been sealed
    pub fn mark_completed(&mut self, node_id: Uuid) -> Result<(), DagError> {
        if !self.sealed {
            return Err(DagError::InvalidGraph {
                reason: "Cannot mark nodes as completed before sealing".to_string(),
            });
        }

        if !self.node_index.contains_key(&node_id) {
            return Err(DagError::TaskNotFound { id: node_id });
        }

        if self.execution_state.completed.contains(&node_id) {
            return Ok(()); // Idempotent
        }

        self.execution_state.completed.insert(node_id);

        // Update dependents' in-degree and add to ready queue if 0
        if let Some(deps) = self.execution_state.dependents.get(&node_id).cloned() {
            for dep_id in deps {
                if let Some(degree) = self.execution_state.in_degree.get_mut(&dep_id) {
                    *degree = degree.saturating_sub(1);
                    if *degree == 0 {
                        self.execution_state.ready_queue.push_back(dep_id);
                    }
                }
            }
        }

        Ok(())
    }

    /// Return the IDs of nodes whose dependencies are all satisfied.
    ///
    /// Returns an empty Vec if the graph has not been sealed.
    /// Provides O(1) amortized access via the internal ready queue.
    pub fn ready_nodes(&self) -> Vec<Uuid> {
        if !self.sealed {
            return Vec::new();
        }
        self.execution_state.ready_queue.iter().copied().collect()
    }

    /// Pop the next ready node (removes it from the ready queue).
    pub fn pop_ready_node(&mut self) -> Option<Uuid> {
        self.execution_state.ready_queue.pop_front()
    }

    /// Check if all nodes have been completed.
    pub fn is_execution_complete(&self) -> bool {
        self.sealed && self.execution_state.completed.len() == self.nodes.len()
    }

    /// Get a node by its ID.
    pub fn get_node(&self, node_id: Uuid) -> Option<&TaskNode> {
        self.node_index
            .get(&node_id)
            .and_then(|&idx| self.nodes.get(idx))
    }

    /// Get a mutable reference to a node by its ID.
    pub fn get_node_mut(&mut self, node_id: Uuid) -> Option<&mut TaskNode> {
        self.node_index
            .get(&node_id)
            .copied()
            .and_then(|idx| self.nodes.get_mut(idx))
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

    /// Return the topological ordering (if sealed).
    pub fn topological_order(&self) -> Option<&[Uuid]> {
        self.topological_order.as_deref()
    }

    /// Return the set of completed node IDs.
    pub fn completed_nodes(&self) -> &HashSet<Uuid> {
        &self.execution_state.completed
    }

    /// Return the in-degree map.
    pub fn in_degree(&self) -> &HashMap<Uuid, usize> {
        &self.execution_state.in_degree
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
