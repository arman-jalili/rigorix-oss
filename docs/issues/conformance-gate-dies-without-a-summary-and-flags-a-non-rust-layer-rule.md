---
guardian_issue:
  id: "ISSUE-CONFORMANCE-GATE-DIES-WITHOUT-A-SUMMARY"
  title: "check_architecture_conformance.sh either dies before its summary or reports 32 failures from a rule inherited from a React module template"
  epic: "ci-integrity"
  component: "Validation"
  module: "architecture"
  status: planned
  priority: medium
  tracking_issue: 1281
  dependencies: []

  intent: |
    Two modes, both wrong, and which one you see depends on the directory you run it from.

    **From the repository root**, where the hardening stages call it:

    ```
    $ bash .pi/scripts/ci/check_architecture_conformance.sh
    ✓ PASS Architecture sanity (no orphaned imports)
    ⊘ SKIP Architecture sanity — No concurrency-sensitive code in this slice
    ✓ PASS Architecture sanity (no settings/env collisions)
    exit=1                      ← no "═══ Conformance Summary ═══", no FAIL line
    ```

    Cause: `module_dirs=$(find src … | sort)` (line 701) under `set -euo pipefail`. There is no `src/` at the
    repository root, `find` exits 1, the pipeline fails, and the assignment terminates the script — before the
    summary and before any failure is named. `validate-architecture-readiness.sh` therefore reports
    "conformance failed" for a run in which nothing failed.

    **From the crate directory** (`engine/`), it reaches the summary and reports:

    ```
    ═══ Conformance Summary ═══
      Pass: 10
      Fail: 32
      Total: 47
    ```

    every one of the 32 the same rule:

    ```
    ✗ FAIL import_boundaries — Module 'approval' has incomplete DDD layers (missing: interfaces)
    … audit, budget_tracking, cancellation, code_gen, code_graph, …
    ```

    The rule (line 715) requires all four of `domain application infrastructure interfaces`. That layout comes
    from `module-template.md`, whose `interfaces/` row reads *"UI components, pages, layouts — React, Next.js"*.
    The engine's modules have three layers by design; `interfaces/` exists at the crate boundary, not once per
    module. So the check encodes a template this codebase does not use, and 32 of 37 modules are "incomplete".

    CI does not run it — `.github/workflows/ci.yml` names `run_preflight.sh` and `run_hardening_stages.sh` as
    deliberately excluded — which is why both modes survived: the gate is wired into the stages nobody runs,
    and it reports failure without naming a check.

  in_scope:
    - "A missing `src/` is reported as a SKIP with a summary, not an exit: the scan says what it covered, and `pipefail` does not turn 'nothing to scan' into 'failed'"
    - "The four-layer rule applies to modules whose architecture requires four layers; the engine's three-layer modules are conformant, and the check is derived from this repository's documented module layout rather than from the React/Next.js template"
    - "Every failure line names the check that produced it and the module (already true) and the summary prints in every outcome, including an empty scan"
    - "`validate-architecture-readiness.sh` exits 0 on main, and a seeded violation (a module with an import across a forbidden boundary) still fails"

  out_of_scope:
    - "Wiring the preflight/hardening stages into CI, which the workflow comments exclude on purpose"
    - "Adding `interfaces/` directories to 32 modules"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "Modified: `.pi/scripts/ci/check_architecture_conformance.sh` — the `find` guard, the layer rule, the summary path"

  citations:
    - file: ".pi/scripts/ci/check_architecture_conformance.sh"
      note: ":701 the `find src` assignment under pipefail; :713-724 the four-layer rule"
    - file: "engine/.pi/architecture/modules/module-template.md"
      note: ":22 the interfaces/ row that names React/Next.js"
    - file: ".github/workflows/ci.yml"
      note: ":81-82, :111-112, :141 comment naming run_preflight.sh and run_hardening_stages.sh as excluded"

  verification:
    - "from the repository root: exit 0 (or a SKIP-only summary that says there is no src/)"
    - "from engine/: exit 0 with the summary printed"
    - "seeded violation: a `domain/` file importing from `infrastructure/` in a module → FAIL, naming module and check"
---

# ISSUE-CONFORMANCE-GATE-DIES-WITHOUT-A-SUMMARY

## Two modes

| where | what happens |
|---|---|
| repository root | `exit 1`, no summary, no FAIL line — `set -e` + `pipefail` on `find src` |
| `engine/` | summary prints: 10 pass, **32 fail**, all `missing: interfaces` |

## Why both matter

The first mode is a gate that says "failed" and names nothing — the same defect as a validator that prints a
count instead of a location, and the reason `validate-architecture-readiness.sh` reports a conformance failure
on a tree where every check passed.

The second is a rule inherited from a template for a different stack: requiring `interfaces/` in every module
of a Rust crate whose modules are three-layered by design. A gate that is wrong 32 times out of 37 is a gate
that gets ignored, and the one real boundary violation would be the 33rd line in a wall of noise.
