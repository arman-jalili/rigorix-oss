# Batch 3 — Issue #1016

**Branch:** `issue/1016`
**Decision:** delete both dead persistence ports; persistence is the adapter's job.

## Rationale
- `LlmBudgetRepository` had one impl (`InMemoryLlmBudgetRepository`) used only in
  its own tests; `LlmBudgetImpl` owns its counters and persists nothing.
- `CapacityLedgerRepository` had **zero** implementations and zero callers; the
  real ledger port (`CapacityLedger` → `InProcessLedger`/`HttpLedger`) already
  owns transport/storage, and `HttpLedger` delegates durability to the
  enterprise ledger service.

## Changes
- Delete `budget_tracking/infrastructure/repository/` + `in_memory_budget_repository.rs`.
- Delete `capacity_ledger/infrastructure/repository/`.
- Update both `infrastructure/mod.rs` docs and the budget module tree.
- Record the decision in `budget-tracking.md`, `runbook-budget-tracking.md`,
  and `capacity-ledger.md`/`infrastructure/mod.rs`.

## Acceptance
- No trait method in either module lacks a production caller (ports removed).
- Build + tests prove the adapters did not rely on the deleted seam.
- Decision recorded in module docs.
