# Issue Implementation Groups — Round 2

**Generated:** 2026-10-18
**Open issues:** #1050–#1054, #1060, #1061 (skip #1018 — deferred)

## Batches (dependency order)

| Batch | Branch | Issues | Rationale |
|-------|--------|--------|-----------|
| 1 | `issue/1060-1061` | #1060, #1061 | Consequences of #1016/#1017: stale docs + unbound `shared_contested` footgun |
| 2 | `feature/capacity-idempotency-ambiguity` | #1052, #1050 | The issue notes say these must be resolved together (key lifecycle ↔ ambiguous outcome) |
| 3 | `issue/1051` | #1051 | Window semantics (rolling duration), independent of 1–2 |
| 4 | `feature/capacity-guarantee-accuracy` | #1053, #1054 | Both correct the stated guarantee: single-process atomicity + best-effort Drop |

## Decisions
- #1051: **rolling duration** — at most N units in any trailing W seconds.
- #1050: add an explicit **ambiguous** reservation state that retains exposure
  until reconciled; expiry alone never proves "no effect".
- #1052: same key + different amount → typed conflict; retry after expiry →
  reject (never silently create a second reservation).
- #1061: **refuse** a `shared_contested` budget in config validation until a
  ledger binding exists (fail closed, loud).
