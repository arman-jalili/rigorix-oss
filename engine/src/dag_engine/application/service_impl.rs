//! Service implementations for the DAG Engine bounded context.
//!
//! @canonical .pi/architecture/modules/dag-engine.md
//! Implements: TaskGraph — DagGraphServiceImpl, DagPlanningServiceImpl
//! Issue: issue-taskgraph
//!
//! Concrete implementations of DagGraphService and DagPlanningService
//! that operate directly on TaskGraph domain objects in memory.
//!
//! # Design Decisions
//! - In-memory storage for graph construction phase
//! - Graph state is passed by dag_id which maps to an in-memory TaskGraph
//! - Planning service delegates to PlanDiff::compute for structural comparison

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use chrono::Utc;
use uuid::Uuid;

use crate::dag_engine::domain::{
    DagError, ImpactLevel, PlanDiff, TaskGraph, TaskNode,
};

use super::dto::{
    AddNodeInput, AddNodeOutput, ComparePlansInput, ComparePlansOutput, ConstructGraphInput,
    ConstructGraphOutput, GetGraphInput, GetGraphOutput, GetNodeInput, GetNodeOutput,
    ListNodesInput, ListNodesOutput, SealGraphInput, SealGraphOutput,
};
use super::service::{DagGraphService, DagPlanningService, ImpactLevelResult};

/// In-memory implementation of DagGraphService.
///
/// Stores TaskGraph instances in a HashMap keyed by UUID.
/// Not suitable for production multi-process use but provides
/// the contract-level behavior needed for testing and single-process
/// execution.
pub struct DagGraphServiceImpl {
    /// In-memory graph store.
    graphs: Mutex<HashMap<Uuid, TaskGraph>>,
}

