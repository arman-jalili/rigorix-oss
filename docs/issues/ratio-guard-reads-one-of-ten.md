---
guardian_issue:
  id: "ISSUE-RATIO-GUARD-READS-ONE-OF-TEN"
  title: "The portable-inventory guard checks 1 of the record's 10 ratio statements — the Decision section's headline passes when wrong — and the one unchecked ratio that matters is already wrong"
  epic: "portable-slice"
  component: "Tooling"
  module: "wasm-seam"
  status: planned
  priority: high
  tracking_issue: 1363
  dependencies:
    - "#1361 (introduced the guard and the counts it certifies)"

  intent: |
    `.pi/scripts/validate-portable-inventory.sh:174` extracts **one** ratio from ADR-014:

    ```sh
    ratio="$(grep -oE '[0-9]+ of [0-9]+ public modules' "$adr" | head -1)"
    ```

    Two mechanisms narrow that to almost nothing:

    1. **Emphasis defeats the regex.** The Decision section states `**25 of 38** public modules
       excluded`; `**` between the numbers and the words means the pattern does not match, so the
       headline — the number a reader quotes — is invisible.
    2. **`head -1` checks one occurrence by construction.** The table row is the one that matches, so
       it is the only one verified. Every later statement is unchecked, not merely unparsed.

    Measured, on a clean tree, seeded and restored:

    | seed | what changed | guard |
    |---|---|---|
    | A | line 46 prose `**25 of 38**` → `**22 of 40**` | **exit 0** — prints `record's ratio: 25 of 38` (read from the table) |
    | B | line 93 table `25 of 38 public modules` → `22 of 40` | exit 1 — names both values |
    | C | a 4th module named in the record | exit 1 — `names 5 excluded modules (limit 3)` |

    The record contains **ten** ratio-shaped statements: `25 %`, `78.0 %` ×3, `100.8 %`, `22.6 %`,
    `26.5 %`, `25 of 38` ×2, `11 of 25`. The guard sees **one**.

    And the first one I checked by hand is wrong. Row 9 reads:

    > `data` = 400 735 B = 26.5 % **of the shipped engine-linked module**

    Measured on the artifact: the data section is 400 735 B of the **1 512 420 B emitted** module
    (26.5 %) and of the **1 170 635 B shipped** module it is **34.2 %**. The cell names the wrong
    denominator — 26.5 % is the emitted share, and the shipped module is what the sentence says.

    That number reached the record from my own review note, which said "`data` is 400 735 B, a
    quarter of the module" without naming which module. The ADR supplied the denominator. This is
    why a percentage without a named subject is not a fact.

  in_scope:
    - "Every ratio-shaped statement in the record is checked, and **all** occurrences, not the first: markdown emphasis is stripped before matching"
    - "Percentages are verified against the byte counts stated in the same cell, so a percentage cannot drift from its denominator; module ratios are verified against the manifest"
    - "A ratio-shaped statement the guard cannot derive is a **failure with a named reason**, not a skip: an unreadable claim is an unchecked claim"
    - "Row 9's percentage names its denominator correctly — 34.2 % of the shipped 1 170 635 B module — and any emitted-share figure appears with its own denominator"
    - "Seed A is added to the self-test: prose ratio wrong, table ratio right, guard must exit 1 naming the prose site by line"

  out_of_scope:
    - "Re-measuring the artifact. Every byte count in the record reproduces: emitted 1 512 420, shipped 1 170 635, name 341 785, data 400 735"
    - "The 'at most 3 named modules' rule, which works and was seeded (exit 1, counted 5 named)"
    - "The 74-crate count, the 20-items count and the 9-dependency count — they are counts of lists the guard already recomputes"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - ".pi/scripts/validate-portable-inventory.sh — the ratio extraction and the failure rule"
      - "wasm/.pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md — row 9's denominator"
    interfaces:
      - "None"

  citations:
    - file: ".pi/scripts/validate-portable-inventory.sh"
      note: ":174 `grep -oE … | head -1`; :175-181 the compare; :203 the self-test block"
    - file: "wasm/.pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md"
      note: ":46 the bolded prose ratio; :49 `11 of 25`; :90 `78.0 %` and `100.8 %`; :91 `22.6 %`; :93 the table ratio; :97 the data row with the wrong denominator"

  verification:
    - "Seed A: change `**25 of 38**` to `**22 of 40**` on ADR line 46 → guard exits 1 naming the prose line; restored, exit 0"
    - "Seed B: change the table row to `22 of 40` → 1, with both values named — unchanged behaviour"
    - "Seeded wrong denominator: change row 9 to `26.5 % of the shipped` → guard exits 1 stating the recomputed 34.2 % from the cell's own bytes"
    - "A ratio-shaped statement the guard cannot derive — e.g. `half the excluded modules` — fails with a reason naming it, rather than passing silently"
    - "`bash .pi/scripts/validate-portable-inventory.sh --self-test` reports more than 8 assertions and includes seed A"
---

# One ratio checked, ten stated, one wrong

```
$ grep -oE '[0-9]+ of [0-9]+ public modules' ADR-014-… | head -1
25 of 38 public modules          ← the table
$ grep -n '\*\*[0-9]* of [0-9]*\*\*' ADR-014-…
46: … and **25 of 38** public modules excluded …    ← the headline, invisible to the pattern
```

The guard's good properties are real and were verified: it recomputes the module count from the
manifest, it fails when the summary disagrees with `lib.rs`, it fails when the record restates the
exclusion list, and it fails when the record names more than three modules. Eight self-test
assertions, each shown to fire.

What it does not do is read the rest of the record's arithmetic — and the first unchecked number
turned out to be wrong by 7.7 points, with the right byte counts sitting in the cell next to it.

## Why "checked once" is the dangerous shape

A guard that verifies one occurrence teaches a reader that the guard verifies the statement. The
next editor who changes line 46 gets a green run and concludes the record is consistent. This is the
same defect as a size gate weighing a debug file, one layer up: the check is real, the number is
real, and the subject is not the claim.

The fix that closes this class is the failure rule, not the extra pattern: **anything that looks
like a ratio and cannot be derived is a failure.** A guard that silently skips what it does not
understand will always be narrower than the document it certifies.
