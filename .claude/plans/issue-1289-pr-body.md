Implements **#1289** (`ISSUE-FINAL-GATE-ACCUSES-A-COMMIT-FOR-A-CANCELLED-RUN`).

## Measured

On `b7414228` (the merge commit of #1287), all seven stages `cancelled`, and the only real verdict on the commit was the gate accusing it:

```
🔒 Audit: cancelled            📝 Documentation: cancelled
🔗 Integration: cancelled      📦 Build: cancelled
🧪 Test (matrix): cancelled    📐 SDK-schema conformance: cancelled
🚪 Final Gate: failure         ← a verdict on a run that never happened
```

The successor run on `54da9edf` is green. `cancel-in-progress` makes this the *common* case on this repo, not the exception.

## Changed

| # | Before | After |
|---|---|---|
| 1 | failing step fires on `!= 'success'`, which `cancelled` satisfies | fires for `failure` and (policy unchanged) `skipped` only |
| 2 | superseded run's gate = `failure` | gate job **skipped**: `always() && !contains(needs.*.result, 'cancelled')` — a grey status, not a verdict (green would be a verdict too: the run never evaluated the change) + a `::notice` naming the cancelled stages |
| 3 | results table lived only in the gate, so skipping it would lose the evidence | new `📋 Stage report` job runs `always()`, never fails, prints every stage — a reader still sees `cancelled` per stage |
| 4 | the verdict was five YAML expressions | one script, `.github/scripts/ci_verdict.sh`, which classifies and names the reason; the table renderer lives in `ci_stages.sh`, shared by both jobs so they cannot drift |

## Seeds

Local (script-level, no CI needed):

```
all success                   → exit 0  ✅ All mandatory stages passed: lint build test audit conformance
test failure                  → exit 1  ❌ Mandatory stage(s) did not pass: test
all cancelled                 → exit 0  ⏹️ No verdict: the run was superseded (cancelled: …) + ::notice
lint ok, rest cancelled       → exit 0  ⏹️ No verdict (a partly-finished run still has none)
mandatory skipped             → exit 1  ❌ Mandatory stage(s) did not pass: lint
no wiring (empty envs)        → exit 1  ❌ (a missing stage is not a pass)
docs cancelled, mandatory ok  → exit 0  ✅ (docs is reported, not required)
```

Live seed: this branch carries empty commits pushed one at a time so the previous run is superseded. Measured on the superseded commit `a7cbcee8` (`gh api …/commits/a7cbcee8/check-runs`):

```
📋 Stage report:      success      ← the new job, printing the evidence the gate can no longer print
🚪 Final Gate:        cancelled    ← not `failure`; no failing check on the commit
🔎 Lint (×5), 📦 Build, 🧪 Test, 🔒 Audit, 📐 Conformance, 🔗 Integration, 📝 Docs: cancelled
```

and the report job's own log on that cancelled run:

```
| Stage       | Result        | Mandatory |
|-------------|---------------|-----------|
| lint        | ⏹️ cancelled | yes       |
| build       | ⏹️ cancelled | yes       |
| test        | ⏹️ cancelled | yes       |
| audit       | ⏹️ cancelled | yes       |
| conformance | ⏹️ cancelled | yes       |
| docs        | ⏹️ cancelled | no        |
| integration | ⏹️ cancelled | no        |
```

(The gate lands on `cancelled` rather than `skipped` when the run is torn down mid-flight; both are "no verdict" and neither is a `failure`. The `skipped` path is the job-level `if`, and the discriminating proof that `cancelled` no longer produces a verdict is the script seed above — the old condition, `!= 'success'`, turns that exact table into `exit 1`.)

Empty commits change no tree content; the squash merge collapses them.