impl DagGraphServiceImpl {
    /// Create a new empty DagGraphServiceImpl.
    pub fn new() -> Self {
        Self {
            graphs: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for DagGraphServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DagGraphService for DagGraphServiceImpl {
    async fn construct_graph(
        &self,
        input: ConstructGraphInput,
    ) -> Result<ConstructGraphOutput, DagError> {
        let mut graph = TaskGraph::new();
        let dag_id = Uuid::new_v4();

        for node in input.nodes {
            graph.add_unchecked(node)?;
        }

        let node_count = graph.node_count() as u32;
        let constructed_at = Utc::now();

        let mut graphs = self.graphs.lock().map_err(|e| DagError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;
        graphs.insert(dag_id, graph);

        Ok(ConstructGraphOutput {
            dag_id,
            graph: graphs.get(&dag_id).unwrap().clone(),
            node_count,
            constructed_at,
        })
    }

    async fn add_node(
        &self,
        input: AddNodeInput,
    ) -> Result<AddNodeOutput, DagError> {
        let mut graphs = self.graphs.lock().map_err(|e| DagError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;

        let graph = graphs.get_mut(&input.dag_id).ok_or_else(|| {
            DagError::InvalidGraph {
                reason: format!("Graph {} not found", input.dag_id),
            }
        })?;

        let node_id = input.node.id;
        graph.add_unchecked(input.node)?;

        Ok(AddNodeOutput {
            dag_id: input.dag_id,
            node_id,
            node_count: graph.node_count() as u32,
            added_at: Utc::now(),
        })
    }

    async fn seal_graph(
        &self,
        input: SealGraphInput,
    ) -> Result<SealGraphOutput, DagError> {
        let mut graphs = self.graphs.lock().map_err(|e| DagError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;

        let graph = graphs.get_mut(&input.dag_id).ok_or_else(|| {
            DagError::InvalidGraph {
                reason: format!("Graph {} not found", input.dag_id),
            }
        })?;

        let total_nodes = graph.node_count() as u32;
        graph.seal()?;

        let topo_order = graph.topological_order()
            .map(|o| o.to_vec())
            .unwrap_or_default();
        let processed_count = topo_order.len() as u32;
        let sealed_at = Utc::now();

        Ok(SealGraphOutput {
            graph: graph.clone(),
            topological_order: topo_order,
            processed_count,
            total_nodes,
            sealed_at,
        })
    }

    async fn get_graph(
        &self,
        input: GetGraphInput,
    ) -> Result<GetGraphOutput, DagError> {
        let graphs = self.graphs.lock().map_err(|e| DagError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;

        let graph = graphs.get(&input.dag_id).ok_or_else(|| {
            DagError::InvalidGraph {
                reason: format!("Graph {} not found", input.dag_id),
            }
        })?;

        Ok(GetGraphOutput {
            dag_id: input.dag_id,
            graph: graph.clone(),
            retrieved_at: Utc::now(),
        })
    }

    async fn get_node(
        &self,
        input: GetNodeInput,
    ) -> Result<GetNodeOutput, DagError> {
        let graphs = self.graphs.lock().map_err(|e| DagError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;

        let graph = graphs.get(&input.dag_id).ok_or_else(|| {
            DagError::InvalidGraph {
                reason: format!("Graph {} not found", input.dag_id),
            }
        })?;

        let node = graph.get_node(input.node_id).ok_or_else(|| {
            DagError::TaskNotFound { id: input.node_id }
        })?;

        Ok(GetNodeOutput {
            node: node.clone(),
            retrieved_at: Utc::now(),
        })
    }

    async fn list_nodes(
        &self,
        input: ListNodesInput,
    ) -> Result<ListNodesOutput, DagError> {
        let graphs = self.graphs.lock().map_err(|e| DagError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;

        let graph = graphs.get(&input.dag_id).ok_or_else(|| {
            DagError::InvalidGraph {
                reason: format!("Graph {} not found", input.dag_id),
            }
        })?;

        Ok(ListNodesOutput {
            nodes: graph.nodes.clone(),
            total_count: graph.nodes.len() as u32,
        })
    }

    async fn mark_node_completed(
        &self,
        dag_id: Uuid,
        node_id: Uuid,
    ) -> Result<(), DagError> {
        let mut graphs = self.graphs.lock().map_err(|e| DagError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;

        let graph = graphs.get_mut(&dag_id).ok_or_else(|| {
            DagError::InvalidGraph {
                reason: format!("Graph {} not found", dag_id),
            }
        })?;

        graph.mark_completed(node_id)
    }

    async fn get_ready_nodes(&self, dag_id: Uuid) -> Result<Vec<Uuid>, DagError> {
        let graphs = self.graphs.lock().map_err(|e| DagError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;

        let graph = graphs.get(&dag_id).ok_or_else(|| {
            DagError::InvalidGraph {
                reason: format!("Graph {} not found", dag_id),
            }
        })?;

        Ok(graph.ready_nodes())
    }

    async fn is_sealed(&self, dag_id: Uuid) -> Result<bool, DagError> {
        let graphs = self.graphs.lock().map_err(|e| DagError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;

        let graph = graphs.get(&dag_id).ok_or_else(|| {
            DagError::InvalidGraph {
                reason: format!("Graph {} not found", dag_id),
            }
        })?;

        Ok(graph.sealed)
    }
}

/// In-memory implementation of DagPlanningService.
///
/// Delegates plan comparison to PlanDiff::compute which implements
/// the structural node-by-node comparison logic.
pub struct DagPlanningServiceImpl;

impl DagPlanningServiceImpl {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl DagPlanningService for DagPlanningServiceImpl {
    async fn compare_plans(
        &self,
        input: ComparePlansInput,
    ) -> Result<ComparePlansOutput, DagError> {
        let diff = PlanDiff::compute(&input.old_nodes, &input.new_nodes);
        Ok(ComparePlansOutput {
            diff,
            compared_at: Utc::now(),
        })
    }

    async fn compute_impact(
        &self,
        old_nodes: Vec<TaskNode>,
        new_nodes: Vec<TaskNode>,
    ) -> Result<ImpactLevelResult, DagError> {
        let diff = PlanDiff::compute(&old_nodes, &new_nodes);
        let summary = match diff.impact_level {
            ImpactLevel::None => "No changes detected between plans".to_string(),
            ImpactLevel::Low => "Cosmetic or non-functional changes (e.g., intent text, reordering)".to_string(),
            ImpactLevel::Medium => "Behavioural changes within the same scope".to_string(),
            ImpactLevel::High => "Structural changes (tool bindings modified)".to_string(),
            ImpactLevel::Breaking => format!(
                "Breaking changes: {} added, {} removed, {} modified",
                diff.added.len(),
                diff.removed.len(),
                diff.modified.len(),
            ),
        };

        Ok(ImpactLevelResult {
            impact_level: diff.impact_level,
            summary,
        })
    }
}
