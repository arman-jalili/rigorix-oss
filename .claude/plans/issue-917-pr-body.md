# refactor: #917 OSS-REFACTOR-TESTS — externalize the 3 in-module test files

Tier-0 (zero-behaviour-risk) warm-up for the refactoring program (#914).
`execution_engine/tests.rs` (2634), `planning/tests.rs` (2203), and
`dag_engine/tests.rs` (1230) exceeded 1000 LOC **only** because of in-module
test code. They are now `tests/` submodule directories split by concern, wired
from the parent with the existing `#[cfg(test)] pub(crate) mod tests;`.

## Layout

```
engine/src/execution_engine/tests/{mod,dispatch,parallel,retry,hooks,approval,execution}.rs
engine/src/planning/tests/{mod,hash,pipeline,clarification,mocks,errors}.rs
engine/src/dag_engine/tests/{mod,topo,graph,ready,completion,diff}.rs
```

- **`tests/mod.rs`** keeps the module docs, the shared imports (re-exported
  `pub(crate)` so the concern files get them via `use super::*;`) and the shared
  helpers.
- **Concern files** hold only the `#[test]` / `#[tokio::test]` items, each
  beginning with `use super::*;`.
- Largest new file: **746 LOC** (`retry.rs`); every file is under 1000.

## Acceptance criteria

| # | Criterion | Evidence |
|---|-----------|----------|
| 1 | Three sources drop below 1000 LOC; `#[test]` count unchanged | `tests.rs` removed; **230** test attrs before == after |
| 2 | `cargo test --workspace` green — same tests, same results | engine lib **2100 passed** (unchanged); all engine targets green |
| 3 | No production code changed (only module declarations) | `git diff` = test files only |

## Validation

```
cargo test -p rigorix-engine            ✅ 2100 lib + all integration targets, 0 failed
cargo clippy -p rigorix-engine --all-targets -- -D warnings  ✅
cargo fmt --all --check                 ✅
```

## Notes
- Pure mechanical move via a top-level-item parser (brace-depth aware); no test
  body was edited. Helpers live in `mod.rs`; concern files pull them through
  `use super::*;`.
- Parent `mod.rs` declarations were already `#[cfg(test)] pub(crate) mod tests;`
  — unchanged.
- CI follow-up (Phase 9): `engine/.pi/scripts/ci/check_planning-pipeline_contracts.sh`
  counted tests only in `$SRC/tests.rs`; it now also counts `$SRC/tests/**` so the
  externalized planning tests are recognized (99 test functions).

Refs #917 #914
