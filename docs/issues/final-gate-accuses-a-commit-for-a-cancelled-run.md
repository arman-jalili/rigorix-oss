---
guardian_issue:
  id: "ISSUE-FINAL-GATE-ACCUSES-A-COMMIT-FOR-A-CANCELLED-RUN"
  title: "a superseded run's Final Gate reports failure, so a merge commit reads as red when its successor run is green"
  epic: "ci-integrity"
  component: "CI"
  module: "ci-workflow"
  status: planned
  priority: medium
  tracking_issue: 1289
  dependencies: []

  intent: |
    `ci.yml` cancels superseded runs on the same ref:

    ```yaml
    concurrency:
      group: ${{ github.workflow }}-${{ github.ref }}
      cancel-in-progress: true
    ```

    and its `Final Gate` runs `if: always()`, requiring `success` from five stages:

    ```yaml
    - name: ✅ All mandatory stages passed
      if: needs.lint.result == 'success' && … 5 stages … && needs.conformance.result == 'success'
    - name: ❌ Some mandatory stages failed
      if: needs.lint.result != 'success' || … 5 stages … || needs.conformance.result != 'success'
      run: |
        echo "❌ Mandatory stage(s) failed"
        exit 1
    ```

    A cancelled stage is `!= 'success'`, so the gate takes the failing branch and **accuses the commit**.
    Measured on the merge commit of #1287 (`b7414228`), by `gh api repos/…/commits/b7414228/check-runs`:

    ```
    🔒 Audit: cancelled                    📝 Documentation: cancelled
    🔗 Integration: cancelled              📦 Build: cancelled
    🧪 Test (${{ matrix.crate }}): cancelled   📐 SDK-schema conformance: cancelled
    🚪 Final Gate: failure                 ← a verdict on a run that never happened
    ```

    38 check runs on that commit, every mandatory stage `cancelled`, and the only real verdict is the
    gate's failure. The successor run on `54da9edf` is green, so the tree is fine — but *the commit* has a
    failing check on it, and the two are indistinguishable from a real red unless you read the job list.

    The same happened on `007cc2e`, `814d2b6` and `bebad0c` in the last two batches: each merge commit
    shows `CI cancelled`, and checking whether a batch landed green has required looking at a *later* commit
    that happens to contain the same tree.

  in_scope:
    - "The gate's failing step fires for `failure` (and for a `skipped` mandatory stage, if that stays the policy) and not for `cancelled`"
    - "A cancelled or superseded run reports `neutral` with a line naming the reason — a status, not a verdict"
    - "The results table keeps printing every stage's result as it does now, so a reader sees `cancelled` rather than a bare red gate"
    - "Seeded: a run cancelled mid-flight reports `neutral`/skipped for the gate and no failing check on the commit; a genuine `failure` in one stage still fails the gate"

  out_of_scope:
    - "Removing `cancel-in-progress`, which is what keeps a push from queueing behind a stale run"
    - "Making `docs` and `integration` mandatory: the gate prints their results but does not require them, and whether they should is a separate decision"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "Modified: `.github/workflows/ci.yml` — the `gate` job's conditions"

  citations:
    - file: ".github/workflows/ci.yml"
      note: ":36-38 concurrency with cancel-in-progress; :330-364 the gate job"

  verification:
    - "a cancelled run's `Final Gate` conclusion is not `failure`"
    - "a run with one genuinely failing stage still reports `failure`"
---

# ISSUE-FINAL-GATE-ACCUSES-A-COMMIT-FOR-A-CANCELLED-RUN

## Measured

```
commit b7414228 — 38 check runs
  lint / build / test / audit / docs / integration / conformance : cancelled
  🚪 Final Gate                                                  : failure
successor run on 54da9edf                                       : success
```

## Why it is worth fixing

`cancelled` is not a property of the change; it is a property of the scheduler. Converting it into a failing
gate means the only status a merge commit reliably carries is "a run started and was superseded", and every
reader — including an automated one asking `check-runs` — has to reconstruct that from the job list. On a
repository where every merge triggers a push to `main` *and* the merge event, the superseded run is the
common case, not the exception.

It is the same rule the rest of this work keeps applying: a check that could not run must say it could not
run, and must not report as the thing it never evaluated.
