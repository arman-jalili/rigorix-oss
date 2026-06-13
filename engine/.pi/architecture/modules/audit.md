# Audit Architecture

<!--
Canonical Reference: .pi/architecture/modules/audit.md
Blueprint Source: Domain Exploration Session 63c25384
-->

## Overview

Records execution audit trails via typed envelopes for governance, replay, and external audit backends. Provides the audit envelope format, sender with retry logic, queue management, and circuit breaker for resilient delivery to remote audit services.

## Responsibilities

- Produce typed AuditEnvelope for each execution record
- Send envelopes to remote audit backend with retry and circuit breaker
- Queue failed deliveries for later retry
- Support HMAC signing for envelope integrity
- Integrate with EventBus for event consumption

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| AuditEnvelope | `rigorix/src/audit/envelope.rs` | Typed envelope with execution metadata | #envelope |
| AuditSender | `rigorix/src/audit/sender.rs` | HTTP sender with retry logic | #sender |
| AuditQueue | `rigorix/src/audit/queue.rs` | Queue for failed deliveries | #queue |
| CircuitBreaker | `rigorix/rigorix-action/src/circuit_breaker.rs` | Circuit breaker for HTTP resilience | #breaker |

---

## Component Details

### AuditEnvelope

**Purpose:** Typed envelope containing execution audit data

**Implementation File:** `rigorix/src/audit/envelope.rs`

```rust
pub struct AuditEnvelope {
    pub execution_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub template_id: String,
    pub planning_hash: String,
    pub events: Vec<ExecutionEvent>,
    pub signature: Option<String>,  // HMAC signature for integrity
}
```

---

## Data Flow

```mermaid
flowchart TB
    EXEC["Execution completes"] --> DRAIN["EventBus::drain_persisted()
Vec<PersistedEvent>"]
    DRAIN --> BUILD["Build AuditEnvelope
{ execution_id, events,
planning_hash, signature }"]
    
    BUILD --> SEND["AuditSender::send(envelope)"]
    
    SEND -->|success| DELIVERED["Envelope delivered
to audit backend"]
    SEND -->|failure| QUEUE["AuditQueue::enqueue(envelope)
for retry"]
    
    QUEUE --> CB{"CircuitBreaker
state?"]
    CB -->|Closed| RETRY["Retry send"]
    RETRY --> SEND
    CB -->|Open| WAIT["Wait timeout
(probe)"]
    WAIT --> CB
    CB -->|HalfOpen| TEST["Test single send"]
    TEST --> SEND
```

**Flow Description:**
1. On execution completion, EventBus drains all persisted events
2. AuditEnvelope is built with execution metadata, events, and HMAC signature
3. AuditSender delivers envelope to remote audit backend with retry logic
4. CircuitBreaker guards against backend failures with open/half-open/closed states
```

---

## Dependencies

### Depends On
- **Event System**: Consumes ExecutionEvent stream
- **Configuration**: AuditConfig (backend_url, api_key, max_retries)

### Used By
- **Orchestrator**: Builds and sends audit envelope after execution
- **rigorix-action**: GitHub Action governance integration

---

## Security Considerations

| Concern | Mitigation | Validator |
|---------|------------|-----------|
| Envelope tampering | HMAC signature field for integrity verification | security-validator |
| Sensitive data in events | Event payload reviewed; no Secret values in events | security-validator |

---

*Last updated: 2026-06-13*
*Module version: 1.0.0*
