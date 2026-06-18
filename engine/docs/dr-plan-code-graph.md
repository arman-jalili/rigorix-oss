# Disaster Recovery Plan: code-graph Module

<!--
Canonical Reference: .pi/architecture/modules/code-graph.md
Last Updated: 2026-06-18
-->

## Scope

This DR plan covers the `code-graph` module — the code dependency graph
construction, analysis, persistence, and formatting system. The module builds
directed multigraphs of module relationships by scanning workspaces and parsing
import statements.

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 5 minutes | Graph construction is I/O-bound (filesystem scanning); persisted graphs load in < 1 second |
| RPO (Recovery Point Objective) | < 1 second | Graphs are rebuilt on demand; each `save()` is atomic |

## Backup Strategy

### What Gets Backed Up

| Data | Format | Location | Frequency |
|------|--------|----------|-----------|
| Persisted CodeGraphs | JSON (single file per graph) | `{storage_dir}/{uuid}.json` | On every `save()` |
| Graph index | JSON | `{storage_dir}/.index.json` | On every index mutation |

### Backup Schedule

- **Automatic:** Each `save()` call writes atomically (temp file → rename)
- **On-demand:** Call `persist_graph()` to trigger a save
- **Bulk:** Directory-level backup of `storage_dir`

### Retention

- All persisted graphs retained indefinitely
- Index file tracks all known graph UUIDs
- Old graphs can be pruned via `delete_graph()`

## Restore Procedure

### Single Graph Restore

```
1. Identify the graph UUID from the index: .index.json
2. Locate the graph file: {storage_dir}/{uuid}.json
3. Create a FilesystemCodeGraphRepository pointing to the storage dir
4. Call load(uuid) to deserialize the graph
5. Verify graph integrity: check sealed state, node_count, edge_count
```

### Full Storage Restore

```
1. Restore the storage directory from backup
2. Verify .index.json exists and is valid JSON
3. Spot-check random graph UUIDs: load and verify node/edge counts
4. Run validate-code-graph-contracts.sh to verify module integrity
```

### Cross-Platform Restore

Graph files are platform-independent JSON. Restoring on a different OS:
1. Copy the storage directory to the target system
2. Ensure the same schema_version (currently "1.0.0")
3. Initialize FilesystemCodeGraphRepository with the storage path

## Failover Plan

### Service Instance Failure

Since `CodeGraphServiceImpl` stores graphs in-memory:
1. Unprocessed graph data is lost if not persisted
2. Reconstruct graphs from filesystem persistence
3. All persisted graphs survive instance restart

### Failover Steps

```
1. Detect failure (graph operations return InternalError)
2. Initialize new CodeGraphServiceImpl instance
3. Load required graphs from FilesystemCodeGraphRepository
4. Re-run any failed graph construction/analysis
```

## RTO/RPO Verification

| Test | Frequency | Procedure |
|------|-----------|-----------|
| Restore single graph | Monthly | Save a graph, delete original, restore from backup, verify data integrity |
| Full storage restore | Quarterly | Back up storage dir, wipe it, restore from backup, verify all graphs load |
| Cross-platform restore | Quarterly | Copy storage dir to alternate platform, verify load and query |
| Builder recovery | Monthly | Run CodeGraphBuilder::build() on the same workspace, verify identical output |

## Failure Scenarios

| Scenario | Impact | Mitigation | Recovery Time |
|----------|--------|------------|---------------|
| Storage directory deleted | All persisted graphs lost | Regular backups | Depends on backup freshness |
| Index corruption | Cannot list graphs | Rebuild index from graph files via filesystem scan | < 1 minute |
| Graph file corruption | Single graph lost | Rebuild from workspace scan | < 5 minutes |
| Builder fails mid-scan | Partial graph constructed | Builder is transactional (all-or-nothing via service) | < 1 minute retry |

## Related Documents

- [Runbook](runbook-code-graph.md)
- [Architecture Module](../.pi/architecture/modules/code-graph.md)
- [API Contracts](../src/code_graph/interfaces/http/mod.rs)
- [Repository Implementation](../src/code_graph/infrastructure/repository/filesystem_repository.rs)
