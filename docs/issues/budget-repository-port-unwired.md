---
guardian_issue:
  id: "ISSUE-BUDGET-REPOSITORY-PORT-UNWIRED"
  title: "budget_tracking: the LlmBudgetRepository persistence port exists but nothing calls it"
  epic: "architecture hygiene"
  component: "BudgetTracking"
  module: "budget-tracking"
  status: planned
  priority: medium
  dependencies:
    - "ADR-018 capacity ledger (decides the long-term home of reservations)"

  intent: |
    Resolve a dead seam. The LlmBudgetRepository port declares save/record_reservation/
    record_commit/list and documents "persistent storage", but llm_budget_impl.rs never
    references the repository, the only implementation is in-memory, and record_reservation is
    called only by test_budget_round_trip. Either wire it or remove it — a port that exists to
    look like persistence is worse than no port, because it reads as a guarantee.

  in_scope:
    - "Decide and record: implement a durable adapter, fold into the capacity ledger, or delete the port"
    - "Implement the decision"
    - "If deleted: remove the trait, the in-memory adapter, and the round-trip test, and confirm nothing references them"
    - "If wired: an adapter plus a test that proves it is called on the real path"
    - "A guard against the reappearance of an unwired port of this shape"

  out_of_scope:
    - "The capacity ledger itself"
    - "Changing LLM budget semantics"

  affected_layers:
    domain:
      - "Possibly modified: LlmBudgetRepository trait (delete or narrow)"
    application:
      - "Modified: llm_budget_impl.rs (call the repository, or drop the dependency)"
    infrastructure:
      - "Modified: in_memory_budget_repository.rs (delete, or keep as the test adapter)"
    api:
      - "None"

  canonical_references:
    - module: ".pi/architecture/modules/capacity-ledger.md"
    - evidence: "engine/src/budget_tracking/infrastructure/repository/mod.rs"
    - evidence: "engine/src/budget_tracking/infrastructure/in_memory_budget_repository.rs"

  acceptance_criteria:
    - "No trait method exists that has no production caller (verified by search, not by reading)"
    - "If a durable adapter is kept, a reachability test proves it is invoked on the execution path"
    - "Deleting the port leaves no dangling references (build + tests green)"
    - "The decision is recorded in the module doc or the ADR"

  validators:
    - ci
    - tests
    - architecture
    - canonical

  implementation_notes: |
    - The honest default here is deletion unless a durable store is actually wanted now; the
      capacity ledger is the designed home for contested persistent reservations.
    - "Called only by tests" is the falsifier: if the only caller is a test, the seam is dead.

  file_changes:
    - "modify or delete: engine/src/budget_tracking/infrastructure/repository/mod.rs"
    - "modify or delete: engine/src/budget_tracking/infrastructure/in_memory_budget_repository.rs"
    - "modify: engine/src/budget_tracking/application/llm_budget_impl.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-BUDGET-REPOSITORY-PORT-UNWIRED

## Intent

A declared persistence port that nothing persists through is an active hazard: a reader
reasonably concludes that budgets are durable. Resolve it deliberately.

## Evidence

| claim | evidence |
|---|---|
| the port declares persistence | `repository/mod.rs` — `save`, `record_reservation`, `record_commit`, `list` |
| nothing calls it | `llm_budget_impl.rs` has **zero** references to the repository |
| only tests call it | graph: `record_reservation` → *"Called by: test_budget_round_trip"* |
| only one adapter exists | `in_memory_budget_repository.rs` is the sole implementation |

## Decision required

Delete, fold into the capacity ledger, or wire a durable adapter. Record which, and why.

## Acceptance Criteria

No trait method without a production caller. If kept, a reachability test — not unit coverage.
