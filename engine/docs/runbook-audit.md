# Audit Module Runbook

> **Last updated:** 2026-06-13
> **Module:** Audit (`engine/src/audit/`)
> **Components:** AuditEnvelope, AuditSender, AuditQueue, CircuitBreaker

## Startup Sequence

1. **Audit initialization** happens at process start:
   - `AuditEnvelopeFactoryImpl` created with optional HMAC signing key
   - `AuditSenderImpl` created with circuit breaker and backend URL
   - `AuditQueueImpl` created with bounded capacity (default: 100)
   - `AuditServiceImpl` orchestrates the components
2. **Configuration** from `AuditConfig` (in `Config`):
   - `enabled`: Whether audit is active
   - `backend_url`: Remote audit backend endpoint
   - `max_retries`: Delivery retry limit
   - `circuit_breaker_threshold`: Failures before circuit opens
3. **Circuit breaker** starts in Closed state

## Dependencies

| Dependency | Required | Source |
|-----------|----------|--------|
| `reqwest` | Yes | HTTP client for backend delivery |
| `tokio::fs` | Yes | Async file I/O for envelope persistence |
| `serde_json` | Yes | Envelope serialization/deserialization |
| `sha2` | Yes | Planning hash computation |
| `hmac` | Optional | Envelope signing (when key configured) |
| `chrono` | Yes | Timestamp management |
| `uuid` | Yes | Execution ID generation |

## Graceful Shutdown

1. **Stop accepting** new audit events
2. **Flush pending queue** — attempt to deliver all queued envelopes
3. **Log summary** — report delivered vs dropped count
4. **Close circuit breaker** — reset state for next startup

```bash
# Shutdown sequence (conceptual)
audit_service.stop_accepting();
audit_service.flush_pending().await;     # Retry all queued
let status = audit_service.status().await;
info!("Audit shutdown: {} delivered, {} dropped", delivered, dropped);
```

## Common Failure Modes

| Failure | Symptom | Recovery |
|---------|---------|----------|
| Backend unreachable | `AuditError::SendFailed` | Check network + backend URL in config |
| Circuit breaker open | `AuditError::CircuitBreakerOpen` | Wait for half-open probe or reset breaker |
| Queue full | `AuditError::QueueFull` | Increase capacity or clear pending queue |
| HMAC key missing | `AuditError::NotConfigured` | Set signing key in config |
| Signature mismatch | `AuditError::SignatureMismatch` | Check signing key match between sender/receiver |
| Envelope serialization error | `AuditError::SerializationFailed` | Check event payload validity |
| `history_integrity = anchored_pending` | The anchor is configured in anchored mode but no verified head exists yet | Normal during startup: the label turns `anchored` once a history read verifies a head. If it persists, the anchor is unreachable — check anchor connectivity; deny-class cross-run rules must treat `anchored_pending` as unanchored (it is not evidence). |
| `audit verify-projection` reports `provenance_established: false` | Expected. The command proves the projection matches the supplied history (`chain_consistent`) | Not an error. To establish provenance, verify the history against the anchor out of band — the bundle carries no anchor-signed checkpoint, so this check must not be read as authentication. |
| `audit verify-projection` fails | `facts_differ` / `anchor_differs` / chain error | The exporter asserted facts the bundled history does not support, or the history is not an intact chain. Treat the bundle as untrustworthy and re-export from the producer. |


### Release order after an integrity-vocabulary change

`history_integrity` is inside the signed bytes, so its values are a shared
contract (`rigorix-sdk` `schemas/envelope.json`). Adding a value is a rollout,
not an edit:

1. `rigorix-sdk` schema (values + description) — the contract.
2. this engine — start emitting.
3. enterprise ingestion — must accept it, or it **drops** every envelope that
   carries it (fail closed). These are the first records an anchored deployment
   writes, so the symptom is a gap at the head of the chain.
4. console.

Shipped example (#1071): the engine emitted `anchored_pending` before the schema
knew it, and enterprise ingestion rejected it before `rigorix-enterprise` #268
accepted it — consumer-first ordering was required for the release even though
the engine and schema were already on `main`. To check the emit side only:
`engine/tests/conformance/envelope.rs` validates all three variants against the
real SDK schemas.


## Operations

### Check audit status
```bash
curl http://localhost:8080/api/v1/audit/status
# Response: { "pending_count": 0, "circuit_breaker_state": "Closed", "backend_available": true }
```

### Retry failed deliveries
```bash
curl -X POST http://localhost:8080/api/v1/audit/retry
# Response: { "delivered": 5, "still_pending": 0, "dropped": 1 }
```

### Reset circuit breaker
```rust
// Programmatic reset
circuit_breaker.reset().await?;
```

## Configuration Reference

See `engine/src/configuration/domain/config.rs` for `AuditConfig` schema:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | `false` | Enable audit logging |
| `backend_url` | Option\<String\> | None | Remote audit backend URL |
| `max_retries` | u32 | 3 | Max delivery retry attempts |
| `circuit_breaker_threshold` | u32 | 5 | Failures before circuit opens |

## Key Environment Variables

| Variable | Purpose |
|----------|---------|
| `AUDIT_BACKEND_URL` | Audit backend endpoint |
| `AUDIT_SIGNING_KEY` | HMAC signing key for envelopes |
