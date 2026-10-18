---
guardian_issue:
  id: "ISSUE-CAPACITY-GUARD-ASYNC-DROP"
  title: "capacity_ledger: ReservationGuard drop performs a fire-and-forget async release and swallows the failure"
  epic: "capacity ledger"
  component: "CapacityLedger"
  module: "capacity-ledger"
  status: planned
  priority: medium
  dependencies:
    - "ISSUE-CAPACITY-GUARD-DTO (#1049) — the service never returns the guard, so this path is only reachable via the port directly"
    - "ISSUE-CAPACITY-AMBIGUOUS-OUTCOME — release is only safe when the effect provably did not happen"

  intent: |
    Stop treating Drop as a correctness mechanism for a remote release. Drop cannot await, so
    ReservationGuardImpl spawns the release onto a captured runtime handle and discards the
    result. If the runtime is gone, or the spawned task never runs, or the release returns an
    error, capacity is held until the lease lapses and nothing records that cleanup failed.

    That is acceptable as belt-and-braces for a local in-memory release; it is not acceptable
    as the documented guarantee. The contract has to say what Drop can and cannot promise, and
    the recovery path has to be the durable lease, not the drop.

  in_scope:
    - "State the rule: Drop performs only cleanup that can be guaranteed synchronously"
    - "Make the async release best-effort and explicit about being best-effort (log at warn with the reservation id, do not silently discard)"
    - "Make normal transitions explicit and awaited (commit/release), never 'rely on drop'"
    - "Confirm the recovery path is the durable lease + reconciliation, and document it as such"
    - "Guarantee that a failed cleanup never converts a possibly-completed effect into available capacity"
    - "Add a test that a drop outside a runtime (or with a failed release) does not make capacity look reusable before expiry"

  out_of_scope:
    - "Making Drop async (not possible in Rust)"
    - "The DTO field question — tracked separately in #1049"
    - "The ambiguity state machine — tracked in ISSUE-CAPACITY-AMBIGUOUS-OUTCOME"

  affected_layers:
    domain:
      - "None"
    application:
      - "Modified: guard_impl.rs Drop — explicit best-effort semantics + observable failure"
      - "Modified: guard.rs — the trait doc must state what Drop guarantees and does not"
    infrastructure:
      - "None"
    api:
      - "None"

  canonical_references:
    - module: ".pi/architecture/modules/capacity-ledger.md#reservationguard"
    - adr:
        - ".pi/architecture/decisions/ADR-008-raii-budget-reservation.md"
        - ".pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
    - evidence: "engine/src/capacity_ledger/application/guard_impl.rs — Drop spawns release and discards the Result"
    - evidence: "engine/src/capacity_ledger/application/guard_impl.rs — no-runtime branch logs a warning and relies on lease expiry"
    - evidence: "engine/src/execution_engine/application/service_impl/dispatch.rs — the seam settles by id, not via Drop"

  acceptance_criteria:
    - "The guard trait documents precisely what Drop guarantees (local cleanup only; remote release is best-effort)"
    - "A failed or skipped async release is observable (warn with reservation id), not silently discarded"
    - "Normal completion uses an awaited commit/release; no code path relies on Drop for correctness"
    - "A drop outside a runtime does not make capacity appear reusable before lease expiry"
    - "The durable lease is documented as the recovery mechanism, not Drop"
    - "A test covers the no-runtime drop path"

  validators:
    - ci
    - tests
    - architecture

  implementation_notes: |
    - Replace 'let _ = ledger.release(grant).await;' with an explicit match that logs at warn on
      error, naming the reservation id. A swallowed error is indistinguishable from success.
    - The no-runtime branch is already honest (it warns); make the spawned branch equally
      observable.
    - Do not add a blocking wait inside Drop. Blocking a runtime thread to perform cleanup is
      worse than the problem it solves.
    - Keep the pattern: Drop is a safety net, the lease is the guarantee. Say that in the docs.

  file_changes:
    - "modify: engine/src/capacity_ledger/application/guard_impl.rs"
    - "modify: engine/src/capacity_ledger/application/guard.rs"
    - "create: engine/tests/capacity_ledger_guard_drop.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-CAPACITY-GUARD-ASYNC-DROP — Drop cannot be the correctness mechanism

## The defect

```rust
Some(handle) => {
    let ledger = self.ledger.clone();
    handle.spawn(async move {
        let _ = ledger.release(grant).await;    // <-- result discarded
    });
}
None => {
    tracing::warn!("... dropped outside a Tokio runtime; relying on lease expiry to reclaim it");
}
```

Rust's `Drop` cannot await. So the release is spawned and its outcome thrown away. If the task
never runs, the runtime has shut down, or the release returns an error, the hold survives until
the lease lapses — and **nothing records that cleanup failed**.

## Why it matters

For a local in-memory release this is a reasonable belt-and-braces. It is not reasonable to
describe it as a guarantee. Two specific hazards:

1. **Silent failure.** `let _ =` makes a failed release indistinguishable from a successful one.
2. **Ambiguity.** Releasing on drop assumes the effect did not happen. For a side-effecting
   resource that assumption is exactly what the ambiguous-outcome issue says must never be made
   automatically.

## Required contract

| mechanism | guarantees |
|---|---|
| `Drop` | local cleanup only, best-effort; remote release may not happen |
| explicit `commit` / `release` (awaited) | the normal path; must be used for correctness |
| durable lease + reconciliation | the actual recovery mechanism when a process dies or cleanup fails |

**RAII is a safeguard against forgotten cleanup paths, not a substitute for durable recovery.**

## Notes

Scope is deliberately narrow: the service currently returns `guard: None` (#1049), so this
path is reachable only by calling the port directly — which the contract suite and the HTTP
adapter's internal grant handling do. It is still the documented guarantee, so it must be
accurate.
