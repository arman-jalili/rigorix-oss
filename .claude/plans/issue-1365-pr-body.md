The guard derived three counts, printed them on every run, and compared one of them:

```
  derived: 11 forbidden-crate + 6 narrowed-tokio = 17 direct, 8 closure · 25 of 38 modules · 20 items · 9 dependencies
  ✅ manifest, subjects, counts and the single citation agree
                                     ^^^^^^ modules only
```

`21 inner items` and `10 dependencies` both passed. The values were in the same process, one line above the check that ignored them.

## Counts are rows now

A `total` kind — numerator is a derived quantity, the stated form keeps the record's own noun:

```
total   20 inner items   items   -   20 inner items   evidence table (#1365)
total   9 dependencies  deps    -   9 dependencies   Decision §1 (#1365)
```

The guard checks the arithmetic against the derived value **and** requires the fragment that pins the site. Bare `N items` / `N dependencies` occurrences joined the ratio-shaped extraction, so an uncovered count fails with a line number rather than passing quietly.

## Seeds

```
SEED H  ADR "20 inner items" → "21 inner items"   → exit 1
        '20 inner items' — the record no longer contains its subject fragment …
        :46 and :94 state '21 inner items' with no derivation covering that site

SEED I  ADR "9 dependencies" → "10 dependencies"  → exit 1
        '9 dependencies' — the record no longer contains its subject fragment …
        :47 states '10 dependencies' with no derivation covering that site
```

`--self-test`: **14/14**.

The record had no dependency count at all before this — the manifest's 9 rows were summarised only as "6 forbidden crates + 3 crates with no wasm support + 1 narrowed feature set". Decision §1 now states **four** counts: 74 crates, 25 of 38 modules, 20 inner items, 9 dependencies.

## What stayed unchecked, and why it is said out loud

`74 crates` needs `cargo tree --features portable`, and `portable` does not exist on `main` until Phase C. The byte counts are the same case. The evidence table marks them **`no, yet`**, which is a claim about the artifact's absence — not a skipped check. Phase C is what converts them, and the derivation rows for those numbers already exist, so the conversion is an edit to a value whose subject is already written down.

## Validation

```
validate-portable-inventory.sh              exit 0   (22 derivation rows)
validate-portable-inventory.sh --self-test  14/14
docs / guards / canonical / tests           all exit 0
```

Closes #1365
