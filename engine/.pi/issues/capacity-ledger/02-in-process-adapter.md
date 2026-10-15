---
guardian_issue:
  id: "ISSUE-CAPACITY-LEDGER-INPROC"
  title: "capacity_ledger: in-process adapter — the standalone default"
  epic: "distributed capacity ledger"
  epic_id: "EPIC-CAPACITY-LEDGER"
  component: "CapacityLedger"
  module: "capacity-ledger"
  status: planned
  priority: high
  dependencies:
    - "ISSUE-CAPACITY-LEDGER-DOMAIN (port + contract tests)"

  intent: |
    Implement the default adapter: single-execution, in-memory, no network, no DB. This is what
    preserves the OSS standalone guarantee. It must pass the same contract suite as the HTTP
    adapter, and it must state its guarantee honestly — single-execution atomicity only.

  in_scope:
    - "New: InProcessLedger implementing CapacityLedger over in-memory state"
    - "Atomic reserve under intra-process concurrency (the Arc/Mutex discipline already used by LlmBudget)"
    - "Lease-based expiry with explicit reclaim"
    - "Idempotency-key replay returning the same Grant"
    - "Explicit, documented guarantee: single-execution atomicity (claim ceiling)"
    - "Passes the full contract suite"

  out_of_scope:
    - "Cross-process visibility (that is the HTTP adapter's job)"
    - "Persistence across restarts"
    - "Wiring into enforcement"

  affected_layers:
    domain:
      - "None (consumes the domain model)"
    application:
      - "New: InProcessLedger use-case implementation"
    infrastructure:
      - "New: engine/src/capacity_ledger/infrastructure/in_process.rs"
    api:
      - "None"

  canonical_references:
    - module: ".pi/architecture/modules/capacity-ledger.md"
    - adr:
        - ".pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
    - last_implementation: "budget_tracking/application/llm_budget_impl.rs (RAII reference)"

  acceptance_criteria:
    - "Full contract suite passes against InProcessLedger"
    - "Concurrency test: N concurrent reserves never exceed hard_limit (falsifies the naive counter)"
    - "Idempotency test: same key replayed returns the identical Grant; a *different* key over the limit is refused"
    - "Lease test: an expired reservation stops counting toward exposure after reclaim"
    - "Doc comment states the guarantee is single-execution, not distributed"
    - "No network or DB types introduced"

  validators:
    - ci
    - tests
    - architecture

  implementation_notes: |
    - Follow the proven discipline in budget_tracking (Arc + Mutex on the state, RAII handle for
      release-on-drop). Do not invent a second concurrency idiom.
    - The guarantee must be *stated in code* where a reader will see it, because the temptation
      to describe this as "the distributed ledger" is high and wrong.
    - Reclaim should be explicit and testable, not a background timer.

  file_changes:
    - "create: engine/src/capacity_ledger/infrastructure/in_process.rs"
    - "create: engine/src/capacity_ledger/infrastructure/mod.rs"
    - "create: engine/tests/unit/capacity-ledger/in_process_test.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-CAPACITY-LEDGER-INPROC — in-process adapter

## Intent

The standalone default. No DB, no network — this is what keeps "OSS runs with no database"
true while still giving single-execution atomicity.

## Why this shape

Because a distributed guarantee cannot be faked, and the most common way to get this wrong is
to give the in-process adapter a distributed name. Its guarantee is **single-execution
atomicity**; that sentence belongs in the code.

## In Scope

In-memory adapter, intra-process atomicity, leases, idempotency, contract-suite conformance.

## Out of Scope

Cross-process visibility, persistence, wiring.

## Acceptance Criteria

Contract suite passes; under N concurrent reservations the limit is never exceeded; the
guarantee is documented in the code.

## Notes

Reuse the ADR-008 RAII discipline rather than inventing a parallel idiom.
