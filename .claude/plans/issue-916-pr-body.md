# refactor: #916 OSS-REFACTOR-ORCHESTRATOR — split `orchestrator_impl.rs`

Tier-1 of the refactoring program (#914). The 4166-LOC coordinator
(`engine/src/orchestrator/application/orchestrator_impl.rs`) is split into
single-responsibility child modules; the file is now **1449 LOC** (< 1500).

## Extracted modules (children of the impl type — private-field access preserved)

| Module | Responsibility | LOC |
|--------|----------------|-----|
| `orchestrator_impl/approval_flow.rs` | `approve_execution` body (engine-session approval + cross-process hydration); coordinator delegates | 144 |
| `orchestrator_impl/effect_key.rs` | `run_effect_key` (R8/ADR-014 effect-key derivation) | 26 |
| `orchestrator_impl/policy_pipeline.rs` | composed R2 sequence-policy → R9 requirement gates + `PolicyPipeline` (fixed fail-closed order) | 308 |
| `orchestrator_impl/helpers.rs` | remaining assembled helper methods (`pub(super)`) | 346 |
| `orchestrator_impl/tests/*` + `identity_gate_tests.rs` | externalized test modules (Phase A commit) | ~1930 |

## Key changes
- **Test externalization (Phase A):** the inline `mod tests` (~1900 LOC) and
  `identity_gate_tests` are now `tests/` submodules split by concern — pure
  move, zero test-body change.
- **`PolicyPipeline`:** the two plan-time gates are now composed and invoked as
  one unit at the `run_from_template` / `plan_from_template` call sites,
  replacing the inline gate arms. The fixed order is **sequence policy (R2)
  then operator requirements (R9)**, requirements evaluating the
  sequence-enforced step list.
- **`approve_execution`** delegates to `approval_flow::approve_execution_flow`.
- New child modules are `pub(super)` — **no public-surface change**.

## Acceptance criteria

| # | Criterion | Evidence |
|---|-----------|----------|
| 1 | `orchestrator_impl.rs` < ~1500 LOC; new modules single-responsibility | **1449 LOC**; 4 new modules |
| 2 | No public-surface change; all existing orchestrator tests pass (same count) | 2100 existing lib tests pass; only `pub(super)` methods added |
| 3 | `cargo test --workspace` + clippy `-D warnings` green | below |
| 4 | Plan-time gate order + fail-closed semantics unchanged (ordering assertion) | `policy_pipeline_runs_sequence_policy_before_requirements` |

## Validation

```
cargo test -p rigorix-engine --lib        ✅ 2101 passed (2100 existing + 1 new ordering test)
cargo clippy -p rigorix-engine --all-targets -- -D warnings  ✅
cargo fmt --all --check                   ✅
```

## Notes
- The children live under `orchestrator_impl/` (not `orchestrator/tests/`) so
  they can access the coordinator's private fields without widening visibility
  to the rest of the module tree.
- `enforce_identity_requirement` (L1 gate) remains called **before** the
  composed pipeline at both call sites — unchanged.
- The M-15 rider (`approval_records` into `ExecutionState`) was **not** folded
  in: it is a behaviour change, not a pure move (out of scope here).

Refs #916 #914
