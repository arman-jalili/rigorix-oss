# Capacity Ledger Architecture

<!--
Canonical Reference: .pi/architecture/modules/capacity-ledger.md
Blueprint Source: Guardian Framework v1.2
Generated: NEVER (this is the source)
-->

## Overview

Owns the **model and the port** for contested, limited capacity shared across independent
executions: what a resource is, what a reservation is, what a lease is, what aggregate exposure
is, and which atomic operations the engine may perform against them.

It explicitly does **not** own storage. The engine has no database dependency and will not
acquire one; storage is an adapter. See ADR-018.

**Guarantee ceiling:** the in-process adapter provides **single-execution atomicity**; only the
HTTP adapter against a real contested store provides cross-session atomicity. No deployment may
claim the second while running the first.

## DDD Layers

| Layer | Purpose | Tech |
|-------|---------|------|
| `domain/` | Pure types, invariants, errors | Zero framework imports |
| `application/` | Use cases over the port, RAII guard | Rust, async |
| `infrastructure/` | Adapters: in-process, HTTP | std sync, reqwest |
| `interfaces/` | The `CapacityLedger` port | Rust traits |

**Dependency rule:** `domain ← application ← infrastructure` (inward); `interfaces/` is the
contract consumed by `enforcement`.

## Components by Layer

#### Domain Layer (`domain/`)
| Component | Description | Framework? |
|-----------|-------------|------------|
| Resource | `ResourceId` + `ResourceSpec` (name, hard_limit, window) | ❌ No |
| Reservation | Reservation aggregate + `ReservationState` (outstanding/committed/released/expired) | ❌ No |
| Grant | Opaque handle returned by reserve; binds resource, amount, lease, idempotency key | ❌ No |
| Exposure | Aggregate view: committed vs outstanding kept distinct | ❌ No |
| IdempotencyKey | Retry identity; makes double-reserve impossible by construction | ❌ No |
| LedgerError | Typed: `Insufficient \| Contended \| Unavailable \| LeaseExpired` | ❌ No |

#### Application Layer (`application/`)
| Component | Description | Type |
|-----------|-------------|------|
| CapacityLedgerService | reserve / commit / release / exposure use cases over the port | Service |
| ReservationGuard | RAII: commit on success, release on drop | Guard |

#### Infrastructure Layer (`infrastructure/`)
| Component | Description | Connects to |
|-----------|-------------|-------------|
| InProcessLedger | Default adapter; single-execution atomicity; no network, no DB | In-memory state |
| HttpLedger | Opt-in adapter; bounded body, no redirects, deadline, redaction | Enterprise ledger service |

#### Interfaces Layer (`interfaces/`)
| Component | Description | 'use client'? |
|-----------|-------------|---------------|
| CapacityLedger | The port: reserve / commit / release / exposure + invariants I1–I6 | No |

---

## Component Details

### Resource

status: implemented
depends: none

**Purpose:** A named, limited capacity. `ResourceId` is the identity; `ResourceSpec` carries
`hard_limit` and the window over which exposure is aggregated. The engine attaches no domain
meaning to a resource name — it is an operator-declared label.

**Layer:** `domain/`

### Reservation

status: implemented
depends: Resource

**Purpose:** The aggregate. A held claim on capacity with a lease and exactly one terminal
transition (committed, released, or expired). State transitions are enforced in the type, not
by convention.

**Layer:** `domain/`

### Grant

status: implemented
depends: Reservation

**Purpose:** The opaque handle returned by `reserve`. Must not be constructible by a caller —
otherwise a forged grant becomes a way to skip reservation.

**Layer:** `domain/`

### Exposure

status: implemented
depends: Reservation

**Purpose:** The aggregate view. **Committed and outstanding are separate fields, never summed
into one number**: "what is actually spent" and "what is in flight" are different operational
questions, and collapsing them destroys the ability to answer either.

**Layer:** `domain/`

### IdempotencyKey

status: implemented
depends: none

**Purpose:** Retry identity. Part of the reserve request so that a retried reservation cannot
create a second one *by construction*. In a distributed system, network retries otherwise
become the over-spend.

**Layer:** `domain/`

### LedgerError

status: implemented
depends: none

**Purpose:** Typed failures. `Insufficient` (no room) and `Unavailable` (control plane
unreachable) must remain distinguishable — an outage and a full cap are different operational
events, and the operator-facing surfaces depend on the distinction.

**Layer:** `domain/`

### CapacityLedgerService

status: implemented
depends: CapacityLedger, Reservation, Grant, Exposure

