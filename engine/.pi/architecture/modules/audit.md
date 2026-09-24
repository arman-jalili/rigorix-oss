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
│   ├── envelope.rs              # AuditEnvelope, ExecutionEventRef, EventStatus, CircuitBreakerState, HistoryIntegrity, HistoryPolicy
│   ├── error.rs                 # AuditError (8 variants)
│   └── event/                   # AuditEvent payload schemas
├── application/                 # Service traits and implementations
│   ├── service.rs               # AuditService, AuditSender, AuditQueue, CircuitBreaker traits
│   ├── factory.rs               # AuditEnvelopeFactory, CircuitBreakerFactory traits
│   ├── dto/                     # Input/output DTOs for all operations
│   ├── audit_service_impl.rs    # AuditServiceImpl — orchestrator + chain link assignment
│   ├── chain.rs                 # ADR-016 local chain: next_link_from + verify_chain
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

status: implemented

depends: none

```rust
pub struct AuditEnvelope {
    pub execution_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub template_id: String,
    pub planning_hash: String,    // SHA-256 of planning prompt
    pub source: Option<String>,   // Source environment (e.g. "rigorix_cli", "rigorix_action")
    pub repository: Option<String>, // Repository name (e.g. "my-org/my-repo")
    pub author: Option<String>,   // Author identity (email or username) — legacy, self-asserted
    pub events: Vec<ExecutionEventRef>,
    pub signature: Option<String>, // HMAC-SHA256 signature for integrity

    // ── Approval & Identity evidence (additive, serde-defaulted) ──
    /// Signed approval decisions, in approval order (see approval module).
    pub approval_events: Vec<ApprovalRecordRef>,
    /// Post-execution scope violations (non-blocking evidence).
    pub scope_violations: Vec<ScopeViolationRef>,
    /// Reference + summary of decision context (full payload opt-in, stored locally).
    pub decision_context_ref: Option<String>,
    /// Attributed human identity of author/approver (see identity module).
    /// Redacted summary — never the raw token.
    pub identity: Option<IdentityRef>,
}
```

### Approval & Identity Evidence (Contract Amendment)

See [approval module](./modules/approval.md) and [identity module](./modules/identity.md) for the full contracts. Summary:

| Field | Type | Source | Notes |
|-------|------|--------|-------|
| `approval_events` | `Vec<ApprovalRecordRef>` | ApprovalService | One ref per human approval: step_name, node_id, intent_hash, approver_id, authority, decided_at, decision_context_ref |
| `scope_violations` | `Vec<ScopeViolationRef>` | ApprovalService (git-diff oracle) | Out-of-scope effects; non-blocking evidence |
| `decision_context_ref` | `Option<String>` | ApprovalService (R4) | Points to full decision context; summary embedded in the ref |
| `identity` | `Option<IdentityRef>` | Identity module | Subject, issuer, source, authority, expires_at — redacted, never the raw token |

All fields are additive with `#[serde(default, skip_serializing_if = ...)]` — backward compatible with existing envelopes (same pattern as `scoring_results`). When envelope signing is disabled, these remain operational evidence; docs/UI must never claim tamper-evidence for unsigned runs.

**Privacy:** `identity` and `decision_context` follow the `planning_prompt` opt-in pattern — full payloads stored locally, redacted summaries in the envelope (SpanPrivacy).

### Effect Key (Contract Amendment, ADR-014)

The envelope carries one additive field for effect-identity matching (sequence-policy R8):

| Field | Type | Source | Notes |
|-------|------|--------|-------|
| `effect_key` | `Option<String>` | Domain integration (supplied with the step) | Opaque canonical effect identity for the run/effect, recorded as a **one-way hash keyed by the domain secret** — never raw parameters |

`#[serde(default, skip_serializing_if = "Option::is_none")]` — an envelope without it is valid and never matches an effect-keyed history rule. Entity resolution lives **outside** rigorix (ADR-014 Layer 1); this module stores and compares the opaque key, it does not compute it. Effect-keyed rules have a retention dependency: pruning below the longest rule window silently disables them.

### Step Requirement Evidence (Contract Amendment, ADR-015)

The envelope carries one additive block for operator-controlled step requirements
(sequence-policy R9):

| Field | Type | Source | Notes |
|-------|------|--------|-------|
| `requirement_findings` | `Vec<RequirementFindingRef>` | SequencePolicyService | One ref per unmet requirement: `requirement_id`, `step`, `action`, `unmet` (pointer names / `identity`), `summary` |

`#[serde(default, skip_serializing_if = "Vec::is_empty")]` — additive and
backward compatible; an envelope without it is valid. Pointer **names** may be
recorded; parameter **values** stay redacted (SpanPrivacy). Requirement config is
operator-owned, so a finding is evidence of an operator policy decision, not of
plan data.

