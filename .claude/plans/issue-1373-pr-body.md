The runner, not the layout — and the channel the runner silently drops.

## What changes

| half | file | what it does |
|---|---|---|
| runner | `.config/nextest.toml` | one process per test; a test over 5 s is named `SLOW`; a hung test is terminated at 5 min instead of holding the job |
| doctests | `.pi/scripts/run-doctests.sh` | the channel nextest does not run — prints discovered/executed/ignored per crate, fails below a floor, fails on zero executed |
| floor | `.pi/doctest-floors.tsv` | ratchet over **executed** doctests (`rigorix_engine 6`) |
| pin | `.github/actions/setup/action.yml` | nextest **0.9.151**, prebuilt, and the pin is *verified* at runtime |
| CI | `.github/workflows/ci.yml` | test stage: `nextest run -p … --lib` + per-crate doctests; integration stage: `nextest run --workspace` + workspace doctests + a no-retry assertion |
| loop | `.pi/scripts/local-ci.sh` (+ wasm mirror) | nextest when present, announced fallback to `cargo test` when not |

## The counts reconcile exactly

```
$ cargo nextest run --workspace
    Starting 4109 tests across 90 binaries
    Summary [  29.797s] 4109 tests run: 4109 passed, 0 skipped

$ cargo test --workspace
  96 suites, 4115 passed, 10 ignored

$ bash .pi/scripts/run-doctests.sh
rigorix_engine   16 discovered  6 executed  10 ignored  ok
4115 − 6 executed doctests = 4109   ✓
```

That reconciliation is the whole reason the doctest step is required rather than nice: adopting nextest alone would have dropped 6 executed doctests and left the job green.

## Two numbers in the issue that measurement did not support

**The 43 s target is not a slow test.** The issue says `reconciliation_policy_integration` is "70 % of the wall time". That 43 s was a `cargo test --test <t>` **invocation**, measured while the tree was still rebuilding after a cold start. Warm, back to back:

```
cargo test --test reconciliation_policy_integration:  0 s, 1 s      (whole invocation)
nextest, same target:                                 Summary [0.012s] 4 tests run
```

No test in the suite exceeds the 5 s threshold. So the value claimed for nextest is attribution and isolation — which the seeds below do demonstrate — not the naming of an outlier that does not exist.

**No throughput claim survives either.** Back-to-back warm: `cargo test -p rigorix-engine` 11–23 s, nextest 9–10 s. An earlier reading of 64 s vs 10 s was taken *while alternating runners*, which forces rebuilds on every switch — trap 10's shape (a measurement without a rebuild in the same command). Discarded rather than quoted.

## Doc tests are a smaller channel than the count suggests

`cargo test --doc` prints `running 16 tests` for `rigorix_engine` and executes **six**. The other ten are ```ignore examples on public APIs — `AuditEnvelopeFactoryImpl::with_pinned_inputs`, `LlmBudgetReservation`, `Secret`, `MockClassifier`, `AnchorParams`, … — never compiled, never run, free to rot. The floor is therefore over `executed`: a count of discovered tests is not a count of tests that ran, which is the same distinction that made a 262 351 B budget pass a 243 366 B module (#1353) and a 26.5 % share read as 34.2 % of a different denominator (#1363).

## Seeds — each verdict observed to fire, then restored

```
SLOW [>  5.000s] (───) rigorix-engine::identity_envelope_integration test_envelope_factory_populates_identity_block_from_input
Summary [   6.020s] 3 tests run: 3 passed (1 slow), 0 skipped                      ← seeded 6 s sleep

FAIL [   0.014s] (3/3) rigorix-engine::identity_envelope_integration test_envelope_factory_populates_identity_block_from_input
thread '…' panicked at engine/tests/identity_envelope_integration.rs:60:5:
SEEDED: one failure must name one test
Summary [   0.017s] 3 tests run: 2 passed, 1 failed, 0 skipped                    ← one failure, one name

❌ rigorix_engine: 1 doctest(s) failed                                             ← broke a live doc example
❌ rigorix_engine: 6 doctest(s) executed, floor is 7 (.pi/doctest-floors.tsv)     ← floor ratchet
doctests present but all ignored → fail                                           ← discovered ≠ executed
no Doc-tests section → fail                                                       ← a command that ran nothing
run-doctests self-test: 6/6 assertions passed
```

The self-test caught two real defects in this very script while it was being written, both of the "assertion that cannot fail" family: a `trap … RETURN` deleted the throwaway crates after `mk` created them, so four cases "passed" by reading empty logs; and `verdict` read the log from **stdin**, which the heredoc carrying its own program had already claimed — so it read its own source and reported "no Doc-tests section found" every time. Only the self-test's *passing* cases could expose the second one.

## Out of scope, as filed

- No shared `CARGO_TARGET_DIR` between root and `wasm/` — 34 proc-macro crates is the whole deduplication, against one cargo lock shared across two workspace roots, and that lock is on record as having blocked a build for fifty silent minutes.
- No retries. `.config/nextest.toml` has no retry key, and CI asserts the key is absent rather than trusting the prose.
- No reduction of the dependency graph or the 46 test binaries.

## Validation

```
cargo nextest run --workspace                4109 tests, 4109 passed, 0 skipped
cargo test --workspace                       4115 passed, 10 ignored   (4415 − 6 doctests = 4109)
bash .pi/scripts/run-doctests.sh             ✅ 16 discovered / 6 executed / 10 ignored
bash .pi/scripts/run-doctests.sh --self-test 6/6
docs guards ci canonical tests architecture script-drift shell-exit-status   all exit 0
```

`AGENTS.md`/`CLAUDE.md` carry an unrelated symbol-count refresh from the GitNexus indexer; left out of this commit.

Closes #1373
