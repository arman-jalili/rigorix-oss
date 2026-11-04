ADR-014 (branch D's decision record), plus the counting error that writing it surfaced.

## The ADR

Three parts, and the third is what makes the first two auditable rather than decorative.

**1. `portable`'s exclusions are a manifest.** `engine/.pi/portable-inventory.tsv` — one row per excluded dependency, module and inner item. The ADR **cites** it and does not restate it: a second copy is a second authority, and two authorities drift. What it does state is the counts, because those are the decision: **74 crates** in the `wasm32` graph, **25 of 38** public modules.

**2. Determinism is one operation and a carrier, not a clock abstraction.** `audit::application::pinned::refusal_envelope_input` is called by both runtimes, and the factory takes the pins through an additive `with_pinned_inputs`. `PinnedInputs` is `#[non_exhaustive]` from its first commit. **No `Clock`/`IdSource` trait pair on the gate path** — a trait with one implementor is paid for by every future caller; those ports belong to `portable-session`'s domain when a second consumer exists. The roadmap line promising them is annotated as superseded, so Phase 0 does not build what D showed is unnecessary.

**3. Every number states its artifact and whether it reproduces.**

| Fact | Value | Reproducible |
|---|---|---|
| Empty-seam module, shipped | 209 881 B (budget 262 351 B) | **yes** — `check-bundle-size.sh`, presence assertion first |
| Engine-linked module, shipped | 1 170 635 B = 78.0 % of 1 500 000 B | **no, yet** — `portable` + wiring is Phase C |
| `name` section stripped | 341 785 B = 22.6 % of the emitted module | **no, yet** — same |
| `wasm32` graph under `portable` | 74 crates, exit 0 | **no, yet** — feature not on `main` |
| Modules excluded | 25 of 38 (11 crate, 6 narrowed `tokio`, 8 closure) | **yes** — guarded |
| M2 witness | `c47b27f9…`, 853 B, twice equal | **yes** — the golden test |
| Unpinned control | differs every run, 860 B | the variance *is* the result |
| Apparatus-era | 877 / 884 B, `32211b9d…` | **no** — renamed in the port; history |
| Remaining size lever | `data` = 400 735 B = 26.5 % | **no, yet** |

Nine rows, three verdicts, and the un-reproducible ones are **marked rather than omitted** — that is what makes "about 1.17 MB" impossible to write here.

## What writing it surfaced: #1361

Stating the counts meant counting them, and the manifest's own summary disagreed with its own rows:

```
$ awk -F'\t' '$1=="module"{ if ($4 ~ /tree_sitter|reqwest|prometheus/) d++;
                              else if ($4 ~ /tokio/) t++; else c++ } END {print d, t, c, d+t+c}' \
    engine/.pi/portable-inventory.tsv
11 6 8 25
```

The summary said **22 of 40 (11 direct, 11 indirect)**. The rows held **25 of 38**: 11 for a forbidden crate, **6 for the narrowed `tokio` feature set**, 8 by closure. The "indirect" bucket had been *estimated* — and it swallowed the six modules the manifest's own header counts separately as "+1 narrowed feature set". The denominator was never 40 either: `grep -cE '^pub mod' engine/src/lib.rs` is 38 on `c118cb06`, on `24e6dd6f` and on `main`.

So the headline ratio was **55 % instead of 66 %** of public modules excluded — understated, in the direction that flatters the decision.

Corrected in the manifest, in the report's three sites (which now name all three triggers in a table), in the roadmap and here.

## The guard, so it cannot drift again

`.pi/scripts/validate-portable-inventory.sh` — registered gating in `.pi/guards.tsv`:

- **counts** recomputed from the rows and compared with what the summary and the ADR state;
- **subjects** — every `module` must be a real `pub mod` in `lib.rs`, every `item` path must resolve;
- **structure** — 6 columns, known layer, numeric sites;
- **one authority** — the ADR must cite the manifest and must not restate the list (≤3 names).

Eight self-test assertions, run in CI-ish form before the check, including the exact summary collapse that caused this: replace `= 17 direct, 8 closure,` with `= 11 indirect,` and the guard fails. The ADR's own ratio is checked the same way — that is the assertion #1361 needed and did not have.

```
derived: 11 forbidden-crate + 6 narrowed-tokio = 17 direct, 8 closure · 25 of 38 modules · 20 items · 9 dependencies
record's ratio: 25 of 38 public modules
self-test: 8/8 assertions
```

## Validation

```
validate-docs / guards / architecture / canonical / tests / wiring-claims   all exit 0
validate-shell-exit-status / script-drift                                   exit 0
cargo fmt --all --check                                                     clean
cargo test -p rigorix-engine --test unit pinned                             7 passed
```

Closes #1361
