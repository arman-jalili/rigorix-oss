# Issue groups — batch 3 (portable-slice follow-ups)

Fetched 2026-11-04. Five open issues, all `epic: portable-slice`, all opened after ADR-014
landed. None is blocked; all inherit the same decision, which is why they group cleanly.

## Group 1 — `issue/1365` (tooling, single)

| | |
|---|---|
| **#1365** | The guard derives `20 items` and `9 dependencies`, prints them, and never compares them to the record |

Single by the rules: one file (`.pi/scripts/validate-portable-inventory.sh`), one plan, and it
is a *precondition* for the others — Phase C will move counts in the ADR, and this makes the
counts fail loudly first. Nothing else touches the guard.

## Group 2 — `feature/wasm-docs-adr-014-1366-1368` (component + related batch, 3 issues)

| | |
|---|---|
| **#1366** | Roadmap Phase 5, runbook §3/known-limits, module doc still send a reader to the `Clock`/`IdSource` seam ADR-014 removed |
| **#1367** | The `normalised_comparison=true` permission's precondition expired; harness must refuse the flag |
| **#1368** | The runbook says "eight gates"; there are ten, and nothing compares the number to the script |

Grouped because the files are the same files:

| file | #1366 | #1367 | #1368 |
|---|---|---|---|
| `runbook.md` | §3, known-limits rows | — | the gate count on :5 |
| `modules/portable-session.md` | criteria 19–21, :502 note | criterion 22, :502 note | — |
| `implementation-roadmap.md` | Phase 5 title/goal/days, totals | — | — |
| `wasm/ci/run-conformance.sh` | — | the flag assertion | — |

Order inside the batch: **#1366 → #1367 → #1368.** #1366 rewrites the module doc's blocked note,
#1367 rewrites the same note's criterion 22 — doing them the other way would edit the sentence
twice. #1368 is last because it is one line plus one check and depends on nothing.

## Group 3 — `issue/1369` (CLI, single)

| | |
|---|---|
| **#1369** | `rigorix-cli` 1.12.1 publishes `spike-m2` and a hidden `pinned-refusal`, with no test that a default build excludes them |

Single: one crate (`cli/`), and it is the only issue that changes shipped behaviour rather than
documentation or tooling. It also needs a build in two feature configurations, so it does not
share a validation loop with the docs batch.

---

## Execution order

1. **#1365** — closes the "computed, printed, ignored" class before Phase C moves the numbers.
2. **#1366 + #1367 + #1368** — one docs batch, one PR; the record and the plan agree afterwards.
3. **#1369** — the capability rename, last, because it is the only one with a compile-time surface.

Phase C is *not* in this series: it is implementation work with no issue, and the three "no, yet"
rows in ADR-014 are its exit criteria.
