# Disaster Recovery Plan: Audit Posting

## RTO/RPO Targets

| Metric | Target | Notes |
|--------|--------|-------|
| RTO (Recovery Time) | < 5 minutes | Service restart + queue recovery |
| RPO (Recovery Point) | < 1 minute | In-memory queue loss window |

## Backup Strategy

### Filesystem Backend (OSS Default)
- **Method**: Atomic write-rename for each record.
- **Location**: Configurable `filesystem_path` (default `.rigorix/audit-records`).
- **Schedule**: No explicit backup needed — records are persisted instantly.
- **Retention**: Configurable via `prune()` method (age-based cleanup).

### HTTP Backend (Enterprise)
- **Method**: Backend is responsible for its own durability.
- **Fallback**: If HTTP backend is unavailable, records are queued in-memory.
  On process restart, queued records are lost (in-memory queue).

## Restore Procedure

### From Filesystem
1. Ensure storage directory exists and is writable.
2. Instantiate `FilesystemAuditBackendImpl` with the storage path.
3. Call `load()` to retrieve individual records by execution ID.
4. Call `list()` to enumerate all stored records.

### From Queue (after crash)
1. In-memory queue is lost on crash — this is by design.
2. The `recover()` method on `AuditRecordQueue` is a no-op for in-memory
   (returns 0). Filesystem-backed queue recovery is planned.

## Failover Plan

### HTTP → Filesystem Fallback
If the HTTP backend is unavailable:
1. `AuditPostingService` catches `BackendUnavailable` / `PostFailed`.
2. Record is enqueued to `AuditRecordQueueImpl` for later retry.
3. `retry_pending()` can be called manually or on a timer to drain the queue.

### Retry Logic
- Exponential backoff: `base * 2^(attempt-1)` seconds, capped at 60s.
- Default max retries: 3 (configurable).
- Records that exhaust retries are dropped with a warning log.

## Testing

| Test | What It Verifies |
|------|-----------------|
| `test_post_and_load` | FS write-then-read round-trip |
| `test_delete` | Record removal |
| `test_prune` | Age-based cleanup |
| `test_queue_full_error` | Queue capacity enforcement |
| `test_sign_and_verify` | HMAC integrity verification |
| `test_verify_tampered_record` | Tamper detection |

## Recovery Scenarios

### Scenario A: Process crash during post
1. Filesystem write is atomic (write-then-rename) — no partial files.
2. HTTP post either completes or fails — no partial state.
3. `AuditRecordQueue` is in-memory — queued items are lost.
4. Run `retry_pending()` if there's a persistent queue.

### Scenario B: HMAC signing key rotated
1. Old records remain verifiable with the old key.
2. New records use the new key.
3. If old key is discarded, old records cannot be verified.
4. Store key history for audit verification.

### Scenario C: Disk full
1. `FilesystemAuditBackendImpl::post()` returns `FilesystemError`.
2. Record is queued for retry.
3. Free disk space and re-run `retry_pending()`.
