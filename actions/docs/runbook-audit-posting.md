# Runbook: Audit Posting Module

## Overview

The audit-posting module posts HMAC-signed audit records from GitHub Actions to a
configurable backend (HTTP or local filesystem). It supports retry with exponential
backoff, bounded in-memory queuing, and HMAC-SHA256 integrity signing.

## Component Map

```
AuditPostingService
  ├── AuditRecordFactory    → create + sign records
  ├── AuditBackend          → post to storage (HTTP or filesystem)
  └── AuditRecordQueue      → buffer failed deliveries
```

## Startup Sequence

1. **Configuration Load**: The `AuditBackendFactory` reads configuration and creates
   the appropriate backend (`HttpAuditBackend` or `FilesystemAuditBackendImpl`).
2. **Factory Initialization**: `AuditRecordFactoryImpl` loads the HMAC signing key
   (hex-encoded) and key identifier.
3. **Queue Initialization**: `AuditRecordQueueImpl` creates an empty in-memory queue.
4. **Service Assembly**: `AuditPostingServiceImpl` wires all components together.

## Graceful Shutdown

1. Process remaining queue items via `retry_pending()`.
2. Allow in-flight HTTP requests to complete (default timeout: 30s).
3. No persistent state to flush (filesystem writes are atomic).

## Configuration Reference

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `backend_url` | `Option<String>` | `None` | HTTP backend URL (HTTP mode) |
| `filesystem_path` | `Option<String>` | `.rigorix/audit-records` | Storage path (filesystem mode) |
| `signing_key` | `Option<String>` | `None` | Hex-encoded HMAC-SHA256 key |
| `key_id` | `Option<String>` | `"default"` | Key identifier |
| `max_retries` | `u32` | `3` | Max retry attempts |
| `retry_delay_secs` | `u64` | `1` | Base retry delay |
| `queue_capacity` | `u32` | `100` | Max queued records |

## Common Failure Modes

### Backend Unavailable
- **Symptom**: `AuditPostingError::BackendUnavailable`
- **Action**: Record is queued for retry. If transient, retry succeeds.
- **Recovery**: Verify backend URL is reachable. Check network/firewall.

### HMAC Key Not Configured
- **Symptom**: `AuditPostingError::KeyNotAvailable`
- **Action**: Record is created without signature.
- **Recovery**: Set `signing_key` environment variable.

### Queue Full
- **Symptom**: `AuditPostingError::QueueFull`
- **Action**: New failures are dropped.
- **Recovery**: Increase `queue_capacity` or process pending via `retry_pending()`.

### Signature Mismatch
- **Symptom**: `AuditPostingError::SignatureMismatch`
- **Action**: Record is rejected.
- **Recovery**: Verify signing keys match between creator and verifier.

## Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `audit_posts_total` | Counter | Total post attempts |
| `audit_posts_success` | Counter | Successful posts |
| `audit_posts_failed` | Counter | Failed posts |
| `audit_queue_depth` | Gauge | Current queue size |
| `audit_sign_errors` | Counter | HMAC signing errors |

## Logging

All operations use structured tracing via the `tracing` crate:
```
2026-06-21T12:00:00Z INFO audit_posting::poster{execution_id="abc-123"}:
  "Record posted to backend" success=true duration_ms=42
2026-06-21T12:00:01Z WARN audit_posting::poster{execution_id="abc-123"}:
  "Failed to post record, queuing for retry" error="BackendUnavailable"
```
