# Issue groups — batch 6

Fetched at `c8d162e1`. Two open issues, both from the review of batches 5 and 6, both the same
class: **the record does not match the fact it claims.** Grouped as a related batch because the
fix in each is a correction to a record, and the second is a defect in the first.

| Issue | ID | Priority | Module |
|---|---|---|---|
| #1509 | WASI-9 | **high** | `CHANGELOG.md`, `docs/runbook.md`, `wasi/ci/`, `release.yml` |
| #1508 | ADR-3 | low | `engine/.pi/scripts/validate-adr-references.sh` |

**Branch:** `feature/record-vs-fact-1508-1509`. Order: #1509 → #1508 (priority, and #1509 adds a
check while #1508 fixes a summary line).

## Measured before planning

`v1.14.3` = `c8d162e1`. Three digests for one tag, and the documented one denotes nothing
downloadable:

| where | digest | bytes | how |
|---|---|---|---|
| release page (`package-wasi`, ubuntu-latest) | **`ab77bb82…`** | 1,899,431 | `check-release-asset.sh v1.14.3` |
| **docs** (`CHANGELOG.md:117`, `docs/runbook.md:90`) | `b0ed0ee6…` | 1,897,095 | my measurement — **of the wrong tree** |
| tag rebuilt here (macOS, rustc 1.96.1) | `0cfacf43…` | 1,899,660 | worktree at `v1.14.3` |

`b0ed0ee6…` was rebuilt from `9727559b` (version **1.14.2**, before the bump) and matches
byte-for-byte — a real number for a tree that never shipped. So the delta to the tag is the
version string, not only the platform: the bump changed the bytes, and I measured before it.

## Validation per issue

| # | Gates |
|---|---|
| #1509 | docs, canonical, ci; `wasi/ci/check-recorded-digest.sh --self-test`; `check-release-asset.sh v1.14.3` |
| #1508 | tests, canonical; `validate-adr-references.sh` + `--self-test`; `validate-guards.sh` |
