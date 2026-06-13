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

| Component | Implementation | File Path | Purpose |
|-----------|---------------|-----------|---------|
| AuditEnvelope | AuditEnvelope struct | `domain/envelope.rs` | Typed envelope with execution metadata |
| AuditError | AuditError enum | `domain/error.rs` | Domain error types (7 variants) |
| AuditEvent | AuditEvent enum | `domain/event/mod.rs` | Event payload schemas (5 event types) |
| AuditService | AuditServiceImpl | `application/audit_service_impl.rs` | Orchestrates build-and-send flow |
| AuditSender | AuditSenderImpl | `application/audit_sender_impl.rs` | HTTP sender with retry + exponential backoff |
| AuditQueue | AuditQueueImpl | `application/audit_queue_impl.rs` | Bounded in-memory FIFO queue |
| CircuitBreaker | CircuitBreakerImpl | `application/circuit_breaker_impl.rs` | Closed/Open/HalfOpen state machine |
| AuditEnvelopeFactory | AuditEnvelopeFactoryImpl | `application/envelope_factory_impl.rs` | Envelope construction + HMAC signing |
| CircuitBreakerFactory | CircuitBreakerFactoryImpl | `application/circuit_breaker_factory_impl.rs` | Breaker instance creation |
| AuditEnvelopeRepository | LocalAuditEnvelopeRepository | `infrastructure/local_audit_repository.rs` | Filesystem envelope persistence |
| HTTP API | — | `interfaces/http/mod.rs` | REST endpoints + error format |

## Architecture

```text
audit/
├── domain/                      # Domain entities and interfaces (frozen contracts)
│   ├── mod.rs
│   ├── envelope.rs              # AuditEnvelope, ExecutionEventRef, EventStatus, CircuitBreakerState
│   ├── error.rs                 # AuditError (7 variants)
│   └── event/                   # AuditEvent payload schemas
├── application/                 # Service traits and implementations
│   ├── service.rs               # AuditService, AuditSender, AuditQueue, CircuitBreaker traits
│   ├── factory.rs               # AuditEnvelopeFactory, CircuitBreakerFactory traits
│   ├── dto/                     # Input/output DTOs for all operations
│   ├── audit_service_impl.rs    # AuditServiceImpl — orchestrator
│   ├── audit_sender_impl.rs     # AuditSenderImpl — HTTP delivery with reqwest
│   ├── audit_queue_impl.rs      # AuditQueueImpl — bounded VecDeque
│   ├── circuit_breaker_impl.rs  # CircuitBreakerImpl — atomic state machine
│   ├── envelope_factory_impl.rs # AuditEnvelopeFactoryImpl — SHA-256 + HMAC
│   └── circuit_breaker_factory_impl.rs
├── infrastructure/              # Repository interfaces and implementations
│   ├── repository/              # AuditEnvelopeRepository trait
│   └── local_audit_repository.rs # LocalAuditEnvelopeRepository — JSON files + atomic write
└── interfaces/                  # API contracts
    └── http/                    # REST endpoint contracts
```

---

## Component Details

### AuditEnvelope

**Purpose:** Typed envelope containing execution audit data

**Implementation File:** `engine/src/audit/domain/envelope.rs`

```rust
pub struct AuditEnvelope {
    pub execution_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub template_id: String,
    pub planning_hash: String,    // SHA-256 of planning prompt
    pub events: Vec<ExecutionEventRef>,
    pub signature: Option<String>, // HMAC-SHA256 signature for integrity
}
```

### AuditSender

**Purpose:** Deliver envelopes via HTTP with retry logic

**Implementation File:** `engine/src/audit/application/audit_sender_impl.rs`

- Uses `reqwest` for HTTP POST delivery to configurable backend URL
- Exponential backoff with jitter (base * 2^attempt, capped, +25% jitter)
- Integration with `CircuitBreaker` for backpressure
- Configurable timeout per request
- Supports per-call backend URL override

### AuditQueue

**Purpose:** Queue for failed deliveries

