---
guardian_issue:
  id: "ISSUE-REPORT-M4-WITNESS-AND-ROW-B-STALE"
  title: "The report's M4 witnesses are apparatus-era values that merged main does not produce, and branch-table row B still says the CLI half was not reached while row D fires"
  epic: "portable-slice"
  component: "Docs"
  module: "wasm-seam"
  status: planned
  priority: low
  tracking_issue: 1359
  dependencies:
    - "#1357 (fixed the M2 witness; this is the neighbouring row and the table's own consistency)"

  intent: |
    #1357 corrected the M2 witness. Two residues of the same class remain in the same document, one
    row apart:

    1. **M4's witnesses are apparatus-era.** `32211b9d…` twice "← equal", `4fe4487d…` vs
       `d1e9cf0d…` as the unpinned control, and the "884 B unpinned vs 877 B pinned" reading. On
       merged main, pinned is `c47b27f9…` at **853 B** — the same witness M2 now cites — and unpinned
       is **860 B** with a different digest on every run, because the wall clock is the variable M4
       exists to demonstrate. None of those three values reproduces.

    2. **Row B contradicts row D in the same table.** B reads "Fires in part… CLI: not reached", D
       reads "Fires — M2 holds". Both rows are present tense, four lines apart, and the table is the
       verdict of the epic.

    The prose fix in #1357 stopped at §2. The lesson in that commit — "an edit reporting success
    without verifying it landed" — applies to the edit that fixed it, one row down.

    There is a second, sharper point about unpinned witnesses: **a digest from an unpinned run is
    not reproducible by construction.** Citing one asks the reader to run a command that cannot
    agree, and the failure looks like the document's error rather than the measurement's design. An
    unpinned witness must be stated as "differs every run; 860 B", never as a digest.

  in_scope:
    - "M4's row and its detail cite merged-main evidence: pinned `c47b27f9…` 853 B twice-equal (the witness M2 already cites, one fact not two), and unpinned stated as 'differs each run; 860 B' with no digest claimed as checkable"
    - "Row B reads as historical: it fired at the time on the branch, and #1355 reached the CLI half, so B is superseded and D holds"
    - "§2's branch-era block keeps 884 B / 877 B / 4fe4487d / 32211b9d and is labelled as branch-era, stating that the fixture was renamed in the port (#1357) so those values are not reproducible on main — which is why the current-era witnesses are the ones the table carries"

  out_of_scope:
    - "Re-measuring anything. Every value needed was already measured; this is a provenance edit"
    - "Removing the apparatus-era numbers. They are the evidence that the fixture moved, and deleting them would leave the correction unexplained"
    - "The six occurrences of the withdrawn digest in docs/issues/m2-digest-is-the-apparatuses.md. That file is the filing record; docs/issues/** is outside validate-docs.sh's scope by design, and no per-file exemption exists or is needed"

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
      note: ":348-349 `32211b9d…` twice as the equal witness; :352 the unpinned control 4fe4487d/d1e9cf0d; :363 branch-table row B; :366 row D"
    - file: "engine/src/audit/application/pinned.rs"
      note: "the shipped fixture whose output is the current-era witness"

  verification:
    - "`cargo run -q -p rigorix-engine --example pinned_refusal -- --pinned-clock 2026-11-03T12:00:00Z --pinned-ids 018f5b8e-0000-7000-8000-000000000001` prints c47b27f9… and 853 B, and every current-era digest in the report is that one value"
    - "`cargo run -q -p rigorix-engine --example pinned_refusal` twice prints two different digests at 860 B — and the report claims no digest for the unpinned case"
    - "`grep -c 32211b9d docs/spike-portable-slice-2026-11.md` returns 2 (both inside the branch-era block), and neither sits in the M4 row or its detail"
    - "Row B's verdict cell contains 'superseded' or 'historical'; `sed -n '363p'` and `sed -n '366p'` do not both read present tense"
---

# The two residues #1357 left behind

| | report says | merged main produces |
|---|---|---|
| M4 pinned | `32211b9d…` ×2, 877 B | `c47b27f9…` ×2, **853 B** |
| M4 unpinned | `4fe4487d…` / `d1e9cf0d…`, 884 B | two different digests each run, **860 B** |
| Row B | "Fires in part… CLI: not reached" | superseded — M2 reached the CLI half (#1355) |
| Row D | "Fires" | consistent with B only if B is past tense |

## Why the unpinned case needs different wording, not a better number

An unpinned run reads the wall clock, so its digest changes by design. A document that prints one
invites a reader to reproduce it and fail, and the failure is indistinguishable from a broken claim.
The witness for "unpinned differs" is **two runs that disagree plus a byte count**, never a digest —
which is also why 884 vs 877 (branch) and 860 vs 853 (main) is the only part of that passage worth
reproducing.

## Why the branch-era block stays

884 B and 877 B are not wrong; they are the evidence that the operation's input moved between the
apparatus and `engine/src/audit/application/pinned.rs`, which is what #1357 diagnosed. Keeping them
labelled prevents the next reader from re-deriving that conclusion from scratch — and deleting them
would make the correction look like a number quietly swapped rather than a subject identified.
