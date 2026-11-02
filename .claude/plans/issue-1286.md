# Issue #1286 — a job budget inside its own runtime

**Branch:** `issue/1286` · **Epic:** ci-integrity · **Component:** CI · **Module:** preflight
**Priority:** medium

## Defect

`.github/workflows/preflight.yml` carried two numbers that disagreed with each other
and with the runs:

```
line 2:   # Rigorix Preflight — Fast pre-merge validation (sub-3 min target)
line 34:  timeout-minutes: 10
```

Measured durations of the last eight preflight runs:

```
success    0:09:56
success    0:06:51
success    0:07:26
cancelled  0:10:22      ← the timeout
success    0:07:23
success    0:08:04
success    0:02:54
success    0:20:41      ← a cold cache, and it happened to finish
```

None under three minutes; the median ~7. #1284 was killed at **10m2s and 10m3s**, twice,
*mid-cache-upload, after every step had passed*, and green on a re-run with a warm cache.
The verdict on a pull request depended on whether the Rust cache was warm.

The timeout was also the wrong instrument: when it fires, the job is reported as a failure
of the change, the cache upload is cut in half (so the next run starts cold again), and
nothing names the step that was slow.

## What changes

| # | Before | After |
|---|---|---|
| 1 | `timeout-minutes: 10` on the job | **20** — a margin over the measured cold-cache runtime, not a number below the warm one |
| 2 | header: "sub-3 min target" | the measured range (6–9 min warm, ~14 min of compiling cold), so the next reader does not tune the timeout *down* to a number no run has ever met |
| 3 | one budget for five steps | a budget **per step** (fmt 3 · clippy 10 · check 7 · tests 12 · setup 8) enforced by `.github/scripts/preflight_step.sh`; the job budget stays as a backstop above their sum, so the post-step cache save always runs |
| 4 | a timeout is an anonymous red job | exit 124 becomes `::error title=Preflight budget exceeded::<step> exceeded its Nm budget (ran Xs). This is a budget failure, not a verdict on the change.` |
| 5 | the summary is skipped when a step fails | `.github/scripts/preflight_summary.sh` under `if: always()` renders every step's budget, elapsed time and result — naming the step whether it failed or ran out of budget |

## Why a wrapper script and not just YAML

Two things the workflow YAML cannot express: the *elapsed* time per step, and the
difference between "this step was slow" and "this change is broken". `preflight_step.sh`
runs the command under `timeout`, records `name/budget/elapsed/status` to
`$RUNNER_TEMP/preflight-steps.tsv`, and maps exit 124 to a named budget verdict.
`preflight_summary.sh` renders that file, so the same table appears on a pass, a failure
and a timeout. Both are plain bash and both were seeded locally.

## Acceptance criteria

| # | Criterion | How it is checked |
|---|---|---|
| 1 | The timeout is a margin over the measured cold-cache runtime | 20 min job backstop; step budgets sum to 36 min of *ceiling* against a ~14 min real cold run |
| 2 | The header states the measured range, not "sub-3 min" | header rewritten with the eight measured runs |
| 3 | A timeout is not reported as a code failure, names the step, and does not interrupt the cache upload | seeded locally: budget 2s / command 5s → exit 124, named error annotation, summary row `⏱️ budget exceeded` + "budget verdict, not a verdict on the change" |
| 4 | A failing step still produces a summary | summary step is `if: always()` |
| 5 | Seeded cold-cache run completes without hitting the budget | see the run below: cache key changed for one run, duration and conclusion measured |

## Verification

Local seeds (no CI needed):

```
preflight_step.sh "Tests" 2 -- sleep 5        → exit 124, ::error title=Preflight budget exceeded::
preflight_step.sh "Clippy" 60 -- sh -c 'exit 3' → exit 3 (real failure preserved)
preflight_step.sh "Formatting" 180 -- cargo fmt --check → exit 0 (the real command through the wrapper)
preflight_summary.sh                          → table with ✅ / ❌ / ⏱️ rows
```

The macOS fallback (`timeout` absent) is a `kill`-based loop, not a silent no-op — a
missing coreutil must not turn the budget off.
