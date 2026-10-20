---
guardian_issue:
  id: "ISSUE-IDEMPOTENT-RETRY-GUARD-REPLACEMENT-UNTESTED"
  title: "the fix for retry-replaces-guard (found by the capacity demo) has no test — a future edit to the guard registry silently reopens it"
  epic: "capacity ledger"
  component: "CapacityLedgerService"
  module: "capacity-ledger"
  status: planned
  priority: high
  dependencies:
    - "#1049 — the service owns the guards"
    - "#1052 — an idempotent retry returns the same reservation id"
    - "the capacity-ledger demo, which found the interaction"

  intent: |
    Pin the fix that the capacity demo discovered by running it.

    The interaction: an idempotent retry (same key, same amount) returns the **same reservation id**
    (#1052). Registering a guard for that id a second time **replaces** the owner, and the replaced
    guard's `Drop` attempts a release — handing back a hold the caller is about to commit. The
    caller's own `commit` then fails with `Contended { … already Released }`.

    The fix is correct (`Entry::Vacant`, so the retry path constructs no guard). It has no test. The
    demo that found it is not a test: it prints, it does not assert.

  in_scope:
    - "A test that reserves, retries with the same key and amount, then commits successfully"
    - "Assert the retry returns the same reservation id and that exactly one guard exists"
    - "Assert the commit succeeds (the regression was Contended/already Released)"
    - "Assert exposure is correct after the commit (one hold consumed, not two)"
    - "Show the test fails against the pre-fix code (remove the Entry::Vacant guard)"

  out_of_scope:
    - "Re-deciding guard ownership (#1049 is settled: the service owns them)"
    - "The demo itself (it is the discovery tool, not the guard)"
    - "Other idempotency behaviour (#1052's amount-conflict path already has a test)"

  affected_layers:
    domain:
      - "None"
    application:
      - "Modified: tests only"
    infrastructure:
      - "None"
    api:
      - "None"

  canonical_references:
    - evidence: "engine/src/capacity_ledger/application/service_impl.rs:108 — if let Entry::Vacant(slot) = guards.entry(reservation.id) { slot.insert(ReservationGuardImpl::new(..)) }"
    - evidence: "engine/src/capacity_ledger/application/service_impl.rs:99-104 — the comment states the retry returns the same reservation id and a second guard's drop releases the caller's hold"
    - evidence: "engine/tests/capacity_ledger_composition_e2e.rs — 7 tests, none reserves twice with the same key then commits"
    - evidence: "engine/src/capacity_ledger/infrastructure/in_process.rs:498 — the only retry test is retry_after_expiry_does_not_create_a_second_reservation (expiry, not guard replacement)"

  acceptance_criteria:
    - "A test reserves, retries with the same key and amount, then commits"
    - "It asserts the same reservation id is returned and one hold is consumed"
    - "Removing the Entry::Vacant guard makes it fail (falsifier recorded in the PR)"
    - "It lives with the service's tests, not only in the composition e2e file"

  validators:
    - ci
    - tests

  implementation_notes: |
    The falsifier matters more than usual here. The pre-fix behaviour is a *race*: whether the
    replaced guard's Drop runs before the commit depends on scheduling, which is why the demo failed
    "one run in three". A test that asserts the outcome may pass against the broken code by luck.

    So assert the invariant rather than the timing: after an idempotent retry there must be exactly
    **one** guard for that reservation id. That is deterministic, and it is the property the drop
    race violates. Asserting the commit succeeds is necessary but not sufficient.

    Also worth asserting across several retries (three or four), not just one: the double-release
    is more likely to surface with more guards replaced.

  file_changes:
    - "create: engine/tests/unit/capacity_ledger/idempotent-retry-guard_test.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-IDEMPOTENT-RETRY-GUARD-REPLACEMENT-UNTESTED

## How it was found

The capacity-ledger demo failed **one run in three** at the commit step. That flakiness is the whole
story: it is a scheduling-dependent race, and it needed an end-to-end run to appear at all.

## The interaction

Two decisions, each correct alone, that combine into a bug:

| decision | source | what it means here |
|---|---|---|
| the service owns the guards | #1049 | `guards: HashMap<ReservationId, ReservationGuardImpl>` |
| an idempotent retry returns the **same** reservation id | #1052 | the retry is not a second hold |

Together: a retry registers a second guard **for an existing id**, which replaces the owner. The
replaced guard is dropped, and `Drop` releases — returning a hold the caller is about to commit:

```
commit → Contended { … already Released }
```

## The fix

```rust
// service_impl.rs:108
if let std::collections::hash_map::Entry::Vacant(slot) = guards.entry(reservation.id) {
    slot.insert(ReservationGuardImpl::new(self.ledger.clone(), grant));
}
```

A retry constructs no guard, so there is nothing to drop and no race with the durable release.
Correct, and well-commented.

## Why it needs a test anyway

The fix is a **guard clause inside a mutex** that exists to prevent a drop race. Nothing fails if it
is removed. The demo demonstrates the behaviour but asserts nothing, and it is not in CI.

Concretely, the regression returns the moment someone:
- changes `entry(...)` to a plain `insert(...)` while "simplifying"
- makes the retry path return a fresh grant object (reopening #1052's shape)
- moves guard registration out of the `reserve` path

None of those trip a test today.

## The falsifier is the hard part

Because the defect is a race, a test that merely asserts "commit succeeds" can pass against the
broken code by scheduling luck — the demo failed one run in three, not one in one.

Assert the invariant instead: **after an idempotent retry there is exactly one guard for that
reservation id.** Deterministic, and precisely what the drop race violates.
