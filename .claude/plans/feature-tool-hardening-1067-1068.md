# Batch 1 — Issues #1067 + #1068

**Branch:** `feature/tool-hardening-1067-1068`

## #1067 — the policy must see the string the shell executes
- New `tools::application::shell_policy`:
  - `find_shell_control_operator` — refuses `;` `&&` `||` `|` `&` backticks
    `$(` `<` `>` and newlines.
  - `field_from_intent` / `command_from_intent` — one extraction used by both the
    bash classifier and the executor.
- `RunCommandTool` refuses a compound before the prefix check (and
  `is_command_allowed` refuses it too, defence in depth).
- `exec_run_command` (the *actual* dispatch path, which had no allowlist at all)
  refuses the same operators and reports `shell_operator_rejected`.
- `execute_tool` feeds `check_bash` the extracted command, not the JSON intent.

## #1068 — the child must not inherit engine secrets
- `apply_child_env`: `env_clear()` + `PATH HOME LANG LC_ALL TMPDIR SHELL TERM`
  + per-call `env` overrides (applied last).
- Applied to `RunCommandTool`, `exec_run_command`, `GitReadTool`.
- Docs state explicitly: this removes a gift, it is **not** a sandbox; the child
  still runs as the engine's user with its filesystem access.

## Tests
- `shell_policy`: operator detection, extraction, allowlist excludes the key.
- `run_command_tool`: 10 compound forms refused; `CARGO_MANIFEST_DIR` absent in
  the child while `PATH` is present; explicit overrides still apply.
- Full engine suite + `validate-ci.sh` 5/0.
