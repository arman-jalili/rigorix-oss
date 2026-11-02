---
guardian_issue:
  id: "ISSUE-PREFLIGHT-TIMEOUT-IS-A-CACHE-COIN-FLIP"
  title: "Preflight's 10-minute timeout is inside its own cold-cache runtime, so a passing PR is killed mid-upload"
  epic: "ci-integrity"
  component: "CI"
  module: "preflight"
  status: planned
  priority: medium
  tracking_issue: 1286
  dependencies: []

  intent: |
    `.github/workflows/preflight.yml` carries two numbers that disagree with each other and with the runs:

    ```
    line 2:   # Rigorix Preflight — Fast pre-merge validation (sub-3 min target)
    line 34:  timeout-minutes: 10
    ```

    Measured durations of the last eight preflight runs (`updatedAt - createdAt`, which includes queueing
    where the run waited):

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

    None is under three minutes; the median is around seven; and a cold cache has produced both a 20-minute
    run and a kill at the timeout. Reported on #1284: killed at **10m2s and 10m3s**, twice, *mid-cache-upload,
    after every step had passed*, and green on a re-run with a warm cache. #1279 passed at 6m48s. So the verdict
    on a pull request currently depends on whether the Rust cache was warm — a passing change can be red
    because a download was slow.

    The timeout is also the wrong instrument for the failure mode: when it fires, the job is reported as a
    failure of the change, and the artifact upload is cut in half, which makes the next run start cold again.

  in_scope:
    - "`timeout-minutes` is a margin over the measured cold-cache runtime, not a number below the warm one: 20 minutes"
    - "The header's target is the measured range (6–9 minutes warm, 20 cold), not 'sub-3 min', so the next reader does not tune the timeout down to it"
    - "A timeout is not reported as a code failure: the job's summary says the budget was exceeded and names the step, and the cache upload is not interrupted by the budget (it runs under `if: always()` with its own step budget)"
    - "Seeded: a cache-cold run (`.github/workflows/preflight.yml` with the cache key changed) completes without hitting the budget"

  out_of_scope:
    - "Making the workspace build faster; that is a larger change and does not remove the dependence on a warm cache"
    - "Removing preflight from the PR checks"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "Modified: `.github/workflows/preflight.yml` — the job budget, the header, the timeout summary"

  citations:
    - file: ".github/workflows/preflight.yml"
      note: ":2 the sub-3 min target; :34 timeout-minutes: 10"

  verification:
    - "a cold-cache run finishes inside the budget"
    - "the reported duration of a passing run is unchanged (the budget is not the thing being measured)"
---

# ISSUE-PREFLIGHT-TIMEOUT-IS-A-CACHE-COIN-FLIP

## Measured

| run | duration | outcome |
|---|---|---|
| 2fcfbcc | 0:07:26 | success |
| 3459374 | 0:10:22 | **cancelled** |
| ef27fc4 | 0:20:41 | success (cold cache) |
| #1284 (reported) | 10:02, 10:03 | **killed mid-cache-upload, every step passed** |

## Why the timeout is the wrong control

A budget that is shorter than the slowest legitimate run converts a slow download into a failing verdict,
and cuts the cache upload that would have made the next run fast — so the failure feeds itself. The header
already admits the intent ("sub-3 min target") and no run has met it; the number to encode is the measured
one plus margin, and the failure message should say "budget exceeded", not "your change broke".

Note: the choice of raising the budget rather than shrinking the build is deliberate — this issue exists to
stop a passing PR being red, and that is the smallest change that does it. If the intended direction is to
make preflight genuinely fast, this issue should be superseded rather than half-implemented.