**Purpose:** The use cases over the port: reserve, commit, release, exposure. Contains no
storage logic and no local counter — a local counter is precisely the naive-counter failure
this module exists to replace.

**Layer:** `application/`

### ReservationGuard

status: implemented
depends: CapacityLedgerService

**Purpose:** RAII binding of a grant to a step's lifetime: commit on success, **release on
drop**. Release-on-drop is not optional — if release happens only on an explicit error branch,
a panic or early return leaks capacity until the lease expires.

**Layer:** `application/`

### InProcessLedger

status: implemented
depends: CapacityLedger

**Purpose:** The standalone default. Single-execution atomicity via the proven ADR-008
discipline (shared state + RAII). No database, no network. Its guarantee is stated in code,
because the temptation to describe it as "the distributed ledger" is high and wrong.

**Layer:** `infrastructure/`

### HttpLedger

status: implemented
depends: CapacityLedger

**Purpose:** Opt-in distributed mode. Reaches the enterprise ledger service over HTTP with the
same transport discipline as the precondition provider client (bounded body, no redirect
amplification, explicit deadline, credentials never logged). Maps every ambiguity to
`Unavailable` so the caller fails closed.

**Layer:** `infrastructure/`

### CapacityLedger

status: implemented
depends: none

**Purpose:** The port. The invariants (I1 atomic/linearizable per resource+window, I2 idempotent,
I3 leased, I4 fail-closed, I5 store clock, I6 no claim inflation) are the specification; the
signatures are incidental. One contract suite runs against every adapter.

**Layer:** `interfaces/`

---

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Contract suite passes against **every** adapter | `validate-tests.sh` |
| 2 | N concurrent reservations never exceed `hard_limit` | `validate-tests.sh` |
| 3 | Retried reserve with same key yields one reservation | `validate-tests.sh` |
| 4 | Unreachable ledger refuses (fail-closed) | `validate-integration.sh` |
| 5 | Layer boundaries respected; no storage types in `domain/` | `validate-architecture.sh` |
| 6 | Wiring proven by a reachability test that fails when the call is removed | `validate-integration.sh` |

## Dependencies

### Depends On

- ADR-018 (design), ADR-008 (RAII pattern), `enforcement` (consumer seam)

### Used By

- `enforcement` (the decision path), and optionally `budget_tracking` (staged)

## Related ADRs

- ADR-018: distributed capacity ledger
- ADR-008: RAII reservation pattern
- ADR-016: the **audit** ledger — a different thing (append-only proof, not leased accounting)
- Enterprise counterpart: ADR-010 `capacity-ledger-service`

---

## Implementation Status

All components are implemented under `src/capacity_ledger/` and frozen by the
contract-freeze issue. Status is tracked per epic issue.

| Component | Implementation | Tests |
|-----------|----------------|-------|
| Resource | `domain/resource.rs` — identity, window, saturating capacity arithmetic | unit |
| Reservation | `domain/reservation.rs` — one terminal transition, lease-gated expiry | unit |
| Grant | `domain/grant.rs` — opaque, crate-internal issuance only | in-crate |
| Exposure | `domain/exposure.rs` — committed/outstanding kept distinct | unit |
| IdempotencyKey | `domain/idempotency.rs` — deterministic `for_step` | unit |
| LedgerError | `domain/error.rs` — typed failures + classification | unit |
| CapacityLedger | `interfaces/capacity_ledger.rs` — port + `interfaces/contract.rs` suite | contract suite |
| CapacityLedgerService | `application/service_impl.rs` | 4 tests |
| ReservationGuard | `application/guard_impl.rs` — release-on-drop (ADR-008) | 3 tests |
| InProcessLedger | `infrastructure/in_process.rs` — single-execution atomicity | 7 tests incl. 50-way concurrency |
| HttpLedger | `infrastructure/http.rs` — fail-closed transport | WireMock suite |

### Operability

- **Runbook:** `docs/runbook-capacity-ledger.md`
- **DR plan:** `docs/dr-plan-capacity-ledger.md`
- **Proofing:** `.pi/scripts/ci/check_capacity-ledger_contracts.sh` +
  `stage_capacity-ledger_proofing.sh` (hardening Stage 11)
- **Observability:** `tracing` on declare/reserve/resolve; `reservation_id` and
  `resource` are the correlation keys

### Guarantee ceiling

The in-process adapter provides **single-execution atomicity only**; only the
HTTP adapter against a real contested store provides cross-session atomicity.
