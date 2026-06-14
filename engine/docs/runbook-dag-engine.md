# Runbook: dag-engine Module

<!--
Canonical Reference: .pi/architecture/modules/dag-engine.md
Last Updated: 2026-06-14
-->

## Overview

The `dag-engine` module compiles templates into executable Directed Acyclic
Graphs. It provides two-phase DAG construction (add nodes → seal), topological
sorting via Kahn's algorithm, cycle detection with path reporting, O(1) ready
queue, and per-node execution policies with retry and fallback configuration.

## Components

| Component | Type | Description |
|-----------|------|-------------|
| `TaskGraph` | Domain entity | Core DAG with two-phase construction, topo sort, ready queue |
| `TaskNode` | Domain entity | Single DAG node: id, name, tool, deps, policy, intent |
| `ExecutionPolicy` | Domain value object | Per-node retry/fallback/validation configuration |
| `DagGraphServiceImpl` | Application service | In-memory graph construction, sealing, querying |
| `DagPlanningServiceImpl` | Application service | Plan comparison via PlanDiff::compute |
| `ExecutionPolicyServiceImpl` | Application service | Retry decisions, backoff, policy validation |
| `TaskGraphRepository` | Repository trait | TaskGraph persistence contract |
| `PlanDiffRepository` | Repository trait | Plan diff audit trail contract |

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| tokio runtime | Yes | Async I/O for service operations |
| serde + serde_json | Yes | Graph and DTO serialization |
| chrono | Yes | Timestamps (ISO 8601 UTC) |
| uuid | Yes | Node, graph, and execution identifiers |
| thiserror | Yes | Structured error types |
| async-trait | Yes | Trait object safety for service traits |

### Initialization

1. Create a `DagGraphServiceImpl` instance for graph construction
2. (Optional) Create a `DagPlanningServiceImpl` for plan comparison
3. (Optional) Create a `ExecutionPolicyServiceImpl` for policy evaluation

```rust
use rigorix::dag_engine::application::service::*;
use rigorix::dag_engine::application::service_impl::*;
use rigorix::dag_engine::domain::*;

// Create services
let graph_service = DagGraphServiceImpl::new();
let planning_service = DagPlanningServiceImpl::new();
let policy_service = ExecutionPolicyServiceImpl::new();

// Construct a graph
let output = graph_service.construct_graph(ConstructGraphInput {
    nodes: vec![
        TaskNode::new(node_a, "compile", "cargo build", vec![], "Build project"),
        TaskNode::new(node_b, "test", "cargo test", vec![node_a], "Run tests"),
    ],
}).await?;

// Seal the graph (triggers topological sort + cycle detection)
let sealed = graph_service.seal_graph(SealGraphInput {
    dag_id: output.dag_id,
}).await?;
```

## Configuration Reference

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RIGORIX_MAX_RETRIES` | `3` | Default max retries per node |
| `RIGORIX_BACKOFF_MS` | `100` | Default base backoff interval (ms) |
| `RIGORIX_BACKOFF_MULTIPLIER` | `2.0` | Default exponential backoff multiplier |
| `RIGORIX_MAX_BACKOFF_MS` | `30000` | Default max backoff interval (ms) |

### Graph Directory Layout

```
~/.rigorix/
└── dag/
    ├── {dag_id}.graph.json           # Serialized TaskGraph
    └── {dag_id}.graph.json.tmp       # Temp file during write (should not persist)
```

## Graceful Shutdown

### Procedure

1. **Drain ready queue:** Allow in-flight nodes to complete their current
   operation. Do not pull new nodes from the ready queue.
2. **Save graph state:** If an execution is in progress, serialize the current
   TaskGraph to disk for recovery on restart.
3. **No explicit cleanup needed:** Sealed graphs are immutable — partial writes
   from crash leave the prior state intact.

### Signal Handling

| Signal | Behaviour | Recovery |
|--------|-----------|----------|
| SIGTERM | Graceful — drain queue, save state | Load last saved graph on restart |
| SIGINT | Interrupt — save current state | Partial state preserved |
| SIGKILL | Immediate | Prior state preserved intact |

## Common Failure Modes and Recovery

### Failure: Cycle Detected

**Symptoms:** `DagGraphService::seal_graph()` returns
`DagError::CycleDetected { found, total }`.

**Recovery:**

1. Check the `processed_count` vs `total_nodes` in the error — if
   `processed < total`, a cycle exists among the unprocessed nodes.
2. Review node dependencies for circular references.
3. Break the cycle by removing or reordering dependencies.
4. Reconstruct the graph without the cycle.

### Failure: Dependency Not Found

**Symptoms:** `DagGraphService::seal_graph()` returns
`DagError::DependencyNotFound { missing }`.

**Recovery:**

1. The `missing` field lists UUIDs that were referenced as dependencies
   but are not present in the graph.
2. Either add the missing nodes or remove the dependency references.
3. Reconstruct and seal the graph.

### Failure: Invalid Execution Policy

**Symptoms:** `ExecutionPolicyService::validate_policy()` returns
`ValidatePolicyOutput { is_valid: false }`.

**Recovery:**

1. Check the `errors` field for specific violations:
   - `backoff_ms` must be > 0
   - `backoff_multiplier` must be >= 1.0
   - `max_backoff_ms` must be >= `backoff_ms`
2. Correct the policy configuration and retry.
3. Warnings (e.g., `max_retries = 0`, empty `retry_on`) do not block
   execution but should be reviewed.

### Failure: Concurrent Graph Modification

**Symptoms:** `DagGraphService::add_node()` returns
`DagError::InvalidGraph { reason }`.

**Recovery:**

1. Check if the graph has already been sealed — nodes cannot be added
   to a sealed graph.
2. Create a new graph with the additional nodes if needed.
3. The in-memory implementation uses a `Mutex` — deadlock is prevented
   by short-lived lock scopes.

## Observability

### Metrics

| Metric | Source | Description |
|--------|--------|-------------|
| `dag_engine.graphs_constructed` | Counter | Total graph construction operations |
| `dag_engine.graphs_sealed` | Counter | Total successful seal operations |
| `dag_engine.cycles_detected` | Counter | Cycle detection events |
| `dag_engine.nodes_added` | Counter | Total nodes added to graphs |
| `dag_engine.nodes_completed` | Counter | Total nodes marked as completed |
| `dag_engine.nodes_ready` | Gauge | Current ready queue size |
| `dag_engine.plan_comparisons` | Counter | Total plan comparison operations |
| `dag_engine.retry_decisions` | Counter | Total retry decisions made |
| `dag_engine.policy_validations` | Counter | Total policy validation checks |
| `dag_engine.seal_duration_ms` | Histogram | Graph seal operation latency |

### Health Check

The `/api/v1/dag/health` endpoint returns:

```json
{
  "status": "ok",
  "graph_count": 5,
  "plan_diff_count": 12,
  "storage_path": "/var/lib/rigorix/dag"
}
```

### Structured Logging

Key log events:

| Event | Level | Context |
|-------|-------|---------|
| Graph constructed | INFO | dag_id, node_count |
| Graph sealed | INFO | dag_id, node_count, topo_order |
| Cycle detected | WARN | dag_id, processed, total |
| Node completed | DEBUG | dag_id, node_id |
| Node ready | DEBUG | dag_id, node_id |
| Plan compared | INFO | dag_id, added, removed, modified |
| Retry decision | INFO | node_id, failure_type, attempt, max_retries |
| Policy invalid | WARN | errors (list of validation errors) |

---
*Last updated: 2026-06-14*