### Audit Integrity Chain (Contract Amendment, ADR-016 Phase A)

ADR-016 makes the local cache **tamper-evident and honest**. Additive,
serde-defaulted fields join the envelope and are covered by the HMAC
(canonical bytes) and checked by `application/chain.rs`:

| Field | Type | Meaning |
|-------|------|---------|
| `producer_id` | `Option<String>` | the chain this envelope belongs to (`RIGORIX_AUDIT_PRODUCER_ID`, default `local`) |
| `sequence` | `Option<u64>` | monotonic per-producer number (genesis `0`) |
| `prev_hash` | `Option<String>` | SHA-256 of the predecessor's **canonical bytes** (`signature` nulled) — the same form the HMAC signs, so chain and signature cannot disagree |
| `history_integrity` | `Option<HistoryIntegrity>` | `local_unanchored` (Phase A OSS default) or `anchored` (Phase C) |
| `history_policy` | `Option<HistoryPolicy>` | the recorded `allow_unanchored` opt-in, when the operator accepts best-effort local history |

`AuditEnvelopeFactoryImpl` tags every built envelope `local_unanchored` and
copies the chain link. `AuditServiceImpl::build_and_send` resolves
`sequence`/`prev_hash` from the local store **before signing** (under an
in-process lock), so the link is authenticated. `chain::verify_chain` detects
interior deletion/reorder/insertion and `prev_hash` mismatches;
`chain::next_link_from` computes the next link. Legacy envelopes (`producer_id`
/ `sequence` = `None`) are grandfathered — no chain claim, no behavior change.

**Verify-on-read.** The cross-run guard (`EnvelopeHistoryAdapter`) verifies each
envelope's HMAC (when the HMAC key is available) and the chain before trusting
any action; a failure fails closed. A deny-class cross-run rule that would fire
on `local_unanchored` history additionally **refuses** unless
`RIGORIX_HISTORY_POLICY=allow_unanchored` is set
(`SequencePolicyServiceImpl::with_unanchored_history_allowed`); that opt-in is
recorded in the envelope (`history_policy`). MCP `rigorix_read_audit` recomputes
the stored record's HMAC and rejects a mismatch. **Tail deletion is not
detectable locally** (ADR-016 boundary) — only the anchor (Phase C) closes it.

### AuditSender

**Purpose:** Deliver envelopes via HTTP with retry logic

**Implementation File:** `engine/src/audit/application/audit_sender_impl.rs`

status: implemented

depends: none

- Uses `reqwest` for HTTP POST delivery to configurable backend URL
- Exponential backoff with jitter (base * 2^attempt, capped, +25% jitter)
- Integration with `CircuitBreaker` for backpressure
- Configurable timeout per request
- Supports per-call backend URL override

### AuditQueue

**Purpose:** Queue for failed deliveries

**Implementation File:** `engine/src/audit/application/audit_queue_impl.rs`

status: implemented

depends: none

- Bounded in-memory FIFO queue (configurable capacity, default 100)
- Thread-safe via `tokio::sync::Mutex<VecDeque>`
- Enqueue returns `QueueFull` error at capacity
- Supports peek, clear, len, is_empty operations

### CircuitBreaker

**Purpose:** Circuit breaker for HTTP resilience

**Implementation File:** `engine/src/audit/application/circuit_breaker_impl.rs`

status: implemented

depends: none

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
| Envelope tampering | HMAC-SHA256 signature + ADR-016 verify-on-read; local per-producer chain detects interior deletion/reorder/insertion; deny-class cross-run rules refuse unanchored history by default | security-validator |
| Sensitive data in events | Event payload reviewed; no Secret values in events | security-validator |
| Circuit breaker bypass | Atomic counters prevent race conditions | operations-validator |
| Approval evidence forgery | `approval_events` bound to intent hashes; covered by envelope HMAC when signing on | security-validator |
| Identity leakage | `identity` block redacted; raw tokens never in the envelope (token_ref only) | security-validator |
| Decision context secrets | `planning_prompt` privacy pattern; full payload opt-in | security-validator |

---

## Runbook

See `docs/runbook-audit.md` for operational procedures.

## DR Plan

See `docs/dr-plan-audit.md` for disaster recovery procedures.

## CHANGELOG

See `.pi/architecture/CHANGELOG.md` for architecture change history.

---

Last updated: 2026-08-28
*Module version: 2.2.0*

---

**Status:** Implemented
**Last verified:** 2026-08-28 (approval & identity evidence — contract amendment)
**Module version:** 2.2.0
