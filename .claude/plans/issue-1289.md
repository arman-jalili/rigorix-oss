# Issue #1289 — the Final Gate accuses a commit for a run that never happened

**Branch:** `issue/1289` · **Epic:** ci-integrity · **Component:** CI · **Module:** ci-workflow
**Priority:** medium

## Defect

`ci.yml` cancels superseded runs (`concurrency.cancel-in-progress`), and its `Final Gate` ran
`if: always()` with a failing step conditioned on `needs.<stage>.result != 'success'`. A
cancelled stage *is* `!= 'success'`, so the gate took the failing branch and printed a verdict
on the change. Measured on the merge commit of #1287 (`b7414228`), via
`gh api repos/…/commits/b7414228/check-runs`:

```
🔒 Audit: cancelled            📝 Documentation: cancelled
🔗 Integration: cancelled      📦 Build: cancelled
🧪 Test (matrix): cancelled    📐 SDK-schema conformance: cancelled
🚪 Final Gate: failure         ← a verdict on a run that never happened
```

The successor run on `54da9edf` is green, so the tree is fine — but the *commit* carries a
failing check, and distinguishing that from a real red requires reading the job list.

## What changes

| # | Before | After |
|---|---|---|
| 1 | the failing step fires on `!= 'success'`, which includes `cancelled` | it fires for `failure` and (policy unchanged) `skipped` only — a cancelled stage is never an accusation |
| 2 | a superseded run's gate is `failure` | the gate job is **skipped** (`always() && !contains(needs.*.result, 'cancelled')`) — a grey status, not a verdict. Green would be a verdict too, and the run that never finished has no standing to give one |
| 3 | the results table lived only in the gate, so skipping it would lose the evidence | a new `📋 Stage report` job runs `always()`, never fails, and prints every stage's result — so a reader sees `cancelled` per stage |
| 4 | the verdict was five YAML expressions | one script (`.github/scripts/ci_verdict.sh`) that classifies and names the reason: `::notice title=Run superseded::No verdict on this commit — …` |

`.github/scripts/ci_stages.sh` holds the table renderer both jobs share, so the log and the run
summary cannot drift.

### Why `skipped` and not a green gate

A superseded run never evaluated the change. Reporting it as `success` would be the same lie in
the other direction. The issue's own acceptance says `neutral`/`skipped`; GitHub gives no way to
set a job conclusion to `neutral` from workflow YAML, and `skipped` is exactly the conclusion it
assigns to a job whose `if` is false.

## Acceptance criteria

| # | Criterion | How it is checked |
|---|---|---|
| 1 | the failing step fires for `failure` (`skipped` unchanged), not `cancelled` | seeded: `RESULT_TEST=failure` → exit 1; all `cancelled` → exit 0 with the notice |
| 2 | a cancelled/superseded run reports `neutral`/`skipped` for the gate, naming the reason | job-level `if` skips the gate; the notice and summary name the cancelled stages |
| 3 | the results table keeps printing every stage | `ci-report` job, `if: always()` |
| 4 | a genuine failure still fails the gate | seeded: `exit 1` naming the stage |

## Seeds (local, no CI needed)

```
all success                    → exit 0  ✅ All mandatory stages passed: lint build test audit conformance
test failure                   → exit 1  ❌ Mandatory stage(s) did not pass: test
all cancelled                  → exit 0  ⏹️ No verdict: the run was superseded (cancelled: …) + ::notice
lint success, rest cancelled   → exit 0  ⏹️ No verdict (partial completion is still no verdict)
mandatory skipped              → exit 1  ❌ Mandatory stage(s) did not pass: lint
no wiring (empty envs)         → exit 1  ❌ … (a missing stage is not a pass)
docs cancelled, mandatory ok   → exit 0  ✅ (docs is reported, not required)
```

A live cancelled run is measured on this branch (two pushes: the first run is superseded).
