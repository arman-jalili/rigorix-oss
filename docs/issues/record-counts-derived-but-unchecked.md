---
guardian_issue:
  id: "ISSUE-RECORD-COUNTS-DERIVED-BUT-UNCHECKED"
  title: "The guard derives '20 items' and '9 dependencies' from the manifest, prints them, and never compares them to the record: 21 items and 10 dependencies both pass"
  epic: "portable-slice"
  component: "Tooling"
  module: "wasm-seam"
  status: planned
  priority: low
  tracking_issue: 1365
  dependencies:
    - "#1363 (made ratios derived-or-fail; counts keep the older behaviour)"

  intent: |
    #1363 closed the ratio class: every `N of M`, every `N %`, and every vague quantifier in
    ADR-014 is now either derived or a failure. Verified — six seeds bite, including the one that
    used to pass (the bolded prose headline) and one moved to a line without its subject.

    Counts did not get the same treatment. The guard computes them and prints them on every run:

    ```
    derived: 11 forbidden-crate + 6 narrowed-tokio = 17 direct, 8 closure · 25 of 38 modules ·
             20 items · 9 dependencies
    ```

    `25 of 38` is checked. **`20 items` and `9 dependencies` are printed and not compared** to the
    record's copies — the numbers exist, in the same process, one line above the check that ignores
    them.

    ```
    SEED H  ADR:93 "20 inner items"     → "21 inner items"     → guard exit 0
    SEED I  ADR:92 "74 crates"          → "73 crates"          → guard exit 0
    SEED J  ADR:93 "11 forbidden crate" → "12 forbidden crate"  → exit 1  (the ratio's subject fragment pins the wording)
    ```

    Seed J is the near miss worth noting: the count inside a ratio row is protected only because the
    fragment that pins its subject happens to include it. Change the count and the subject stops
    matching, so the ratio fails — a side effect, not a check.

    `74 crates` is a different case and correctly marked `no, yet` in the evidence table: it needs
    `cargo tree --features portable`, which does not exist on `main`. The two manifest-derived counts
    do not have that excuse.

  in_scope:
    - "The counts the guard already derives — excluded modules (25 of 38), inner items (20), dependencies (9) — are required to appear in the record with those values, per site, the way ratios are"
    - "A count in the record that disagrees with its derivation fails, naming the line and both values"
    - "Seeds H and I are added to the self-test"

  out_of_scope:
    - "`74 crates` and the byte counts. They are branch-bound (`portable` does not exist on `main`) and the evidence table already marks them `no, yet`"
    - "The ratio machinery from #1363, which works and is not touched"
    - "Any attempt to verify a byte count against an artifact. There is no artifact to verify against until Phase C"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - ".pi/scripts/validate-portable-inventory.sh — the count comparison, alongside the ratio pass"
    interfaces:
      - "None"

  citations:
    - file: ".pi/scripts/validate-portable-inventory.sh"
      note: "the summary line printing '20 items · 9 dependencies'; the ratio pass it sits beside"
    - file: "wasm/.pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md"
      note: ":92 the crate-count row (`no, yet`); :93 '20 inner items' and '25 of 38 public modules'"

  verification:
    - "Seed H: change `20 inner items` to `21 inner items` → exit 1, naming line 93 and both values; restored, exit 0"
    - "Seed I: change `9 dependencies` wherever the record states it → exit 1"
    - "Seed J still fails, and for the right reason — the ratio's subject fragment, not the count check (the two checks must not be confused for each other)"
    - "`bash .pi/scripts/validate-portable-inventory.sh --self-test` reports 14 assertions (12 + H + I) and each new one is shown firing"
    - "The summary line's counts and the record's counts agree after the fix: a run prints `20 items` and the record states `20`, and the guard fails if either moves"
---

# The number is computed, printed, and ignored

```
  derived: … 25 of 38 modules · 20 items · 9 dependencies
  ratios: 20 derivation rows; every ratio-shaped token in the record is covered
  ✅ manifest, subjects, counts and the single citation agree
                                          ^^^^^^ "counts" — the module counts
```

The guard is now genuinely good at ratios: derived-or-fail, per-site subject matching, 12 self-test
assertions with the two seeds that matter (prose-right-table-wrong and table-right-prose-wrong). The
adjacent counts are the remaining unguarded class, and they are the cheapest to close because the
values are already in hand.

## Why this is worth a commit rather than a note

Phase C will move numbers in this record: the shipped bundle, the crate count, possibly the
dependency count. Every one of those edits is a chance to update a row and miss a copy. Ratios now
fail loudly; counts fail silently — and a silently wrong count in an ADR reads exactly like a right
one.
