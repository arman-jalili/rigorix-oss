# Batch 2 — Issues #1017 + #1019

**Branch:** `feature/enforcement-ledger-1017-1019`

## Decision (one authoritative mechanism per resource)
- `ResourceBudget` is a **per-execution counter**, authoritative only for
  non-contested limits. New `BudgetScope` (`per_execution` default /
  `shared_contested`) makes the authority explicit in code and config.
- Contested limits are declared in `.rigorix/capacity.toml` and enforced by the
  capacity ledger; a `shared_contested` budget never blocks locally.
- `ToolPolicy::max_calls` is a per-execution counter (same rule), so it is
  implemented locally against a per-tool call map rather than a third ledger.

## Changes
### #1017
- `BudgetScope` enum + `ResourceBudget.scope` (serde default `per_execution`).
- `BudgetSnapshot.authoritative` / `ResourceBudgetStatus.authoritative`.
- Hard-deny and `limit_exceeded` gated on `scope.is_authoritative()`.
- Docs: enforcement runbook + architecture module + capacity-ledger module.

### #1019
- Delete the "reserved for a future implementation" placeholder.
- `EnforcerState.tool_calls: HashMap<String, u64>`; `TrackResourceUsageInput.tool`.
- `evaluate_tool_call` denies when a tool's count reaches `max_calls`.
- Tests: shared budget is advisory; per-tool limit blocks only that tool;
  concurrent counting reaches exactly the limit.

## Validation
ci, tests, architecture, canonical, security, panic surface.
