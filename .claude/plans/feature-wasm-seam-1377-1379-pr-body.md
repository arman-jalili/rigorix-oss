Three findings, all downstream of one session's worth of measurement, and all the same shape: a number or a claim that a checker could not see.

## #1377 — a bare count is now checked like a ratio

ADR-014 §1 read:

> **22 of 38** public modules excluded … Of those **25**, **only 11 leave for a forbidden crate**; 6 leave because they need a `tokio` feature … and 6 because they name a symbol from an excluded module. So **12 of 22** leave for something that is *not* a dependency statement.

10 + 6 + 6 = 22 and 22 − 10 = 12, so the paragraph was self-consistent and describing the **previous** inventory. The guard certified it because it matched `N of M` and `N %`; "25 modules" is neither shape.

`--bare-counts` now derives `modules`, `items`, `dependencies`, `crates`, `direct`, `closure` and checks every bare integer against them, including the pronominal forms the prose uses:

```
❌ ADR-014…:47 states 'Of those 25' but the module count is 22
❌ ADR-014…:47 states 'only 11\nleave for a forbidden crate' but the forbidden-crate count is 10
❌ ADR-014…:47 states '8 dependencies' but the dependency count is 9
```

The middle seed is the one that matters: the claim **wraps across two lines**, so the first version of this check read `only 11` as a number with no subject and passed it. Idioms are now matched on the line joined with the next. A count whose subject cannot be derived fails with a reason — the rule #1363 applied to ratios.

`74 crates` became a recorded quantity with provenance instead of an exemption, and the ratios file's own provenance note ("reaches 11 of the 25") was the third copy of the stale pair.

`--self-test`: **17/17**.

## #1378 — the report says which tree it measured

`docs/spike-portable-slice-2026-11.md` stated "25 of 38 modules excluded, 11 of them for a forbidden crate" as a **current** count in three places, including the M1 bullet. The branch-era figures stay — they are what M1 measured — with the correction attached at each site: what changed, why gating `dag_engine` wholesale does not compile, and where the current counts live.

## #1379 — ten doctests that were never compiled, two of them wrong

```
$ cargo test --doc          # before
running 16 tests … 6 passed; 0 failed; 10 ignored
```

An `ignore` fence is not a slow example; it is an example the compiler never sees. Two had drifted into being **factually wrong about the API they documented**:

| example | what it claimed | reality |
|---|---|---|
| `MockClassifier` | `.with_low_confidence("write", 0.15)` | no such method has ever existed |
| `OrchestratorBuilder` | `OrchestratorBuilder::new(config)` | `new` is a trait method; the impl is `OrchestratorBuilderImpl` |

Now: **16 discovered, 16 executed, 0 ignored.** Seven fences became runnable examples; three are ```no_run` because they genuinely need a live provider, an async runtime with a real reservation, or real services — and `no_run` still compiles, which is the property that was missing.

Converting them found three more drifted examples: `classify_failure` imported `rigorix::…` (the crate is `rigorix_engine`), the budget lifecycle called `budget.reserve(100)` against a trait that takes `ReserveBudgetInput`, and `with_pinned_inputs` asserted on an accessor that does not exist. Each is now written against the real signature.

**The ratchet moved with it**: floor 6 → 16, so a fence cannot quietly go back to `ignore`. `run-doctests.sh` also fails on any ```ignore` fence with no row in `.pi/doctest-exemptions.tsv` (currently no rows, which is the point), seeded both ways — **self-test 8/8**.

Whole-channel seed: rename `with_pinned_inputs` and the doc example stops compiling:

```
error[E0599]: no method named `with_pinned_inputs` found for struct `AuditEnvelopeFactoryImpl`
❌ rigorix_engine: 1 doctest(s) failed
```

## Validation

```
cargo nextest run --workspace                    4109 passed, 0 failed
cargo test -p rigorix-engine --features portable --tests   880 passed
bash .pi/scripts/run-doctests.sh                 16 discovered / 16 executed / 0 ignored
bash .pi/scripts/run-doctests.sh --self-test     8/8
validate-portable-inventory.sh --self-test       17/17
cargo fmt --all --check                          clean
cargo clippy --workspace --all-targets -D warnings   0
docs guards ci canonical tests architecture script-drift shell-exit-status   all exit 0
```

One aside from the run: with `status-level = "slow"` in the nextest config, the sole test above 5 s named itself — `rigorix-gate-wasm::unit determinism::a_fresh_process_produces_the_same_bytes_as_this_one`, 8.6 s, a process-spawning determinism check. That is the attribution #1373 was adopted for, arriving unprompted.

Closes #1377
Closes #1378
Closes #1379
