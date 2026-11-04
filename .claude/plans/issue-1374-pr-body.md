The record decided the feature. The repository did not have it.

```
$ grep -c portable engine/Cargo.toml          # on main before this PR
0
$ cargo check -p rigorix-engine --features portable --target wasm32-unknown-unknown
error: failed to run custom build command for `tree-sitter v0.25.10`
```

## What now compiles

| assertion | result |
|---|---|
| `cargo check -p rigorix-engine --features portable --target wasm32-unknown-unknown` | **exit 0** |
| wasm32 **normal+build** graph names none of `tree-sitter`, `reqwest`, `ring`, `libc`, `prometheus`, `tracing-appender` | **all six absent** |
| `cargo test -p rigorix-engine --features portable --tests` | **48 suites, 880 passed, 0 failed** |
| default configuration unchanged | **2910 engine tests / 4109 workspace, 0 failed** |
| native `--features portable` (the reason this is a feature, not a target `cfg`) | **compiles and runs the slice's tests** |

## Rebase, not merge

Branch `c118cb06` is **1 ahead, 16 behind** `merge-base e703d03e`, so merging it would revert sixteen commits — its engine diff shows `main`'s newer `pinnedinputs_test.rs` as 177 deletions. Its 456-line engine diff is applied with `git apply -3` instead, with **one conflict**, and that conflict is the interesting one:

```rust
<<<<<<< ours
execution_id: self.pinned.execution_id().unwrap_or(input.execution_id),
timestamp: self.pinned.timestamp().unwrap_or_else(chrono::Utc::now),
=======
execution_id: input.execution_id,
timestamp: input.pinned_timestamp.unwrap_or_else(chrono::Utc::now),
```

The apparatus put the pins on `BuildEnvelopeInput` — a public, non-`#[non_exhaustive]` DTO. #1355 replaced that with `PinnedInputs` + `with_pinned_inputs` precisely because a new field is a downstream-only semver break. The rebase keeps `self.pinned` and deletes the field and the **17** struct-literal lines that existed only for it. So the gates are the branch's; the carrier is `main`'s.

## Gating the slice means gating its tests

170 `cfg(not(feature = "portable"))` sites across 23 files, and behind them, tests that reference excluded modules:

- `#[cfg(all(test, not(feature = "portable")))]` for source-internal test modules
- `#![cfg(not(feature = "portable"))]` for the 8 integration binaries that touch them
- six top-level `mod`s in `tests/unit/mod.rs` (one line each, rather than an attribute in ~100 files)

**No assertion was edited.** The default suite's count is identical before and after: 2910 tests, 0 failed.

`--tests`, not the whole suite, for the slice: an example demonstrating an excluded module cannot exist in the slice, and gating an example *file* leaves no `main` (E0601 — observed, not assumed).

## The guard found three false rows in the manifest

`--gates` asks both directions: every gated `pub mod` needs a row, and every row needs a gate. On the first run:

```
❌ portable-inventory.tsv: module row `dag_engine` has no cfg(not(feature = "portable")) gate — the module still compiles into the slice
❌ portable-inventory.tsv: module row `event_system` …
❌ portable-inventory.tsv: module row `observability` …
```

All three claimed `whole module`. All three are wrong, and gating them wholesale is not an option either — it fails with `cannot find dag_engine in crate` and `unresolved import super::GatingMode`, because modules that stay use them:

| module | what the code actually excludes |
|---|---|
| `dag_engine` | nothing — it compiles into the slice; row removed |
| `event_system` | one inner item (`event_info_status`); the events stay |
| `observability` | the `metrics` submodule; logging stays |

They are item rows now, so the counts move and everything derived from them moves with them:

| | before | after |
|---|---|---|
| public modules excluded | 25 of 38 | **22 of 38** |
| inner items | 20 | **22** |
| forbidden-crate + narrowed tokio | 11 + 6 = 17 | **10 + 6 = 16** |
| closure | 8 | **6** |
| "leave for a non-dependency reason" | 14 of 25 | **12 of 22** |

The ratio table, ADR §1, the evidence table and the Consequences paragraph follow; the guard recomputes all of it, and **13 of its own seeds had to move with the counts** — which is the check working, not the check being adjusted to agree.

## Seeds

```
❌ module row `repo_engine` has no cfg(not(feature = "portable")) gate — the module still compiles into the slice
❌ `pub mod error;` is gated but has no `module` row in the manifest
✅ 14/14 validate-portable-inventory self-test assertions
✅ scanner caught the planted unwrap
✅ `all(test, …)` is test scope, `not(test)` is production
```

The first seed is the important one: with the gate removed, the **compiler** names `tree-sitter`, not `repo_engine`, because the failure surfaces in a build script two layers away. That is why the manifest check exists.

## Two repairs found on the way

- **`cargo tree` without `-e normal,build` shows `reqwest`**: pulled in by `jsonschema`, a dev-dependency. The assertion ADR-014 leans on must name its subject, or the check fails while saying nothing about what ships. CI uses `-e normal,build`.
- **The panic-site scanner read only `#[cfg(test)]`**, so six test-only panics under `all(test, …)` reported as new production debt. It now recognises any `cfg` mentioning a bare `test`, and the self-test proves `not(test)` stays production — otherwise a real panic could hide behind a negation.

## CI

New **mandatory** stage `portable` (added to `CI_MANDATORY_STAGES`, `ci-report` and the Final Gate's `needs`): wasm32 check, graph assertion, slice tests, `--gates`. A configuration outside CI is a configuration that rots — that is how §1 became a paragraph with no implementation.

## Validation

```
cargo check -p rigorix-engine --features portable --target wasm32-unknown-unknown   0
cargo test  -p rigorix-engine --features portable --tests                          880 passed
cargo nextest run --workspace                                                      4109 passed
cargo fmt --all --check                                                            clean
cargo clippy --workspace --all-targets -- -D warnings                              0
validate-portable-inventory.sh --gates / --self-test                                ✅ / 14 of 14
validate-panic-sites.sh --self-test                                                ✅ / 2 of 2
docs guards ci canonical tests architecture script-drift shell-exit-status          all exit 0
```

`AGENTS.md`/`CLAUDE.md` carry an unrelated symbol-count refresh from the GitNexus indexer; left out.

Closes #1374