**Implementation File:** `engine/src/audit/application/audit_queue_impl.rs`

- Bounded in-memory FIFO queue (configurable capacity, default 100)
- Thread-safe via `tokio::sync::Mutex<VecDeque>`
- Enqueue returns `QueueFull` error at capacity
- Supports peek, clear, len, is_empty operations

### CircuitBreaker

**Purpose:** Circuit breaker for HTTP resilience

**Implementation File:** `engine/src/audit/application/circuit_breaker_impl.rs`

- State machine: Closed → Open → HalfOpen → Closed
- Configurable failure threshold and half-open timeout
- Atomic counters for thread safety
- Stats tracking (total requests, failures, consecutive failures)
- Reset capability for manual recovery

## Data Flow

```mermaid
flowchart TB
    EXEC["Execution completes"] --> DRAIN["EventBus::drain_persisted()
Vec<PersistedEvent>"]
    DRAIN --> BUILD["AuditEnvelopeFactory::build_envelope()
SHA-256 hash + optional HMAC"]
    
    BUILD --> SEND["AuditSender::send(envelope)
HTTP POST to backend"]
    
    SEND -->|success| DELIVERED["Envelope delivered
to audit backend"]
    SEND -->|failure| QUEUE["AuditQueue::enqueue(envelope)
for retry"]
    
    QUEUE --> CB{"CircuitBreaker
state?"]
    CB -->|Closed| RETRY["Retry with backoff
base * 2^attempt + jitter"]
    RETRY --> SEND
    CB -->|Open| WAIT["Wait half-open timeout
(configurable, default 60s)"]
    WAIT --> CB
    CB -->|HalfOpen| TEST["Test single send"]
    TEST --> SEND
    SEND -->|success| CLOSE["Circuit → Closed"]
    SEND -->|failure| REOPEN["Circuit → Open"]
```

**Flow Description:**
1. On execution completion, EventBus drains all persisted events
2. AuditEnvelopeFactory builds envelope with SHA-256 planning hash and optional HMAC signature
3. AuditSender delivers envelope via HTTP POST with retry logic
4. CircuitBreaker guards against backend failures with closed/open/half-open states
5. On failure, envelope is enqueued to AuditQueue for later retry with exponential backoff + jitter

---

## Dependencies

### Depends On
- **Event System**: Consumes ExecutionEvent stream (planned)
- **Configuration**: AuditConfig (backend_url, api_key, max_retries)

### Used By
- **Orchestrator**: Builds and sends audit envelope after execution
- **rigorix-action**: GitHub Action governance integration

---

## Testing

- **34 unit tests** across all components
- **Coverage**: All public methods tested including edge cases
- **Key test scenarios**:
  - Envelope building with/without HMAC signing
  - Signature verification (valid + tampered)
  - Planning hash consistency
  - Circuit breaker state transitions (close → open, open → half-open)
  - Queue full rejection
  - Send with no backend configured
  - Filesystem persistence (save, find, delete, list, count, prune)
  - Backoff delay increasing per attempt
  - Backoff delay capped at maximum

## CI Integration

- **Stage 12** in hardening pipeline: `stage_audit_proofing.sh`
- `check_audit_contracts.sh`: Validates all 11 contract interfaces have implementations
- `check_audit_coverage.sh`: Enforces minimum 80% coverage (fallback: 15+ tests)

## Security Considerations

| Concern | Mitigation | Validator |
|---------|------------|-----------|
| Envelope tampering | HMAC-SHA256 signature field for integrity verification | security-validator |
| Sensitive data in events | Event payload reviewed; no Secret values in events | security-validator |
| Circuit breaker bypass | Atomic counters prevent race conditions | operations-validator |

---

## Runbook

See `docs/runbook-audit.md` for operational procedures.

## DR Plan

See `docs/dr-plan-audit.md` for disaster recovery procedures.

## CHANGELOG

See `.pi/architecture/CHANGELOG.md` for architecture change history.

---

*Last updated: 2026-06-13*
*Module version: 2.0.0*
