# Issue #1083 — confine process spawning with a compile-time lint

**Branch:** `issue-1083`

## Mechanism
- `clippy.toml`: `disallowed-types` = `std::process::Command`, `tokio::process::Command`
  (with reasons pointing at the sink inventory).
- `[workspace.lints.clippy] disallowed_types = "deny"` + `[lints] workspace = true`
  in all five crates → restriction lint (allow-by-default) is denied for plain
  `cargo clippy`, not only CI.
- Every currently approved sink opts in with a file-level
  `#![allow(clippy::disallowed_types)]` + a reason naming its policy.

## Inventory (19 files)
Governed shell (2), policy itself (1), fixed-argv (11), operator argv/hook (2),
test-only (5 incl. CLI/MCP e2e). Documented in
`modules/tool-system.md#process-spawning-sinks`, including the two rows that are
**not** yet conformant so the inventory does not overclaim.

## Demonstration (load-bearing)
`engine/tests/sink_probe.rs` with `Command::new("true")` →
`error: use of a disallowed type std::process::Command` (exit 101).
Same file with the documented `#![allow]` → exit 0. Probe deleted.

## Scope note
The lint confines *where* spawning happens, not *what* the command does with
caller input: `git_read_tool` still interpolates a caller path into `sh -c`, and
`hook_runner_impl` does not clear the child env. Both are listed as
non-conformant and are #1082's guards C2/C3.
