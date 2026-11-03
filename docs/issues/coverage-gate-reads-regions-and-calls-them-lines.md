---
guardian_issue:
  id: "ISSUE-COVERAGE-GATE-READS-REGIONS-AND-CALLS-THEM-LINES"
  title: "`coverage.sh --gate` extracts the first percentage of the TOTAL row — which is Regions — and prints it as line coverage; the per-layer thresholds it is compared to are ungated"
  epic: "ci-integrity"
  component: "CI"
  module: "coverage"
  status: planned
  priority: medium
  tracking_issue: 1348
  dependencies: []

  intent: |
    `cargo llvm-cov --summary-only` orders its TOTAL row as
    `Regions · Missed Regions · Cover · Functions · … · Lines · Missed Lines · Cover · Branches …`, and the gate takes
    the **first** percentage on the last line:

    ```bash
    COVERAGE="$(echo "$SUMMARY" | tail -1 | grep -oE '[0-9]+\.[0-9]+%' | head -1 | sed 's/%//')"
    …
    echo "Line coverage: ${COVERAGE}% (threshold: ${COVERAGE_THRESHOLD}%)"
    ```

    Measured on this crate's application slice:

    ```
    TOTAL  251  26  89.64%  30  8  73.33%  169  11  93.49%  0  0  -
                    ^^^^^^ Regions                        ^^^^^^ Lines
    script reports : "Line coverage: 89.64%"
    actual lines   : 93.49%
    ```

    So the number the gate enforces and the number it prints are **region** coverage, labelled line coverage. The
    repository's own rule for this file is that the verdict comes from the line metric, and `--fail-under-lines`
    exists in the tool precisely so that extraction is not hand-rolled.

    Two consequences:

    1. **The workspace gate's threshold is applied to a different metric than its message names** — every green
       coverage stage printed a number that was not the one it claimed to check.
    2. **The three per-layer thresholds in the roadmap are compared to nothing.** Domain ≥95 %, application ≥90 %,
       interface ≥85 % are documented; the only gate is workspace-wide at 60 %. Measured here (lines, the metric the
       criteria are written against):

       | Layer | Criterion | Measured (lines) | Measured (regions) | Gated |
       |---|---|---|---|---|
       | Domain | ≥95 % | 91.62 % (49 of 585) | 89.52 % (77 of 735) | no |
       | Application | ≥90 % | **93.49 %** (11 of 169) | 89.64 % (26 of 251) | no |
       | Interface | ≥85 % | 74.47 % (84 of 329) | 75.68 % (117 of 481) | no |

       Note the application row: on the criterion's metric it **meets** the threshold (93.49 % ≥ 90 %), and only
       fails if regions are used — which is what the audit and the gate both did.

  in_scope:
    - "The gate takes its number from the Lines column, or delegates the verdict to `cargo llvm-cov … --fail-under-lines` and uses its exit status; the message names the metric it enforced"
    - "A self-test: a TOTAL row whose Regions and Lines differ must yield the Lines value, and a fabricated row with the columns swapped must not"
    - "The three per-layer thresholds are either gated (a per-slice measurement in CI, with the filter regexes the audit already uses) or removed from the roadmap; a documented threshold no gate reads is a criterion nobody enforces"
    - "The audit's three numbers are relabelled to the metric they were measured on (they are regions), and the application row's verdict is corrected to met-on-lines / unmet-on-regions with the metric stated"

  out_of_scope:
    - "Raising coverage; the question is which number is measured and whether anything acts on it"
    - "The wasm crate's own thresholds, except as the first consumer of a per-layer gate"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "Modified: `.pi/scripts/coverage.sh` (extraction or delegation + self-test); `.github/workflows/ci.yml` if a per-layer gate is added"

  citations:
    - file: ".pi/scripts/coverage.sh"
      note: ":53-56 the TOTAL-row extraction; :69 the gate comparison"
    - file: ".github/workflows/ci.yml"
      note: ":174-175 'Workspace coverage (gated at 60%, baseline 78.92%)'"
    - file: "wasm/.pi/architecture/implementation-roadmap.md"
      note: ":188, :289, :341 the three per-layer thresholds and their measured values"

  verification:
    - "a TOTAL row with Regions 89.64 % and Lines 93.49 % yields 93.49"
    - "the printed message names Lines, and the gate's exit status matches `--fail-under-lines`"
    - "each per-layer threshold is either enforced by a CI job that fails below it, or absent from the roadmap"
---

# ISSUE-COVERAGE-GATE-READS-REGIONS-AND-CALLS-THEM-LINES

## Measured

```
TOTAL  251  26  89.64%  …  169  11  93.49%  0 0 -
script extracts "89.64%" and prints "Line coverage: 89.64%"
actual line coverage: 93.49%
```

## Why it matters twice

The label is wrong, and the mislabel propagated: the wasm audit reported its three coverage numbers from the same
column, so one criterion was marked unmet that meets its stated threshold (application, 93.49 % ≥ 90 %) and the other
two were described in a metric they were not measured in. A number is only a check when the reader can compare it to
the number it should be — and here the reader is comparing a region percentage with a line threshold, in a message that
says "Line coverage".

The second half is the governance gap: three documented thresholds, no gate that reads them. At 60 % workspace-wide,
the domain slice can fall from 91.62 % to 61 % and every build stays green.
