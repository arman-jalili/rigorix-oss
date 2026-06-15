# Disaster Recovery Plan: execution-engine

## Overview

Recovery plan for the execution-engine module in the event of process crash, node failure, or data corruption. The execution engine is stateless between runs — each execution is ephemeral — but in-flight state must be preserved for crash recovery.

## RTO/RPO Targets

| Metric | Target | Notes |
|--------|--------|-------|
| RTO (Recovery Time Objective) | < 30s | Auto-restart via process supervisor (systemd/launchd) |
| RPO (Recovery Point Objective) | < 1 execution | In-flight state persisted per-node completion |

## Failure Scenarios

### Scenario 1: Process Crash During Execution

**Impact:** In-flight executions are lost. Completed nodes' results are persisted.

**Recovery:**
1. Process supervisor (systemd/launchd) automatically restarts the process
2. On restart, scan storage for incomplete executions via `ExecutionResultRepository`
3. For each incomplete execution:
   - Load saved `NodeExecutionState` from `ExecutionResultRepository::load_state()`
   - Identify completed nodes (nodes with `NodeStatus::Completed`)
   - Identify remaining nodes (nodes with `Pending`/`Ready` status)
   - Reconstruct the TaskGraph from the DAG Engine
   - Resume execution from the last completed node
4. Log: `"Recovered execution {dag_id}: {completed}/{total} nodes already done"`

**Prevention:** 
- Enable `ExecutionResultRepository` with filesystem persistence
- Periodically save state after each node completion
- Use atomic write-rename for state persistence (`.tmp` → final)

### Scenario 2: State File Corruption

**Impact:** Execution state file is unreadable (partial write, disk error).

**Recovery:**
1. On load failure, treat the execution as failed with `ExecutionError::DeserialisationError`
2. Remove the corrupted state file to prevent re-triggering errors
3. Create an `ExecutionResult` with `cancelled: true` and appropriate reason
4. The orchestrator can detect this and re-plan the work
5. Alert: `"State corruption for execution {dag_id}: {error}. Work must be re-planned."`

**Prevention:**
- Atomic write-rename pattern: write to `.tmp` file, then `rename()` to final path
- Validate file integrity with checksum on read
- Back up state files at configurable intervals

### Scenario 3: DAG Engine Unavailable

**Impact:** `execute_graph()` called but DAG Engine's `DagGraphService` returns errors.

**Recovery:**
1. Retry: wait 1s, 2s, 4s (exponential), max 3 retries
2. After retries exhausted, return `ExecutionError::InternalError` with details
3. The orchestrator should check DAG Engine health before calling `execute_graph()`

**Prevention:**
- Health check coordination: `/api/v1/dag/health` before `/api/v1/execution/...`
- Circuit breaker: if DAG Engine returns 5 errors in 1 minute, back off

### Scenario 4: Cancellation Token Signalled

**Impact:** Execution cancelled mid-flight.

**Recovery:**
1. The `execute_graph()` returns an `ExecutionResult` with `cancelled: true`
2. The caller inspects `completed_count` and `failed_count`
3. If partial results are useful, process completed work
4. Otherwise, re-execute the full graph
5. In-flight nodes are allowed to complete gracefully before the cancelled result is returned

**Prevention:**
- Check `CancellationToken` before each node dispatch (not during execution)
- Allow in-flight nodes to complete cleanly

## Backup Strategy

### What to Back Up

| Data | Backup Frequency | Retention | Method |
|------|-----------------|-----------|--------|
| Execution results | After each completion | 7 days | File system snapshot |
| Execution state (in-flight) | After each node | Not persisted after execution completes | Ephemeral |
| Retry decisions | After each decision | 30 days | Audit log |

### Backup Schedule

- **Real-time:** Execution state saved after every node completion
- **Scheduled:** Full state backup every hour (cron)
- **On-demand:** Before module upgrade/deployment

### Backup Verification

- Restore test: Monthly restore from backup to verify integrity
- Checksum validation: Every read verifies file checksum

## Failover Plan

### Single Process (Current Architecture)

The execution-engine runs in-process with the orchestrator. Failover is automatic via process restart:

```
Process crash → systemd restart → scan incomplete executions → resume
```

### Future: Multi-Process / Distributed

When the system scales to multi-process execution:

1. **Active-Passive:** Secondary process takes over if primary crashes
2. **State sharing:** Execution state persisted to shared storage (S3/ETCD)
3. **Leader election:** Deterministic election based on execution ID

## Restore Procedure

### Full Restore

```bash
# 1. Stop the service
systemctl stop rigorix

# 2. Restore execution state from backup
cp -r /backup/execution-engine/$(date -d "1 hour ago" +%Y%m%d-%H)/* /var/lib/rigorix/executions/

# 3. Restore retry decision audit log
cp -r /backup/retry-decisions/$(date -d "1 day ago" +%Y%m%d)/* /var/lib/rigorix/retry-decisions/

# 4. Verify integrity
bash .pi/scripts/ci/check_execution-engine_contracts.sh

# 5. Start the service
systemctl start rigorix

# 6. Verify recovery
curl http://localhost:8080/api/v1/execution/health
```

### Partial Restore (Single Execution)

```bash
# Restore a single execution's state
cp /backup/execution-engine/$(date +%Y%m%d)/${dag_id}.json /var/lib/rigorix/executions/${dag_id}.json
```

## DR Testing Schedule

| Test | Frequency | Success Criteria |
|------|-----------|-----------------|
| Process crash recovery | Monthly | Incomplete execution resumes from last completed node |
| State corruption | Quarterly | System detects corruption and alerts without crashing |
| Backup restore | Quarterly | Restored state passes contract validation |
| Cancellation handling | Each release | Cancelled execution returns valid partial result |
