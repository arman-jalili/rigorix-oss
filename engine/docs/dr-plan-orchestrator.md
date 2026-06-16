# Orchestrator Disaster Recovery Plan

> **Last updated:** 2026-06-16
> **Module:** Orchestrator (`engine/src/orchestrator/`)
> **RTO Target:** 5 minutes
> **RPO Target:** 0 (no data loss for completed executions)

## Backup Strategy

### Execution Records

| Data | Backup Method | Frequency | Retention |
|------|--------------|-----------|-----------|
| `ExecutionRecord` | Written atomically by `StateManagerService` | Per execution | 30 days |
| Execution snapshots | `ExecutionState` persisted via `filesystem_state_repository` | Per state transition | 7 days |
| Event logs | `EventBus` drain produces `ExecutionEvent` list in record | Per execution | Included in record |

### Backup Locations

- **Primary:** `{repo_root}/.rigorix/executions/` — per-execution JSON files
- **Format:** JSON (human-readable, compressible)
- **Integrity:** SHA-256 content hash included in `PlanningMetadata.prompt_hash`

## Restore Procedure

### Restoring an ExecutionRecord

1. Locate the JSON file: `{repo_root}/.rigorix/executions/{execution_id}.json`
2. Verify integrity using the included prompt_hash
3. Deserialize using `serde_json::from_str::<ExecutionRecord>()`
4. For audit replay, reconstruct events from record

### Replaying an Execution

1. Load the original `ExecutionRecord`
2. Extract planning metadata (template_id, confidence, prompt_hash)
3. Rerun `PlanningPipeline::plan_with_graph()` with the same intent
4. Compare prompt_hash to verify reproducibility

## Failover Plan

### Orchestrator Service Failure

1. **Detect:** Missing heartbeat or timeout on `run()` call
2. **Isolate:** Cancel any in-flight executions via `cancel()`
3. **Recover:** Restart the service — state is persisted on disk
4. **Verify:** Run a test execution to confirm service health

### Sub-Service Failure

| Failed Service | Impact | Mitigation |
|---------------|--------|-----------|
| PlanningPipeline | Cannot plan new executions | Isolate, check LLM provider health |
| ExecutionEngine | Cannot execute DAG | Check DAG state, retry with cancellation |
| StateManager | Cannot persist state | Check disk space, file permissions |
| CancellationService | Cannot cancel executions | Manual shutdown of running tasks |
| EventBus | Events not emitted | Drain on restart, events may be lost |
| AuditService | Envelopes not delivered | Non-fatal — record still returned |

## RTO/RPO

| Metric | Target | Measurement |
|--------|--------|------------|
| RTO (Recovery Time Objective) | 5 minutes | Time from failure detection to service restored |
| RPO (Recovery Point Objective) | 0 | No execution data loss for completed runs |
| MTTR (Mean Time to Recover) | 2 minutes | Expected restart time |

## Testing

### Recovery Tests

1. **Kill and restart:** Start an execution, kill the process, restart — verify state is recoverable
2. **Sub-service failure:** Mock each sub-service to fail, verify orchestrator returns appropriate error
3. **Cancellation:** Start execution, cancel it, verify Cancelled state is persisted
4. **Audit failure:** Configure invalid audit backend, verify execution completes with warning

### Regular Checks

- Run hardening stages: `bash engine/.pi/scripts/ci/run_hardening_stages.sh`
- Run orchestrator tests: `cargo test --lib -- orchestrator`
- Verify contracts: `bash engine/.pi/scripts/ci/check_orchestrator_contracts.sh`
