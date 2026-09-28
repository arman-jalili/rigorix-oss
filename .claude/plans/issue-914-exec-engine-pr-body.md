# refactor: #914 — split `execution_engine/service_impl.rs` (Tier 1 #2)

Tier-1 item #2 of the refactoring program (#914). The 3760-LOC
`ParallelExecutionServiceImpl` file is split into single-responsibility child
modules; it is now **1649 LOC**.

## Extracted modules (children of the impl — private-field access preserved)

| Module | Responsibility | LOC |
|--------|----------------|-----|
| `service_impl/dispatch.rs` | `run_dispatch_loop` + sequence-policy dispatch helpers (`pop_dispatchable`, `verify_before_dispatch`, `effect_scope_check`, `sequence_policy_verdict`, …) | 801 |
| `service_impl/tools.rs` | `execute_tool` + all `exec_*` node-tool methods + `notify_progress` | 917 |
| `service_impl/node.rs` | `execute_node_flow` (per-node execution + inline retry loop); the trait method delegates | 423 |

## Notes
- Methods are `pub(super)`; `notify_progress` keeps its `pub(crate)` visibility.
- **No public-surface change**; all 2101 engine lib tests pass unchanged.
- Coordinator (`service_impl.rs`) retains the struct, ctor/builders, and the
  `ParallelExecutionService` trait impl (graph/state/pause/resume/approve).

## Validation

```
cargo test -p rigorix-engine --lib        ✅ 2101 passed
cargo clippy -p rigorix-engine --all-targets -- -D warnings  ✅
cargo fmt --all --check                   ✅
```

Refs #914
