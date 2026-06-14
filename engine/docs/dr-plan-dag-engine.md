# Disaster Recovery Plan: dag-engine Module

<!--
Canonical Reference: .pi/architecture/modules/dag-engine.md
Last Updated: 2026-06-14
-->

## Scope

This DR plan covers the `dag-engine` module — the DAG construction and planning
system that builds executable DAGs from templates and manages per-node execution
policies. The module is stateless at runtime (graphs are in-memory) but can
persist serialized graphs to disk for crash recovery and audit.

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 2 minutes | Graph construction is fast (O(V+E) topo sort); in-memory state can be recovered from persisted graphs |
| RPO (Recovery Point Objective) | < 1 second | Graphs are built fresh per execution; only the last seal state matters |

## Backup Strategy

### What to Back Up

The dag-engine module is primarily in-memory. If graph persistence is enabled
(via `TaskGraphRepository`), the following should be backed up:

| Directory | Contents | Backup Priority |
|-----------|----------|-----------------|
| `dag/` | Serialized TaskGraph files (`{uuid}.graph.json`) | Medium — enables execution recovery |
| `plan/` | PlanDiff audit trail files (`{uuid}.diff.json`) | Low — rebuildable from logs |

### Backup Schedule

| Type | Frequency | Retention | Method |
|------|-----------|-----------|--------|
| Incremental | Every hour | 24 hours | Archive new `.graph.json` files |
| Full | Daily | 7 days | Tar archive of `dag/` directory |

### Backup Command

```bash
#!/bin/bash
# Backup script for dag-engine graph data

BACKUP_DIR="/var/backups/rigorix/dag"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
DAG_DIR="${RIGORIX_DAG_DIR:-$HOME/.rigorix/dag}"

mkdir -p "$BACKUP_DIR/$TIMESTAMP"
cp -u "$DAG_DIR"/*.graph.json "$BACKUP_DIR/$TIMESTAMP/" 2>/dev/null || true
echo "Backup complete: $(ls "$BACKUP_DIR/$TIMESTAMP"/*.json 2>/dev/null | wc -l) files"
```

## Restore Procedure

### Full Restore

```bash
# 1. Stop the orchestrator
systemctl stop rigorix-orchestrator

# 2. Restore graph directory
RESTORE_POINT="/var/backups/rigorix/dag/20260614-120000"
DAG_DIR="${RIGORIX_DAG_DIR:-$HOME/.rigorix/dag}"
mkdir -p "$DAG_DIR"
cp "$RESTORE_POINT"/*.graph.json "$DAG_DIR/"

# 3. Start the orchestrator
systemctl start rigorix-orchestrator
```

### Point-in-Time Recovery

```bash
# Restore from specific timestamp
RESTORE_POINT="/var/backups/rigorix/dag/$1"
if [ ! -d "$RESTORE_POINT" ]; then
    echo "Usage: $0 <YYYYMMDD-HHMMSS>"
    echo "Available restore points:"
    ls /var/backups/rigorix/dag/
    exit 1
fi

DAG_DIR="${RIGORIX_DAG_DIR:-$HOME/.rigorix/dag}"
cp "$RESTORE_POINT"/*.graph.json "$DAG_DIR/"
echo "Restored from $RESTORE_POINT"
```

## Failover Plan

### Single-Node Failure

Since the dag-engine is in-memory and stateless:

1. **Detect failure:** Orchestrator detects graph service unavailability
2. **Failover:** Start a new `DagGraphServiceImpl` instance
3. **Recover:** If graph persistence is enabled, load the last sealed graph
   from disk via `TaskGraphRepository::load()`
4. **Resume:** Continue execution from the last completed node

### Data Center Failure

1. **Detect:** Health check monitors `/api/v1/dag/health`
2. **Activate DR site:** Start dag-engine services in secondary region
3. **No data loss:** Graphs are ephemeral — they are rebuilt from template
   definitions on the DR site
4. **Failback:** When primary recovers, switch back during a maintenance window

## RTO/RPO Verification

| Test | Frequency | Procedure |
|------|-----------|-----------|
| Restore from backup | Monthly | Execute full restore procedure, verify graph loads correctly |
| Failover test | Quarterly | Simulate service failure, measure RTO |
| Integrity check | Weekly | Run `check_dag-engine_contracts.sh` to verify all contracts |

## Dependencies

### Services

| Service | Dependency Type | Impact of Outage |
|---------|----------------|-----------------|
| tokio runtime | Hard | No async operations possible |
| serde/json | Hard | Cannot serialize/deserialize graphs |
| Filesystem (if persisted) | Soft | In-memory operations continue; persistence unavailable |

### External Systems

| System | Dependency Type | Impact of Outage |
|--------|----------------|-----------------|
| Event bus (if integrated) | Soft | Plan comparison events not emitted |

## Failure Scenarios

### Scenario: In-Memory Graph Lost

**Impact:** Current execution cannot continue. Nodes must be re-queued.

**Recovery:**

1. If graph persistence is enabled, load the last sealed TaskGraph:
   ```rust
   let graph = taskgraph_repository.load(dag_id).await?;
   ```
2. Rebuild execution state from the loaded graph (completed nodes set,
   ready queue, in-degree map).
3. Call `mark_completed` for already-completed nodes to rebuild the
   ready queue.
4. Resume execution.

### Scenario: Corrupted Persisted Graph

**Impact:** Cannot load graph from disk for recovery.

**Recovery:**

1. Check for `.graph.json.tmp` files (indicates crash during write)
2. If temp file exists and is valid JSON, rename to replace corrupted file
3. If unrecoverable, rebuild the graph from the original template
4. The orchestrator's `CompositeValidator` will re-validate the rebuilt plan

---
*Last updated: 2026-06-14*
