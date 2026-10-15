---
guardian_epic:
  id: "EPIC-CAPACITY-LEDGER"
  title: "feature: distributed capacity ledger — cross-session aggregate enforcement"
  status: planned
  priority: high
  created_at: "2026-10-14T00:00:00Z"
  adr: ".pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
  module: ".pi/architecture/modules/capacity-ledger.md"

  intent: |
    Make aggregate exposure across independent executions enforceable. Today a single run can
    be capped; five sessions that each stay under a per-session cap can jointly exceed the cap
    that matters. A pre-check-then-increment counter cannot fix this. This epic adds a new
    bounded context owning the model and the port for contested capacity, with a default
    in-process adapter that preserves the standalone OSS guarantee and an opt-in HTTP adapter
    that reaches a durable shared store (the enterprise ledger service) for distributed mode.

  issues:
    - "ISSUE-CAPACITY-LEDGER-DOMAIN"
    - "ISSUE-CAPACITY-LEDGER-INPROC"
    - "ISSUE-CAPACITY-LEDGER-HTTP"
    - "ISSUE-CAPACITY-LEDGER-SEAM"
    - "ISSUE-CAPACITY-LEDGER-VERIFY"

  out_of_scope:
    - "Storage in the engine (no sqlx/redis dependency will be added)"
    - "The enterprise service itself (tracked in the enterprise repo)"
    - "Changing sequence_policy composition semantics"
    - "Making the ledger mandatory — it is opt-in per resource"
    - "Demo work and doc rewrites (separate items)"

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# EPIC-CAPACITY-LEDGER — distributed capacity ledger

## Why

The requirement is a shared cross-session risk ledger with **reserve-then-authorize**
semantics. The engine already ships a *real* reservation lifecycle (ADR-008 RAII,
`budget_tracking`) — but it is LLM-scoped and its state is in-process, and the generic
`enforcement.ResourceBudget` is a **counter with a threshold**, i.e. exactly the naive
counter the requirement says is insufficient.

So the missing part is not the pattern. It is **atomicity across independent executions**:
a durable contested store, plus idempotency and lease semantics that make concurrency safe.

## Target outcome

- A new engine context `capacity_ledger` owns the model and the port; no storage.
- **OSS stays standalone**: no DB, no network, no shared state unless a resource is declared
  ledger-governed. Default adapter is in-process.
- **Distributed mode is opt-in** and reached over HTTP (the enterprise service owns Postgres).
- `enforcement` performs reserve → (allow) → commit / release on the execution path, proven
  by a **wiring/reachability test**, not unit tests alone.
- One conformance suite passes against **every** adapter (I1–I6 identically).

## Scope

See the five issues. Order is load-bearing: domain/port → in-process adapter → HTTP adapter →
seam wiring → verification. The verification issue is not optional and not last-minute: it is
the mechanism that proves the feature is *wired*, not merely *implemented*.

## Out of scope

Storage in the engine. The enterprise service. Sequence-policy composition. Demo/docs.

## Risks

- **The hard part is the adapter's concurrency semantics**, not the port. Budget design time.
- **Fail-closed makes the control plane a critical path** for a configured cap.
- New panic-surface and drift baselines may shift — new code returns `Result` or is baselined.
