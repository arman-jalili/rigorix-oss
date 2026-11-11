---
guardian_issue:
  id: "ISSUE-ARITHMETIC-COUNTERS-END-SCRIPTS-SILENTLY-ON-LINUX"
  title: "167 arithmetic counters end their scripts silently on bash 4.4+, and every measurement of them so far was taken on macOS bash 3.2"
  epic: "ci-integrity"
  component: "Validation"
  module: "tooling"
  status: planned
  priority: high
  tracking_issue: 1291
  dependencies:
    - "the enterprise repository #492 — the same fix and the reference guard, already merged there"

  intent: |
    `(( X++ ))` returns status 1 when the result is 0, so the **first** increment of a counter that
    starts at 0 fails. Whether that ends the script depends on the shell, and this repository is developed
    on the one shell where it does not:

    ```
    set -e; x=0; ((x++)); echo survived
      macOS /bin/bash 3.2.57      → exit 0, "survived"
      bash 4.4.23                 → exit 1, no output
      bash 5.3.20                 → exit 1, no output
    ```

    Measured on a real script in this tree, same input, both shells:

    ```
    mcp/.pi/scripts/ci/validate_agent_output.sh --input /tmp/agent-out.md
      bash 3.2 (macOS)   → exit 1, 21 lines, "❌ Agent output validation failed (3 check(s))"
      bash 5   (Linux)   → exit 1,  0 lines, no output at all
    ```

    So the script does not "fail differently" on Linux — it dies at the first passing check and prints
    nothing, which is indistinguishable from a clean run that had nothing to say.

    Scan of this repository (`.pi/scripts` and the per-crate copies):

    ```
    shell scripts (excluding target/, node_modules/)     518
    standalone arithmetic statements under errexit        167
    distinct files (by content)                            20
    file instances across the five copies                  39
    ```

    The most exposed: `validate_agent_output.sh` (11 sites, 5 copies), `run_hardening_stages.sh` (7, 5
    copies), `run_preflight.sh` (6, 5 copies), `check_architecture_conformance.sh` (3, 5 copies),
    `stage_static_analysis.sh`, `stage_docs_policy.sh`, `stage_remaining.sh`,
    `languages/python/validate-canonical.sh`.

    CI does not run these (`ci.yml` names them as deliberately excluded), which is why the tree has been
    green: they are invoked by agents and by hand, on a Mac, where the increment is harmless.

  in_scope:
    - "Every standalone `(( X++ ))` / `(( X-- ))` in a script that enables `errexit` becomes the assignment form `X=$((X + 1))`, which always returns 0 and does not swallow a real failure"
    - "A guard `validate-shell-exit-status.sh` with a self-test first (positive, negative, and last-line cases), failing on `path:line` for a standalone arithmetic command in an errexit script, and clean on `&&`, `||`, `if`, `while`, assignment form, and `set +e` files"
    - "The guard runs in CI beside the shellcheck step, named so a reader can tell which shell check failed, and against every copy of the scripts that CI-scoped checks live in"
    - "The five copies of a shared script stay byte-identical, so a fix cannot land in one copy and miss four"
    - "Seeded: restoring `((PASSED++))` in `run_preflight.sh` fails the guard naming file and line; the current tree passes"

  out_of_scope:
    - "`(( X++ )) || true`, which suppresses the exit status by suppressing all failures — the enterprise fix rejected it for the same reason"
    - "Rewriting the 32 sites in scripts that do not enable `errexit`; measured, no script in the tree ends in an arithmetic statement, so they cannot change an exit status today"
    - "The `$((X + 1))`-on-unset-variable difference, which is also shell-specific: all counters in scope are initialised"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "Modified: 20 script files (39 instances), one new guard script, one CI step"

  citations:
    - file: "mcp/.pi/scripts/ci/validate_agent_output.sh"
      note: ":51, :57, :76, :83, :89 — the measured silent death, 11 sites"
    - file: ".pi/scripts/ci/run_preflight.sh"
      note: ":71-72 the first increments, reached before any verdict is printed"
    - file: "the enterprise repository's .pi/scripts/validate-shell-exit-status.sh"
      note: "the reference guard and its 12-case self-test, merged as enterprise#492"

  verification:
    - "`bash .pi/scripts/validate-shell-exit-status.sh` exits 0 on the tree, self-test green"
    - "a `(( X++ ))` restored in `run_preflight.sh` fails it, naming file and line"
    - "`validate_agent_output.sh --input <file>` prints the same 21-line verdict on bash 3.2 and bash 5"
---

# ISSUE-ARITHMETIC-COUNTERS-END-SCRIPTS-SILENTLY-ON-LINUX

## Measured

```
bash 3.2.57 (macOS) : set -e; x=0; ((x++)); echo survived   → exit 0
bash 4.4.23 / 5.3.20: same                                   → exit 1

mcp/.pi/scripts/ci/validate_agent_output.sh, same input:
  bash 3.2 → 21 lines, "❌ Agent output validation failed (3 check(s))"
  bash 5   → 0 lines, exit 1
```

## Why this was missed, twice

Every measurement of these scripts in this repository has been taken on macOS bash 3.2 — including mine.
The exemption is a bash 3.2 behaviour; bash 4.4 and later do propagate a failing arithmetic command through
`errexit`. A claim about shell semantics is a claim about a specific shell, and the one that runs in CI is
not the one that runs on the development machine.

The reference implementation exists and is merged: `the enterprise repository #492` fixed 39 sites the same way and
added `.pi/scripts/validate-shell-exit-status.sh` (12/12 self-test, five positive cases, five negative), with
a seeded mutation that names file and line. This issue is that fix applied to this tree, where the site count
is 167 and the copies multiply it.
