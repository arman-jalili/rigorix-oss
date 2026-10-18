# Issue Implementation Groups

**Generated:** 2026-10-18
**Total Issues:** 5 (of 6 open; #1018 skipped — explicitly deferred)

## Grouping Strategy
Component + dependency order. Each batch = one branch + one PR.

## Batches (dependency order)

| Batch | Branch | Issues | Component | Why this order |
|-------|--------|--------|-----------|----------------|
| 1 | `issue/1049` | #1049 | capacity_ledger (application/DTO) | Correctness-of-contract bug; foundational ownership decision |
| 2 | `feature/enforcement-ledger-1017-1019` | #1017, #1019 | enforcement + capacity_ledger | #1019 explicitly ordered after #1017 (one authoritative counting mechanism) |
| 3 | `issue/1016` | #1016 | persistence ports (budget_tracking, capacity_ledger) | Needs the ledger decision from batch 1/2 |
| 4 | `issue/1013` | #1013 | docs (README) | Independent; fix dangling cross-reference + positioning |

## Excluded
- **#1018** — `[deferred]` README demo; blocked on a capacity-ledger demo that does not exist.

## Validation per batch
`validate-ci.sh`, `validate-tests.sh`, `validate-architecture.sh`,
`validate-canonical.sh`, `validate-security.sh`, `validate-operations.sh`,
plus `validate-panic-sites.sh` where production code changes.
