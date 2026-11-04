---
guardian_issue:
  id: "ISSUE-ADR014-BARE-COUNTS-STALE-AND-UNCHECKED"
  title: "ADR-014 §1 now contradicts itself in one paragraph — '22 of 38 … Of those 25, only 11' next to '12 of 22' and '10 of 22' — because the guard checks ratio-shaped tokens and not bare counts"
  epic: "portable-slice"
  component: "Docs"
  module: "wasm-seam"
  status: planned
  priority: medium
  tracking_issue: 1377
  dependencies:
    - "#1374 (re-derived the counts; updated the tokens the guard sees)"
    - "#1363 / #1365 (the ratio and count guards)"

  intent: |
    The `--gates` check in #1374 found three false rows in the merged manifest (`dag_engine`,
    `event_system`, `observability` claimed "whole module" and gating them whole does not compile).
    The honest counts are **22 of 38** modules and **22 items**, 10 forbidden-crate + 6 narrowed
    `tokio` = 16 direct, 6 closure.

    The guarded tokens moved with them — the guard derives `22 of 38`, `12 of 22`, `10 of 22`,
    `22 items` and passes. Inside the same paragraph, two bare counts did not move:

    > Four counts are recorded here …: **74 crates** …, **22 of 38** public modules excluded, **22
    > inner items** …, and **9 dependencies** removed. **Of those 25, only 11 leave for a forbidden
    > crate**; 6 leave because they need a `tokio` feature the slice narrows away and 6 because they
    > name a symbol from an excluded module. So **12 of 22** leave for something that is *not* a
    > dependency statement, and any future "just remove `reqwest`" reasoning reaches only **10 of 22**.

    10 + 6 + 6 = 22 and 22 − 10 = 12, so the paragraph's own arithmetic is right and the two bare
    figures are wrong: there is no "25" and no "11". The ratios file carries a third copy of the
    stale pair in a provenance note — `10 of 22` is derived with the note "a description of the
    dependency list reaches **11 of the 25**".

    The guard cannot see any of them. It extracts `N of M`, `N %`, and vague quantifiers; a bare
    integer followed by a noun is not a shape it recognises, so #1374 re-derived exactly the tokens
    the guard reads and left the adjacent prose describing the previous counts.

    Verified, on a clean tree: `bash .pi/scripts/validate-portable-inventory.sh` prints
    `10 forbidden-crate + 6 narrowed-tokio = 16 direct, 6 closure · 22 of 38 modules · 22 items ·
    9 dependencies`, `22 derivation rows`, and exits 0 — with both stale numbers in the record it
    certifies.

  in_scope:
    - "ADR-014 §1's bare counts corrected: there is no 25 and no 11; the sentence reads 22 modules, of which 10 leave for a forbidden crate"
    - "The ratios file's `10 of 22` provenance note corrected to the same numbers"
    - "The guard extended to bare counts: any integer in the record immediately followed by one of the derived count nouns (modules, items, dependencies, crates) must equal the derived value for that noun, and a count noun with a subject the guard cannot derive is a failure with a reason — the same rule #1363 applied to ratios"
    - "Seeds: `Of those 25` → exit 1 naming the line and both values; `only 11 leave` → exit 1"

  out_of_scope:
    - "Re-deriving the counts again. 10 + 6 + 6 = 22 and 22 − 10 = 12 are confirmed against the manifest and the guard's own summary"
    - "The report's copy of the superseded count — filed separately, because it is the measurement record rather than the decision record"
    - "`74 crates`: measured again today on the merged tree (`cargo tree -p rigorix-engine --features portable --target wasm32-unknown-unknown -e normal,build` → 74 unique), so it is correct, not merely unchecked"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - ".pi/scripts/validate-portable-inventory.sh — the bare-count rule"
      - "wasm/.pi/adr-014-ratios.tsv — one provenance note"
    interfaces:
      - "None"

  citations:
    - file: "wasm/.pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md"
      note: "§1, the paragraph beginning 'Four counts are recorded here'"
    - file: "wasm/.pi/adr-014-ratios.tsv"
      note: "the `count 10 of 22 excluded_crate …` row's provenance cell"
    - file: ".pi/scripts/validate-portable-inventory.sh"
      note: "the ratio extraction; its summary line prints the derived counts"

  verification:
    - "`bash .pi/scripts/validate-portable-inventory.sh` exits 0 and the ADR paragraph contains no integer contradicting its summary line"
    - "Seed: replace `22 of 38 public modules excluded` with `22 modules excluded` in the prose → the guard exits 1, naming the line and the subject it cannot derive"
    - "Seed: set `Of those 25` → `Of those 21` → exit 1 (21 ≠ 22); restored, exit 0"
    - "Seed: set `only 11 leave` → `only 10 leave` → exit 0, because 10 is right; then `only 10` → `only 12` → exit 1"
    - "`--self-test` gains the bare-count cases and reports more than 14 assertions"
---

# The paragraph the guard protects, disagreeing with itself

```
ADR-014 §1:  **22 of 38** modules   …   Of those 25, only 11 leave for a forbidden crate   …
             6 … 6 …   So 12 of 22   …   reaches only 10 of 22
```

A reader gets four different module totals from one paragraph: 22, 25, 12-of-22, 10-of-22. Three of
them are consistent; the fourth — "those 25" and "only 11" — is the count from before #1374.

This is the same class as #1363 one level down. There, the guard read one of ten ratio statements;
here, it reads every ratio-shaped token and none of the bare ones. The fix is the same shape of
rule, not a bigger regex: **a count noun with a number the guard cannot derive is a failure**, so a
re-derivation either updates the prose or fails the build.
