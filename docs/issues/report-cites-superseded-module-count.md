---
guardian_issue:
  id: "ISSUE-REPORT-CITES-SUPERSEDED-MODULE-COUNT"
  title: "The spike report still states '25 of 38 modules excluded, 11 of them for a forbidden crate'; the `--gates` check established 22 and 10, and the report's own convention is to annotate supersessions"
  epic: "portable-slice"
  component: "Docs"
  module: "wasm-seam"
  status: planned
  priority: low
  tracking_issue: 1378
  dependencies:
    - "#1374 (the `--gates` check that re-derived the counts)"
    - "the ADR-014 §1 correction (companion issue)"

  intent: |
    `docs/spike-portable-slice-2026-11.md:402` states the inventory finding it still stands behind:

    > 2. **M1's inventory stands** and is unaffected: **25 of 38** modules excluded, **11** of them for
    >    a forbidden crate.

    Both numbers are superseded. #1374's `--gates` check found that three rows claiming "whole module"
    (`dag_engine`, `event_system`, `observability`) do not compile when gated whole — they are item
    rows — so the honest counts are **22 of 38** modules and **10** for a forbidden crate. The
    manifest, ADR-014 and the ratios file all moved; the report did not.

    The report is the measurement record, and its convention — established by #1354, #1361 and
    #1357 — is that a superseded number stays visible **with the correction attached**, because the
    history of why a number moved is itself the evidence. Deleting the sentence would be wrong; it
    is still the finding M1 produced on the branch. Leaving it unannotated is also wrong: it reads as
    the current count, and it is the count the ADR no longer states.

    This matters more than a stale figure normally would, because the re-derivation is itself
    instructive: three manifest rows claiming a whole module were never gated whole until #1374's
    check tried it. That is a finding about the manifest's own claims, and the report is where a
    reader learns what the inventory cost.

  in_scope:
    - "The sentence keeps the branch-era numbers and gains the correction: 22 of 38 and 10, with the reason (three `whole module` rows were item rows, found by `--gates` in #1374) and a pointer to ADR-014 §1 and the manifest summary"
    - "The report's M1 row in the measurements table is checked for the same number and annotated if present"

  out_of_scope:
    - "Re-measuring M1. The branch figure was correct for the manifest as it existed; the correction is about what the manifest claims, not about the run"
    - "Editing the ADR or the ratios file (companion issue)"
    - "The 74-crate figure, re-measured today on the merged tree and still 74"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "None"
    interfaces:
      - "None"

  citations:
    - file: "docs/spike-portable-slice-2026-11.md"
      note: ":402 the M1 inventory bullet; §0-§1's table for the M1 row"
    - file: "engine/.pi/portable-inventory.tsv"
      note: "the summary that now reads 22 of 38; `dag_engine`, `event_system`, `observability` are item rows"
    - file: "wasm/.pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md"
      note: "§1, the re-derived counts"

  verification:
    - "Every stated count in the report adjacent to 'modules excluded' equals the manifest summary: `grep -n 'of 38' docs/spike-portable-slice-2026-11.md` shows 22 in any current claim, and 25 only inside an annotated supersession"
    - "The annotation names the three rows and the check that found them"
    - "A reader can reach ADR-014 §1 and the manifest from the sentence without searching"
---

# The measurement record, one manifest revision behind

The report is not wrong about what it measured. It is wrong about what the manifest says *now*, and
it says so in the section a reader goes to for the inventory's cost — which is the number every
later estimate inherits.
