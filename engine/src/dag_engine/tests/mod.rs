//! Unit tests for the DAG Engine module.
//!
//! @canonical .pi/architecture/modules/dag-engine.md
//! Implements: TaskGraph — unit tests for all public interfaces
//! Issue: issue-taskgraph
//!
//! Covers:
//! - TaskGraph construction (add_unchecked, seal)
//! - Kahn's algorithm topological sort
//! - Cycle detection with cycle path reporting
//! - Ready queue (O(1) access to nodes with all deps satisfied)
//! - Node completion and execution tracking
//! - ExecutionPolicy defaults
//! - PlanDiff computation and impact levels
//! - Service integration (DagGraphServiceImpl, DagPlanningServiceImpl)
//! - Error handling for all edge cases

pub(crate) use uuid::Uuid;

pub(crate) use crate::dag_engine::application::dto::*;

pub(crate) use crate::dag_engine::application::service::{
    ComputeBackoffInput, DagGraphService, DagPlanningService, ExecutionPolicyService,
    RetryDecision, ShouldRetryInput, ValidatePolicyInput,
};

pub(crate) use crate::dag_engine::application::service_impl::{
    DagGraphServiceImpl, DagPlanningServiceImpl, ExecutionPolicyServiceImpl,
};

pub(crate) use crate::dag_engine::domain::{
    DagError, ExecutionPolicy, ImpactLevel, PlanDiff, TaskGraph, TaskNode, ValidationRule,
};

pub(crate) use crate::failure_classification::domain::{FailureType, RetryStrategy};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_node(id: Uuid, name: &str, deps: Vec<Uuid>) -> TaskNode {
    TaskNode::new(id, name, "cargo build", deps, format!("Build {}", name))
}

fn make_node_with_tool(id: Uuid, name: &str, tool: &str, deps: Vec<Uuid>) -> TaskNode {
    TaskNode::new(id, name, tool, deps, format!("Run {}", name))
}

// ---------------------------------------------------------------------------
// TaskGraph Construction
// ---------------------------------------------------------------------------

mod completion;
mod diff;
mod graph;
mod ready;
mod topo;
