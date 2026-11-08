# Issue groups — batch 2 (2026-11-08)

Fetched with `.pi/scripts/fetch-issues.sh` (5 open issues, no labels — priority and
component live in each body's `guardian_issue:` YAML block, which is why the script's
label-based priority summary reads "No label: 5". The grouping below comes from the
bodies.)

## Batch: sink hardening — `feature/sink-hardening-1487-1491`

Every issue in this batch is a direct consequence of what the sink register (#1481)
recorded: three rows said `mitigation: none`, one row said "NOT yet enforced at this
site", and #1480's third criterion stopped at the engine boundary. They share files
(`run_command_tool`, `shell_policy`, the register, the two host composition roots) and
they share one shape: *a control is present and is not the control the sentence claims*.

| Order | Issue | ID | Priority | Component | Why this position |
|---|---|---|---|---|---|
| 1 | #1487 | SINK-4 | critical | `execution_engine/.../tools.rs`, `service_impl.rs`, cli + mcp composition | The only *reachable* sink. It also edits both host composition roots that #1491 touches, so it goes first to avoid rebasing host code twice. |
| 2 | #1489 | SINK-5 | high | `tools/application/{run_command_tool,shell_policy}.rs` | Same file as #1488; the env/executable question is upstream of the cwd question (the program is chosen before it runs). |
| 3 | #1488 | SINK-6 | medium | `tools/application/run_command_tool.rs` | Sequential on the same file as #1489, and it needs the `fs_at` handle discipline #1479 built. |
| 4 | #1490 | SINK-7 | medium | `tools/application/{file_edit_tool,file_patch_tool}.rs`, template generation, register guard | Independent of 1–3 (different tools), but its third criterion edits the register that 1–3 also touch. |
| 5 | #1491 | EVID-2 | medium | `cli/src/tui`, `mcp/src/execution_tools` | Surface last: it renders/serialises what 1–4 finished changing, and both hosts are settled by then. |

Not grouped separately: all five are one series on one branch, the same decision as
batch 1 (instances before the register that records them).

## Validation per issue

| Issue | Deterministic gate | Extra |
|---|---|---|
| #1487 | `cargo test -p rigorix-engine` + cli + mcp | docs + canonical (deployment.md, register row) |
| #1489 | engine lib tests | the "fake `echo` on a caller PATH" test asserts a marker file, not an exit code |
| #1488 | engine lib tests | symlinked-cwd case uses the same swap discipline as `fs_at` |
| #1490 | engine lib tests + `validate-sink-register.sh --self-test` | canonical (register is a canonical reference) |
| #1491 | cli + mcp tests | docs |

## Branch-wide gate (Phase 6)

`cargo build --workspace`, `cargo test --workspace --all-targets`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`,
`cargo audit`, `.pi/scripts/validate-guards.sh` (+ `--self-test`),
`.pi/scripts/validate-docs.sh`, and every `engine/.pi/scripts/validate-*.sh`.
