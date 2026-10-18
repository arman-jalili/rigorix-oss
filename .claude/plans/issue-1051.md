# Batch 3 — Issue #1051

**Branch:** `issue/1051`
**Decision:** a bounded `Window` is a **rolling duration** — at most `hard_limit`
units in any trailing W seconds.

## Changes
- `LedgerState.committed` is now `HashMap<ResourceId, VecDeque<(DateTime, u64)>>`
  (timestamped commit entries, oldest first).
- `committed_in_window` prunes entries older than W and sums the rest.
  `Window::UNBOUNDED` never prunes.
- `reserve`, `declare_resource` and `snapshot` use windowed committed.
- `resolve(Committed)` appends `(now, amount)`.
- Only committed capacity ages out; outstanding/ambiguous holds always count.
- `declare_resource` validates against the **new** window.

## Tests
- spend to the cap in a 1s window → `Insufficient`; sleep past W → available again.
- unbounded window never ages out.

## Docs
Runbook "Window semantics" section + invariant I7; module Resource purpose.
