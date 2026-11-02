# fix: #1281 — the conformance gate always prints its summary, and judges this repository's layout

Implements **#1281** (`ISSUE-CONFORMANCE-GATE-DIES-WITHOUT-A-SUMMARY`).

## What was wrong

One gate, three answers depending on where it was invoked:

| invocation | result |
|---|---|
| `bash .pi/scripts/ci/check_architecture_conformance.sh` (repo root) | `exit 1`, **no summary**, no FAIL line |
| `cd engine && bash ../.pi/scripts/ci/check_architecture_conformance.sh` | summary: 10 pass, **32 fail**, all `missing: interfaces` |
| `cd engine && bash .pi/scripts/ci/check_architecture_conformance.sh` (crate copy) | 10 pass, **0 fail** — the copy has no per-module layer block at all |

Four causes:

1. `module_dirs=$(find src …)` under `set -euo pipefail` with no `src/` at the root → the
   assignment fails and terminates the script before the summary.
2. `((violations++))` returns **1** when the pre-increment value is 0 → the *first* detected
   violation killed the run too. A gate that reports failure must still print its summary.
3. The required layer set was hardcoded to the four layers of
   `.pi/architecture/modules/module-template.md` — a React/Next.js template whose `interfaces/`
   row says *"UI components, pages, layouts"*. The engine's modules are three-layered by design,
   so 32 of 33 were "incomplete" against a template for another stack.
4. The six copies were **different files**, which is why one command gave three answers.

## What changes

| | before | after |
|---|---|---|
| no `src/` | `exit 1`, silent | `SKIP … nothing to scan`, summary, `exit 0` |
| a check aborts | run dies, no summary | failure line naming the check, summary |
| summary | only on the happy path | always — `print_summary()` behind an `EXIT` trap (exit code unchanged) |
| required layers | 4, from a React template | dominant signature among this crate's DDD modules; `ARCH_REQUIRED_LAYERS=…` pins it |
| boundary rule | none that ran | `domain/` may not reach outward, comments stripped |
| scan cost | 8.6 s per `find`, ×11 | 0.04 s (`-prune`, not `-not -path`) |
| copies | six drifted files | five byte-identical files, copy command in the header |

**Why the layer rule is derived.** A *DDD module* here is a module with a `domain/` layer; the
required set is the **dominant** signature among them. A module missing a layer its peers all
have is drift; an extra layer is not a violation. `cli` and `server` have no DDD module and say
so (`SKIP`), rather than inventing four layers to check.

**Why `application → interfaces` is not enforced.** In this repository `interfaces/` is the
incoming port as often as it is the outermost adapter, and application factories legitimately
wire it (`capacity_ledger/application/factory.rs` returns
`crate::capacity_ledger::interfaces::CapacityLedger`). Enforcing a rule the codebase does not
follow is how the layer check earned its 32 false failures; a gate may only be as strict as the
truth here. Comments are stripped before matching, so a rustdoc link such as
``[`CapacityLedger`]: crate::capacity_ledger::interfaces::CapacityLedger`` documents an edge
instead of becoming one.

## Found while verifying — the scan walked `target/`

`-not -path "*/target/*"` filters *results* but still **descends**. On this checkout that is
8.6 s per `find`, eleven per run — and `validate-architecture-readiness.sh` re-runs the script,
so the "gate" appeared to hang (the verification blew a 900 s timeout) instead of failing.
Replaced with `-prune` (`scan_code_files`): **0.04 s**. The same unbounded `find` in
`validate-architecture-readiness.sh` (runbook/DR lookups, and a 30-file scan) hung it at
"Checking runbook readiness…" once `target/` had grown from a day of `cargo test`; patched the
same way (`_find_pruned`). Without that, the acceptance criterion *"readiness exits 0"* cannot
be observed — only waited on.

## Files

- `.pi/scripts/ci/check_architecture_conformance.sh` — the fix (and the copy command in its header).
- `{actions,cli,engine,mcp}/.pi/scripts/ci/check_architecture_conformance.sh` — same bytes (md5-verified).
- `.pi/scripts/validate-architecture-readiness.sh` — pruned scan.

## Acceptance criteria

| # | Criterion | Status |
|---|-----------|--------|
| 1 | Missing `src/` → SKIP with summary, not an exit | ✅ root: `exit 0`, `⊘ SKIP … no src/ at this level` |
| 2 | Layer rule derived from this repository's layout | ✅ dominant signature among DDD modules |
| 3 | Summary prints in every outcome; failures name check + module | ✅ `EXIT` trap; seeded run shows `import_boundaries: Module 'approval': domain/…` |
| 4 | Readiness exits 0 on main; a seeded violation still fails | ✅ readiness 8/0 exit 0; seeded `domain → infrastructure` import → exit 1 |

## Verification

```
repo root                          exit 0   1.1s   Pass 8  Fail 0  Skip 7   (SKIP names why)
engine/   (root copy)              exit 0   2.4s   Pass 10 Fail 0  Skip 5
engine/   (crate copy)             exit 0   2.4s   Pass 10 Fail 0  Skip 5
mcp/ cli/ actions/                 exit 0   ~1.1s  Fail 0 (cli SKIPs: no DDD module)

seeded engine/src/approval/domain/tmp_boundary_violation.rs:
  ✗ FAIL import_boundaries — Module 'approval': domain/tmp_boundary_violation.rs
    references an outer layer (application|infrastructure|interfaces)
  exit=1   (file removed)

bash .pi/scripts/validate-architecture-readiness.sh   →  exit 0, Pass 8 Fail 0, 1.6s
md5 of the five copies: one unique hash
```
