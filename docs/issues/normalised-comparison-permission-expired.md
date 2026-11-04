---
guardian_issue:
  id: "ISSUE-NORMALISED-COMPARISON-PERMISSION-EXPIRED"
  title: "The 'normalised_comparison=true is permitted while FR-011 is unimplemented' escape hatch has no precondition left, and it is the one shape the metric was built to forbid"
  epic: "portable-slice"
  component: "Conformance"
  module: "wasm-seam"
  status: planned
  priority: medium
  tracking_issue: 1367
  dependencies:
    - "ADR-009 (the conformance harness and its labels)"
    - "#1355 (removed the condition the permission was written for)"

  intent: |
    `modules/portable-session.md` permits a normalised comparison under one condition:

    > `normalised_comparison=true` is permitted only while FR-011 is unimplemented and is labelled in
    > the report.

    FR-011 is no longer "unimplemented" — ADR-014 superseded it for the gate path, and #1355 supplied
    the CLI with pinned inputs, which was the entire reason the escape existed. A permission whose
    condition has been overtaken is worse than either state: it is neither granted nor withdrawn, so
    it stays available and unlabelled.

    The harness already knows why that matters. The report records the case that motivated the
    measurement: an unpinned envelope was 884 B and a pinned one 877 B for the same decision, because
    a whole-second instant serialises shorter. A comparator that normalises timestamps reports those
    as equal — which is a passing gate over two different byte strings, the exact failure this crate
    exists to make impossible. #1355's out-of-scope section says it plainly: "no normalised
    comparison", and "the comparison normalises nothing".

  in_scope:
    - "The permission is deleted from the module doc; criterion 22 states that no vector may carry `normalised_comparison: true` and that a vector which does fails the gate"
    - "`wasm/ci/run-conformance.sh` asserts the field is absent or false on every vector, naming the vector that sets it"

  out_of_scope:
    - "Adding CLI-side conformance vectors. That is Phase C, and it does not need this permission to exist"
    - "The rest of criterion 22 (a skipped or missing vector fails the run), which is correct and untouched"

  affected_layers:
    domain:
      - "None"
    application:
      - "Conformance vector validation"
    infrastructure:
      - "wasm/ci/run-conformance.sh"
    interfaces:
      - "None"

  citations:
    - file: "wasm/.pi/architecture/modules/portable-session.md"
      note: ":502 the note; :529 criterion 22's permission clause"
    - file: "wasm/ci/run-conformance.sh"
      note: ":28-31 the label check that already refuses an unlabelled golden"
    - file: "docs/spike-portable-slice-2026-11.md"
      note: "§2 — 884 B unpinned vs 877 B pinned: digest equality is not payload equality"

  verification:
    - "Seed: set `\"normalised_comparison\": true` in `wasm/tests/conformance/vectors/vec-0001.json` → `bash wasm/ci/run-conformance.sh` exits 1 naming that vector; restored, exit 0"
    - "`grep -c 'permitted only while' wasm/.pi/architecture/modules/portable-session.md` returns 0"
    - "`grep -rn 'normalised_comparison' wasm/` returns the criterion text only, with no permission"
---

# An escape hatch whose condition expired

The permission was reasonable when written: the CLI could not be given a clock, so a byte diff was
impossible and a label seemed better than a false failure. #1355 removed that impossibility. Keeping
the permission now means the harness can absorb a difference it was built to report — and the one
difference it would absorb is the one the oracle already observed (884 vs 877 B).

The rule worth stating in the module doc: a comparison that normalises anything is a comparison of
the thing normalised, not of the artifact.
