---
guardian_issue:
  id: "ISSUE-CAPACITY-LEDGER-VERIFY"
  title: "capacity_ledger: wiring, soundness and end-to-end verification"
  epic: "distributed capacity ledger"
  epic_id: "EPIC-CAPACITY-LEDGER"
  component: "CapacityLedger"
  module: "capacity-ledger"
  status: planned
  priority: critical
  dependencies:
    - "ISSUE-CAPACITY-LEDGER-SEAM"
    - "enterprise ledger service (ADR-010) deployed"

  intent: |
    Prove the finished feature is sound AND wired. This issue is the mechanism, not a
    formality: it exists because a correct implementation that is never invoked, or that is
    invoked only in tests, passes every unit suite and still does nothing in production.
    Runs after all parts land, against a real Postgres and multiple replicas.

  in_scope:
    - "Wiring proof: reachability test + a runtime assertion that the configured adapter is the one in use"
    - "Soundness suite: idempotency, lease expiry, commit-after-expiry, reclaim under N replicas"
    - "Adversarial: N concurrent executions against one real store never exceed hard_limit"
    - "Fail-closed drill: kill the ledger service mid-run; assert refusal, not passage"
    - "Full e2e: multi-replica enterprise + multiple engine executions, one store"
    - "A single command that runs the whole verification (no manual steps)"
    - "Explicit statement of which guarantee applies (single-execution vs distributed)"

  out_of_scope:
    - "Load/performance testing beyond the concurrency correctness case"
    - "The console UI panel (enterprise issue)"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "New: verification harness + scripts"
    api:
      - "None"

  canonical_references:
    - module: ".pi/architecture/modules/capacity-ledger.md"
    - adr:
        - ".pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
    - precedent: ".pi/issues/precondition-followups/03-activate-reachability-test.md"

  acceptance_criteria:
    - "Removing the seam call makes the wiring test FAIL (verified by temporarily removing it)"
    - "Two concurrent executions cannot jointly exceed hard_limit against a real store"
    - "A retried reserve with the same idempotency key yields one reservation, not two"
    - "A killed holder's capacity is reclaimed after lease expiry and becomes usable"
    - "With the ledger service down, governed steps refuse (fail-closed proven, not asserted)"
    - "Every executed scenario states which guarantee it demonstrates"
    - "One command runs the full verification end to end"

  validators:
    - ci
    - tests
    - integration
    - security

  implementation_notes: |
    - A test that cannot fail is worse than no test. Demonstrate the wiring test failing by
      removing the call, then restore it — record both results.
    - The concurrency case must use a real store. An in-process harness proves nothing about
      distributed atomicity and must not be presented as if it did.
    - The fail-closed drill is a first-class scenario, not an afterthought: it is the property
      that distinguishes a cap from a suggestion.

  file_changes:
    - "create: engine/tests/integration/capacity-ledger/e2e_multi_replica_test.rs"
    - "create: .pi/scripts/verify-capacity-ledger.sh"
    - "create: docs/verification/capacity-ledger-verification.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-CAPACITY-LEDGER-VERIFY — wiring, soundness and e2e verification

## Intent

Prove the feature works **and** that it is actually reached in production. Runs last, against
a real Postgres and more than one replica.

## Why this is its own critical issue

Because the two most likely failure modes are invisible to unit tests: (1) the port is correct
but never invoked; (2) the concurrency claim is tested in-process and therefore proves nothing
about the distributed case. Both would look green.

## Acceptance Criteria

The wiring test must be shown to fail when the call is removed. The concurrency and
fail-closed scenarios must run against a real store.

## Notes

State, for every scenario, which guarantee it demonstrates. An in-process run may not be
presented as evidence of a distributed property.
