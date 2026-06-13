# Audit Module DR Plan

> **Last updated:** 2026-06-13
> **Module:** Audit (`engine/src/audit/`)
> **RTO:** < 5 minutes (restart with queue drain)
> **RPO:** < 1 hour (pending envelopes in queue)

## System State

The audit module maintains **ephemeral state** in the delivery queue and
circuit breaker. Persisted envelopes are stored on the local filesystem.

| State Type | Storage | Persistence | Recovery |
|-----------|---------|------------|----------|
| Pending queue | In-memory (`VecDeque`) | Lost on restart | Envelopes to be recreated by orchestrator |
| Circuit breaker | In-memory (`AtomicU32` + `RwLock`) | Lost on restart | Resets to Closed |
| Persisted envelopes | Filesystem (`{base_path}/*.json`) | Persistent | Can be replayed |
| HMAC signing key | Config (`String`) | Loaded at startup | Reconfigured from env/config |

## Backup Strategy

| Asset | Frequency | Method | Retention |
|-------|-----------|--------|-----------|
| Persisted envelopes | Per-execution | JSON files in configurable base path | Configurable via `prune()` |
| Audit config | Per-deployment | Part of `rigorix.toml` (git-tracked) | Git history |
| HMAC signing key | Per-deployment | Environment variable | Secrets manager |

## Restore Procedure

### Scenario 1: Process restart (normal)
1. **Start the process** — audit module initializes with fresh state
2. **Circuit breaker** resets to Closed automatically
3. **Queue** starts empty — orchestrator re-creates envelopes on next execution

### Scenario 2: Data loss — recover persisted envelopes
```bash
# 1. Check persisted envelope directory
ls -la /var/lib/rigorix/audit/

# 2. Replay specific envelopes
for file in /var/lib/rigorix/audit/*.json; do
  execution_id=$(basename "$file" .json)
  echo "Replaying envelope: $execution_id"
  # Submit to audit backend
  curl -X POST "$AUDIT_BACKEND_URL" \
    -H "Content-Type: application/json" \
    -d @"$file"
done
```

### Scenario 3: Backend unavailable — queue drain
1. **Check circuit breaker state** via `/api/v1/audit/status`
2. **If Open**: Wait for half-open probe or reset with `circuit_breaker.reset()`
3. **Flush queue**: `POST /api/v1/audit/retry`
4. **Monitor** pending count decreasing

## Failover Plan

### Single-instance recovery
1. Circuit breaker automatically transitions to half-open after timeout
2. Retry queue persists failed envelopes with exponential backoff
3. Envelopes are dropped only after exhausting max retries

### Multi-instance deployments
1. Each instance has its own in-memory queue (no shared state)
2. Persisted envelopes on shared filesystem can be replayed by any instance
3. Backend should be load-balanced for high availability
4. Circuit breaker is per-instance — no coordination needed

## Monitoring

### Key metrics
- **Queue depth**: Number of pending envelopes
- **Circuit breaker state**: Closed / Open / HalfOpen
- **Delivery rate**: Envelopes delivered per minute
- **Failure rate**: Envelopes dropped vs delivered

### Health check
```
GET /api/v1/audit/status
{
  "pending_count": 0,
  "circuit_breaker_state": "Closed",
  "backend_available": true
}
```

## RTO/RPO

| Metric | Target | Notes |
|--------|--------|-------|
| RTO | < 5 minutes | Time to restart process + drain pending queue |
| RPO | < 1 hour | Envelopes in queue at time of failure (in-memory, lost on restart) |
| RPO (persisted) | 0 | Filesystem envelopes survive restart |
