---
guardian_issue:
  id: "ISSUE-CAPACITY-LEDGER-DOMAIN"
  title: "capacity_ledger: domain model and the atomic port contract"
  epic: "distributed capacity ledger"
  epic_id: "EPIC-CAPACITY-LEDGER"
  component: "CapacityLedger"
  module: "capacity-ledger"
  status: planned
  priority: high
  dependencies:
    - "ADR-018 (accepted design)"
    - "ADR-008 (RAII reservation pattern, reference implementation in budget_tracking)"

  intent: |
    Create the capacity_ledger bounded context's domain model and the port that expresses the
    atomic operations. No storage, no adapters, no wiring — this issue lands the types and the
    contract, with the invariants encoded as tests that every adapter must later satisfy.

  in_scope:
    - "New: ResourceId, ResourceSpec (name, hard_limit, window)"
    - "New: Reservation aggregate + ReservationState (outstanding/committed/released/expired)"
    - "New: Grant (opaque handle returned by reserve), Lease (expires_at)"
    - "New: Exposure (aggregate view: committed + outstanding)"
    - "New: LedgerError taxonomy incl. Insufficient | Contended | Unavailable | LeaseExpired"
    - "New: trait CapacityLedger — reserve/commit/release/exposure"
    - "New: IdempotencyKey value type"
    - "Contract tests expressing invariants I1–I6 (adapter-agnostic)"

  out_of_scope:
    - "Any adapter implementation (in-process is ISSUE-CAPACITY-LEDGER-INPROC)"
    - "Any storage or network dependency"
    - "Wiring into enforcement (ISSUE-CAPACITY-LEDGER-SEAM)"
    - "Configuration surface / TOML parsing"

  affected_layers:
    domain:
      - "New: ResourceId, ResourceSpec, Reservation, ReservationState, Grant, Lease, Exposure"
      - "New: LedgerError taxonomy"
      - "New: IdempotencyKey"
    application:
      - "New: reserve / commit / release / exposure use-case signatures over the port"
    infrastructure:
      - "None — deliberately"
    api:
      - "New: trait CapacityLedger (interfaces/)"

  canonical_references:
    - module: ".pi/architecture/modules/capacity-ledger.md"
    - adr:
        - ".pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
    - last_sync: "2026-10-14"

  acceptance_criteria:
    - "validate-architecture.sh passes (no infrastructure or network types in domain/ or interfaces/)"
    - "Domain invariants covered by unit tests (lease arithmetic, exposure summation, legal state transitions)"
    - "Contract test suite exists and is adapter-agnostic (parameterised over the port)"
    - "validate-canonical.sh passes (module + ADR references resolve)"
    - "No new panics: fallible operations return Result (panic ratchet respected)"
    - "IdempotencyKey is part of the ReserveRequest — retries cannot double-reserve by construction"

  validators:
    - ci
    - tests
    - architecture
    - canonical

  implementation_notes: |
    - The invariants ARE the specification. Encode I1–I6 as contract tests now, so adapters are
      falsifiable later.
    - Grant must be an opaque handle bound to (resource, amount, lease, idempotency_key) — never
      a caller-constructed struct that can be forged.
    - Exposure must distinguish committed from outstanding; conflating them loses the ability to
      answer "what is actually spent?" vs "what is in flight?".
    - Do NOT add a local counter as an optimisation. That is the naive-counter failure.
    - Keep domain free of tokio/reqwest types.

  file_changes:
    - "create: engine/src/capacity_ledger/mod.rs"
    - "create: engine/src/capacity_ledger/domain/{resource,reservation,grant,lease,exposure,error,idempotency}.rs"
    - "create: engine/src/capacity_ledger/interfaces/port.rs"
    - "create: engine/src/capacity_ledger/application/{reserve,commit,release,exposure}.rs"
    - "create: engine/tests/contract/capacity_ledger_contract.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-CAPACITY-LEDGER-DOMAIN — domain model and the atomic port contract

## Intent

Land the types and the contract, and nothing else. The port's invariants (ADR-018 I1–I6) are
the real deliverable; the signatures are incidental. Adapters come next and must be
falsifiable against this contract.

## Why this shape

If the contract is stated only in prose, every adapter will interpret atomicity, idempotency
and lease semantics slightly differently, and the differences will surface as production
incidents. Encoding the invariants as adapter-agnostic contract tests is what makes the
next two issues checkable rather than trust-based.

## Dependencies

```
ADR-018  ──►  [THIS ISSUE]  ──► INPROC ──► HTTP ──► SEAM ──► VERIFY
```

## In Scope

The domain model, the error taxonomy, the port, and the contract test suite.

## Out of Scope

Adapters, storage, network, configuration, wiring.

## Acceptance Criteria

See front matter. The load-bearing one: **an idempotency key is part of the reserve request**,
so a retry cannot double-reserve *by construction*, not by adapter discipline.

## Notes

- No panics on fallible paths — the panic ratchet applies to new code.
- No `tokio`/`reqwest` types in `domain/` or `interfaces/`.
