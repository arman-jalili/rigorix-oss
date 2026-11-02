# Issue #1281 — the conformance gate dies without a summary, or reports 32 template failures

**Branch:** `issue/1281`
**Epic:** ci-integrity · **Component:** Validation · **Module:** architecture
**Priority:** medium · **Dependencies:** none

## Measured (before)

One gate, three answers depending on where it is invoked:

| invocation | result |
|---|---|
| `bash .pi/scripts/ci/check_architecture_conformance.sh` (repo root) | `exit 1`, **no summary**, no FAIL line |
| `cd engine && bash ../.pi/scripts/ci/check_architecture_conformance.sh` | summary: 10 pass, **32 fail**, all `missing: interfaces` |
| `cd engine && bash .pi/scripts/ci/check_architecture_conformance.sh` (crate copy) | 10 pass, **0 fail** — the copy has no per-module layer block at all |

Three root causes:
1. `module_dirs=$(find src …)` under `set -euo pipefail` with no `src/` at the root → the
   assignment fails and terminates the script before the summary (`check_architecture_conformance.sh:701`).
2. `((violations++))` returns 1 when the pre-increment value is 0 → the *first* detected violation
   killed the run too: a gate that reports failure must still print its summary.
3. The required layer set was hardcoded to the four layers of
   `.pi/architecture/modules/module-template.md` — a React/Next.js template (`interfaces/` =
   "UI components, pages, layouts"). The engine's modules are three-layered by design.

Plus drift: the six copies (root + `actions cli engine mcp server`) were *different files*, which is
why the same command gave three answers.

## What the fix does

1. **No `src/` → SKIP, summary, exit 0.** The scan says what it covered; "nothing to scan" is not a
   failure.
2. **The summary always prints.** It moved into `print_summary()` with an `EXIT` trap that prints it if
   the run ends early (exit code unchanged), and every check is invoked through `run_check`, which
   turns an abort into a failure line naming that check.
3. **The layer rule is derived from this repository.** A *DDD module* is one with a `domain/` layer; the
   required set is the **dominant** layer signature among the crate's DDD modules. A module missing a
   layer its peers all have is drift; an extra layer is not a violation.
   `ARCH_REQUIRED_LAYERS=domain,application,…` pins it when a repo wants that instead.
4. **The boundary rule is the one this codebase holds:** `domain/` may not reach outward. The template's
   *application may not import interfaces* rule is not enforced — `interfaces/` is the incoming port as
   often as the outer adapter here, and application factories wire it
   (`capacity_ledger/application/factory.rs`). Comments are stripped before matching, so a doc link like
   `[X]: crate::m::interfaces::X` is documentation, not a dependency.
5. **One file, six identical copies** (md5-verified), with the copy command in the header.

## Verification

| where | before | after |
|---|---|---|
| repo root | exit 1, no summary | exit 0, `SKIP … no src/`, summary |
| `engine/` via root copy | 32 fail | exit 0, 0 fail, summary |
| `engine/` via crate copy | 0 fail (block missing) | exit 0, 0 fail, module scan ran |
| `mcp/ cli/ actions/ server/` | — | exit 0 each (cli/server SKIP: no DDD module) |
| seeded `domain/ → infrastructure` import | not detected | `✗ FAIL import_boundaries — Module 'approval': domain/tmp_boundary_violation.rs …`, exit 1 |
| `validate-architecture-readiness.sh` | FAIL (conformance) | exit 0, 8/0 |

## Found while verifying: the scan walked `target/`

`-not -path "*/target/*"` filters results but still **descends** into the tree: 8.6 s per
`find` on this checkout, eleven of them, and `validate-architecture-readiness.sh` re-ran the
whole script — the gate "hung" for minutes (a 900 s tool timeout) rather than failing. Replaced
with `-prune` (`scan_code_files`): 0.04 s.

The same unbounded `find` in `validate-architecture-readiness.sh` (runbook/DR lookups, and a
30-file scan) made *it* hang at "Checking runbook readiness…" once `target/` had grown from a
day of `cargo test`. Patched the same way (`_find_pruned`), which is what lets the acceptance
criterion "readiness exits 0" actually be observed rather than waited on.

Timings after the fix: conformance 1.1 s from the root, 2.4 s from `engine/`; readiness 1.6 s.
