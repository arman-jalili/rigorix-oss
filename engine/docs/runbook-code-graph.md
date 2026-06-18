# Runbook: code-graph Module

<!--
Canonical Reference: .pi/architecture/modules/code-graph.md
Last Updated: 2026-06-18
-->

## Overview

The `code-graph` module constructs, analyzes, persists, and formats a directed
dependency graph of code modules. It scans workspace directories, parses import
statements, and produces a `CodeGraph` with typed nodes (`ModuleNode`) and
weighted edges (`ModuleEdge`) representing dependency relationships.

## Components

| Component | Type | Description |
|-----------|------|-------------|
| `CodeGraph` | Domain entity | Directed multigraph with two-phase construction (add_node → seal) |
| `ModuleNode` | Domain entity | Single code module: id, name, kind, path, metadata |
| `ModuleEdge` | Domain entity | Typed, directed relationship: source, target, kind, weight |
| `NodeKind` | Enum | Classification: File, Package, Component, Directory, External, Aggregate, Custom |
| `EdgeKind` | Enum | Relationship type: Imports, Extends, Implements, DependsOn, Contains, References, Calls, Custom |
| `CodeGraphError` | Error enum | Structured errors with thiserror derive |
| `CodeGraphEvent` | Event enum | Lifecycle events for audit and integration |
| `CodeGraphService` | Service trait | Graph construction, node/edge management, sealing, persistence |
| `CodeGraphAnalyzer` | Service trait | Dependency analysis, cycle detection, impact analysis |
| `CodeGraphFormatter` | Service trait | Output formatting: Mermaid, DOT, Tree, JSON, List |
| `CodeGraphImporter` | Service trait | Batch import of nodes and edges |
| `CodeGraphBuilder` | Implementation | Workspace scanning with multi-language import parsing |
| `CodeGraphRepository` | Repository trait | CRUD for CodeGraph persistence |

## Startup Sequence

1. **Initialize graph service** — `CodeGraphServiceImpl::new()` creates in-memory graph store
2. **Configure repository** — Choose `InMemoryCodeGraphRepository` (testing) or `FilesystemCodeGraphRepository` (production)
3. **Initialize formatter** — `CodeGraphFormatterImpl::new()` for graph visualization (stateless)
4. **Initialize builder** — `CodeGraphBuilder::new(service, roots, extensions, include_external)` for workspace scanning

## Dependencies

| Dependency | Type | Notes |
|------------|------|-------|
| `serde` | External | JSON serialization for persistence and API |
| `serde_json` | External | JSON formatting |
| `uuid` | External | Node and graph identification |
| `chrono` | External | Timestamps for events and metadata |
| `async-trait` | External | Async trait support for service interfaces |
| `thiserror` | External | Error type derivation |
| `tokio` | External | Async runtime for filesystem operations |
| `tempfile` | Dev | Test isolation with temporary directories |

## Graceful Shutdown

The code-graph module has no background threads or long-lived connections.
Shutdown is handled by dropping the service/repository instances:

1. **For in-memory graphs** — Data is lost on drop; call `persist_graph` first if needed
2. **For filesystem graphs** — Data is already persisted on each `save()` call
3. **No draining or flush required** — All operations are synchronous from the caller's perspective

## Common Failure Modes

### Graph Not Found
- **Error:** `CodeGraphError::InvalidOperation { reason: "Graph not found: {id}" }`
- **Cause:** Attempting to operate on a graph ID that doesn't exist
- **Recovery:** Check the graph ID; create a new graph if needed
- **Prevention:** Use `exists()` before operations on untrusted IDs

### Graph Is Sealed
- **Error:** `CodeGraphError::GraphSealed { operation: "add_node" }`
- **Cause:** Attempting to add nodes/edges to a sealed graph
- **Recovery:** Create a new graph and re-add nodes
- **Prevention:** Check `is_sealed()` before modification operations

### Empty Graph
- **Error:** `CodeGraphError::EmptyGraph`
- **Cause:** Attempting to seal a graph with no nodes
- **Recovery:** Add at least one node before sealing
- **Prevention:** Always add nodes before calling `seal()`

### Duplicate Node/Edge
- **Error:** `CodeGraphError::DuplicateNodeId { id }` / `CodeGraphError::DuplicateEdge { ... }`
- **Cause:** Node with same UUID or edge with same source/target/kind already exists
- **Recovery:** Generate new UUIDs for new nodes
- **Prevention:** Let the service generate UUIDs via `add_node()`

## Configuration Reference

| Parameter | Default | Description |
|-----------|---------|-------------|
| `storage_dir` | (required) | Directory for filesystem persistence |
| `extensions` | `["rs", "ts", "js", "py"]` | Source file extensions to scan |
| `include_external` | `false` | Whether to include external deps as nodes |
| `schema_version` | `"1.0.0"` | CodeGraph data format version |

## Observability

### Metrics
- `code_graph.node_count` — Number of nodes in a graph
- `code_graph.edge_count` — Number of edges in a graph
- `code_graph.graphs_persisted` — Total graphs saved to storage

### Logging
- Module uses `tracing` for structured logging
- All service operations are instrumented
- Events emitted via `CodeGraphEvent` for audit trail

### Health
- GET `/api/v1/code-graph/health` — Returns status, graph count, storage path
- Uses `HealthResponse` schema from HTTP API contracts

## Related Documents

- [Architecture Module](../.pi/architecture/modules/code-graph.md)
- [DR Plan](dr-plan-code-graph.md)
- [API Contracts](../src/code_graph/interfaces/http/mod.rs)
