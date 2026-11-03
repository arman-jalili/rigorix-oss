---
guardian_issue:
  id: "ISSUE-SPIKE-VERDICT-CORRECTED-M2-OUTSTANDING"
  title: "The spike's branch C verdict is falsified: M3 holds on the shipped artifact, and the only outstanding measurement is M2's CLI half"
  epic: "portable-slice"
  component: "Docs"
  module: "wasm-seam"
  status: planned
  priority: high
  tracking_issue: 1354
  dependencies:
    - "the bundle-gate subject fix, which produces the shipped-artifact numbers this report must carry"
    - "#1351 (the report being corrected)"

  intent: |
    `docs/spike-portable-slice-2026-11.md` states a verdict — branch C, "the bundle does not fit" —
    and derives from it a product decision: the demo narrows to verify-only, with gate evaluation
    staying CLI-side. The verdict rests on 1 512 420 B measured against a 1 500 000 B ceiling, and
    that number is the *emitted* artifact, 341 785 B of which consists of a debug `name` section.

    Measured on the artifact a host loads, the bundle is 1 170 635 B — 78.0 % of the ceiling — and
    the module passes all fifteen smoke assertions. The branch table's own condition for firing is
    "the excess is not one trimable dependency". That condition was never established: the trim
    attempted was an LTO profile change, the named trim (`regex`) was never run, and the actual
    excess was a section that ships in no production artifact.

    So branch C did not fire, and the narrowed-demo decision was made on a falsified premise.
    Branch D is not earned either: it requires all four measurements, and M2 reached only its
    in-crate half — the CLI never produced a comparable envelope.

    The report is the artifact of record for this epic. Leaving a withdrawn verdict in it as the
    headline is how a later reader inherits "the engine cannot fit in the browser" as a fact.

  in_scope:
    - "The verdict line and the M1–M4 table carry the shipped-artifact numbers with the commands that produced them: M3 pass at 1 170 635 B / 1 500 000 B; M2 recorded as not reached, naming the CLI half as the reason"
    - "The branch table's C row records that it did not fire, and why — the excess was a debug section, not an untrimable dependency — and the regex trim is recorded as unrun and unnecessary on the shipped artifact"
    - "The next-steps section drops the narrowed-demo decision and names M2's CLI half as the next measurement"
    - "The M1b passage keeps both readings (strict: fails on the seam's 11 test edits; scoped to `portable`: zero edits) and states that the tests needed `pinned_timestamp: None` only because a public DTO gained a field — a carrier that the M2 issue replaces, which removes the edits and the caveat together"

  out_of_scope:
    - "Running M2's CLI half. That is its own issue, and this one only records it as outstanding"
    - "Editing the 84-row inventory. Its contents are unaffected"
    - "Any change to the apparatus branch, which stays linked and unmerged"

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
      note: ":8-10 the verdict paragraph; :180-232 the M3 section; :254 the branch table's C row; :259 the narrowed-demo next step"
    - file: "engine/.pi/portable-inventory.tsv"
      note: "the inventory this report ships with — unchanged"

  verification:
    - "The report's verdict paragraph, rendered, reads that M3 holds on the shipped artifact and names M2's CLI half as the sole outstanding measurement: `sed -n '1,12p' docs/spike-portable-slice-2026-11.md` shows 1 170 635 and no `does not fit`"
    - "`grep -c 'does not fit' docs/spike-portable-slice-2026-11.md` returns 0"
    - "Every number in the M1–M4 table has its command on the same page, and each reproduces when re-run from the merged tree"
    - "The M2 row reads `in part` with the reason, not `holds`: `grep -n 'M2' docs/spike-portable-slice-2026-11.md` shows the CLI half named"
    - "The C row of the branch table contains the measured 1 170 635 and the words for 'did not fire'"
---

# Correct the verdict: M3 holds, M2 is outstanding

## What the report says, and what is measured

| | report | measured on the shipped artifact |
|---|---|---|
| M3 engine-linked bundle | 1 512 420 B → "does not fit" (0.8 % over) | **1 170 635 B**; 78.0 % of the 1 500 000 ceiling |
| the 341 785 B removed | not examined | the `name` custom section — debug symbols |
| the "named trim" | `regex`, unrun | unrun and unnecessary; the excess was not a dependency |
| smoke gate | not run on the trimmed artifact | 15/15 pass, identical to the unstripped run |

`wc -c` on `wasm-bindgen` output counts a file a host never sees. That is the whole of the
correction: not a methodology failure — the report applied the issue's ceiling correctly to the
artifact it had — but a subject mismatch, and the spike's only job was to get the subject right.

## What the correction changes downstream

- **The narrowed demo is withdrawn.** Branch C's decided step ("the browser verifies, the gate stays
  server/CLI-side") was the answer to a size problem that does not exist on the shipped artifact.
  Gate evaluation in the browser remains on the table, contingent on M2.
- **M2 becomes the whole of the remaining question.** Byte identity between the CLI and the crate is
  what makes the browser gate mean anything, and it was reached on one side only. A demo built now
  would demonstrate a link that has never produced a comparable envelope.
- **M1b's caveat resolves by deletion rather than by argument.** The 11 test edits were plumbing for
  a public DTO's new field; removing the field removes the edits, and the strict reading then passes
  without needing the "scoped to `portable`" qualifier.

## What must not happen

The verdict must not be softened into "M3 is close enough". A gate is a comparison against a decided
number, and the number is now met with 22 % headroom on the artifact a host loads. The honest
statement is stronger than the report's: M3 passes, and it passed because the emitted file was
never the deliverable.
