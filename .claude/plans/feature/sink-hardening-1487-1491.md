# Implementation Plan: feature/sink-hardening-1487-1491

**Branch:** `feature/sink-hardening-1487-1491`
**Base:** `main` (a314672c)
**Created:** 2026-11-08

Batch 2 of the sink series. Batch 1 (#1478–#1481) made the *register* honest; this
batch finishes the three rows it could only describe, makes the reachable dispatcher
declare its isolation, and carries #1480's `evidence` to the two surfaces that drop it.

## #1487 — SINK-4 · the reachable dispatcher declares its isolation, and its gate stops failing open

**Priority:** critical · **Exposure:** reachable on every plan step, default config.

| Change | File |
|---|---|
| `sandbox: Option<Arc<SandboxDeclaration>>` + `with_sandbox_declaration` | `engine/src/execution_engine/application/service_impl.rs` |
| `exec_run_command` becomes a method; no declaration ⇒ `sandbox_undeclared` failure naming the reach, nothing spawned | `engine/src/execution_engine/application/service_impl/tools.rs` |
| absent enforcer at the gate ⇒ refusal (`permission_enforcer_unavailable`), not "skip the block" | same two files |
| `resolve_permission_mode` returns `Result`; unrecognized value is a startup error naming the accepted set | `cli/src/cli_boundary/orchestrator.rs`, `mcp/src/host/mod.rs` |
| enforcer construction failure propagates instead of `continuing without mode gating` | same |
| the envelope records the declared regime (`metadata["isolation.regime"]`) | `engine/src/orchestrator/application/orchestrator_impl.rs` |
| register row `engine:exec-run-command`: enforcement is real now | `engine/.pi/architecture/sink-register.tsv` |

**Acceptance**

| # | Criterion | Proof |
|---|---|---|
| 1 | unset ⇒ no execution | `a_run_command_step_without_a_declaration_is_refused_before_any_spawn` (asserts the marker file does not exist, not just the failure type) |
| 2 | declared ⇒ runs | `a_declared_regime_lets_the_step_run` |
| 3 | enforcer not optional | `an_absent_enforcer_refuses_instead_of_skipping_the_gate` + `cli`/`mcp` parse tests + a source guard for the removed sentence |
| 4 | a typo is not permissive | `an_unrecognized_permission_mode_is_an_error` in both hosts |
| 5 | the register says so | `bash engine/.pi/scripts/validate-sink-register.sh` |

**Breaking change:** `RIGORIX_SANDBOX` is now required for `run_command` steps. Recorded
in `docs/deployment.md` and `CHANGELOG.md` `[Unreleased]`.

## #1489 — SINK-5 · env overrides cannot choose the executable

- `shell_policy`: `RESERVED_ENV_KEYS` refused by name (`PATH`, `LD_*`, `DYLD_*`,
  `BASH_ENV`, `ENV`, `PYTHONSTARTUP`, `NODE_OPTIONS`, …) with the reason for each class;
  `DECLARED_OVERRIDE_KEYS` the only keys a caller may set.
- The program is resolved against the **engine's** `PATH` before any override is
  applied, and the resolved path is what gets spawned and recorded.

**Acceptance:** fake `echo` on a caller `PATH` ⇒ refused by name, marker file never
created; `LD_PRELOAD` ⇒ refused by name; unknown key ⇒ `InvalidInput` naming the
declared set; the resolved path appears in the step output.

## #1488 — SINK-6 · cwd cannot leave the workspace

- `fs_at` grows `resolve_dir_for_child(root, cwd) -> ValidatedDir` (open root, walk
  components with `O_NOFOLLOW`, hand back the handle's path).
- `run_command_tool` resolves `cwd` through it: absolute-outside, `..`, and symlinked
  escapes are `PathDenied` before spawn.

**Acceptance:** `/tmp`, `../../`, and `ws/escape → /tmp` all `PathDenied`; `sub` still runs.

## #1490 — SINK-7 · a register row is not a ticket

- `file_edit_tool` / `file_patch_tool` mutate through `fs_at::ValidatedWritePath`;
  `resolve_path` is not used for mutation.
- Template generation: absent provider key ⇒ `InvalidInput` naming the variable; the
  placeholder path is deleted.
- `validate-sink-register.sh`: `mitigation=none` rows must cite an open issue number.

**Acceptance:** the `fs_at` swap test replayed against both tools; no `fs::write` in
their production sections; register guard refuses a planted `none` row with no issue.

## #1491 — EVID-2 · the engines's degraded evidence reaches both surfaces

- CLI TUI: the run summary prints the evidence status, and for `Degraded` the reason
  and undelivered count.
- MCP `rigorix_execute` response: `evidence: { status, reason?, undelivered? }`.

**Acceptance:** rendered sentence / serialised field asserted, not the struct; dropping
the renderer makes criterion 1 fail.

## Status

- [x] Issues identified (5)
- [x] Implementation plan created
- [ ] #1487 implemented
- [ ] #1489 implemented
- [ ] #1488 implemented
- [ ] #1490 implemented
- [ ] #1491 implemented
- [ ] Tests passing (workspace)
- [ ] MR created
- [ ] Pipeline green
- [ ] Merged
