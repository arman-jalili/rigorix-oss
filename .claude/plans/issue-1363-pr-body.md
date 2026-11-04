Two mechanisms in one line, and the number they hid was wrong.

```sh
ratio="$(grep -oE '[0-9]+ of [0-9]+ public modules' "$adr" | head -1)"
```

`**25 of 38**` did not match, so the Decision section's headline was never checked; and `head -1` meant exactly **one** of the record's ten ratio-shaped statements was ever read. The first one I checked by hand was wrong:

```
data section 400 735 B · emitted 1 512 420 B · shipped 1 170 635 B
        as % of emitted: 26.5 %        as % of shipped: 34.2 %
ADR row 9: "`data` = 400 735 B = 26.5 % of the shipped engine-linked module"
```

26.5 % is the emitted share in a cell that says **shipped** — off by 7.7 points, in the row whose own bytes make the error visible.

## Ratios are derived now, not matched

`wasm/.pi/adr-014-ratios.tsv` gives each ratio its operands, its unit, and the **exact fragment** of the record that pins its subject:

```
kind    stated    numerator        denominator      says
share   34.2 %    data_emitted     engine_shipped   34.2 % of the shipped 1 170 635 B module
share   26.5 %    data_emitted     engine_emitted   26.5 % of the emitted engine-linked module
count   25 of 38  mods             declared         25 of 38 public modules excluded
```

The guard then (a) recomputes the arithmetic — `share`, `growth`, exact `count` — (b) requires the fragment to be **present**, and (c) requires the token to sit **inside** it. Coverage is per site: the token on line N must be covered by a fragment on line N, so a percentage cannot borrow an unrelated "of the …" clause from the same line.

## The closing rule

> Anything that looks like a ratio, and cannot be derived, is a failure with a named reason — never a skip.

That is the assertion that catches the *next* shape, not just this one. Emphasis is stripped before matching (`**25 of 38**` → `25 of 38`), and vague quantifiers count as ratio-shaped. It immediately caught one I had written and not noticed:

```
ADR-014:131 states 'half', which has no derivation row
```

— "Half of the slice's module exclusions follow from other exclusions", which with the corrected counts is not half at all: **8 of 25** follow from another exclusion being named, and 6 more from a dropped `tokio` feature. Now counted, with a row.

## The record

- Row 9: **34.2 % of the shipped 1 170 635 B module**, and 26.5 % of the emitted one — each percentage with its own denominator, in the same cell.
- Three vague quantifiers became derived counts: 14 of 25, 11 of 25, 8 of 25.
- Two percentages without a derivable subject were reworded until they had one (`+25 % over the measured 209 881 B`; `100.8 % of the ceiling`).

## Seeds

```
SEED A  prose `**25 of 38**` → `**22 of 40**`, table untouched   → exit 1, names the prose site
SEED B  table `25 of 38` → `22 of 40`, prose untouched           → exit 1, names both values
SEED C  `34.2 %` → `26.5 %` anywhere                             → exit 1:
          '34.2 %' — the record no longer contains its subject fragment …
          :136 states '26.5 %' with no derivation covering that site
SEED D  append "Half the excluded modules are not dependencies." → exit 1, names the line
```

`--self-test`: **12/12**.

## Provenance of the wrong number

It came from the review note, not from the guard: "data is 400,735 B, a quarter of the module" — with no denominator named, and the ADR supplied one. A percentage without a named subject is not a fact; that applies to the prose that feeds a record as much as to the record. Hence the manifest column that now makes the subject mandatory.

## Validation

```
validate-portable-inventory.sh             exit 0   (20 derivation rows)
validate-portable-inventory.sh --self-test 12/12
docs / guards / architecture / canonical / tests / wiring-claims / script-drift / shell-exit-status / ci   all exit 0
```

Closes #1363
