---
guardian_issue:
  id: "ISSUE-COMMAND-INHERITS-ENGINE-SECRETS"
  title: "run_command: the child process inherits the engine environment, including RIGORIX_HMAC_KEY and anchor credentials"
  epic: "execution boundary"
  component: "RunCommandTool"
  module: "tools"
  status: planned
  priority: critical
  dependencies: []

  intent: |
    Stop handing the engine's own credentials to commands the engine executes on behalf of an
    agent. `run_command` spawns `/bin/sh -c` with no `env_clear` and no `env_remove`, so the
    child inherits the full parent environment. That environment carries the signing key used
    for audit HMACs and approval binding, and any anchor or provider credentials the operator
    exported.

    A command that can read its own environment can therefore read the key that authenticates
    the audit trail, and use it. The control and the thing it protects share one identity.

  in_scope:
    - "Clear the inherited environment for the spawned process"
    - "Re-introduce only an explicit allowlist (PATH, HOME, LANG, TMPDIR, …) plus caller overrides"
    - "Never pass signing keys, anchor credentials or provider tokens"
    - "Adversarial test: a command that echoes the environment must not see RIGORIX_HMAC_KEY"
    - "Document the inherited-environment contract on the tool"

  out_of_scope:
    - "OS-level sandboxing / containerisation (deployment concern)"
    - "The policy defects in the allowlist and classifier (separate issue)"
    - "Protecting `.rigorix/` from an arbitrary subprocess (same class, needs the same isolation)"

  affected_layers:
    domain:
      - "None"
    application:
      - "Modified: RunCommandTool — process environment construction"
    infrastructure:
      - "Modified: process spawn"
    api:
      - "None"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - evidence: "engine/src/tools/application/run_command_tool.rs:125 — Command::new(\"/bin/sh\") with no env_clear"
    - evidence: "engine/src/tools/application/run_command_tool.rs:129 — only cmd.env(k, v) for caller overrides; the rest is inherited"
    - evidence: "grep 'env_clear' across engine/src/tools and engine/src/execution_engine -> no matches"
    - evidence: "engine/src/execution_engine/application/factory.rs:212 — RIGORIX_HMAC_KEY is read from the environment"

  acceptance_criteria:
    - "The spawned process does not inherit the engine environment"
    - "Only an explicit allowlist plus caller overrides reach the child"
    - "A test runs a command that prints its environment and asserts the signing key is absent"
    - "The tool documents which variables the child receives"
    - "Existing callers that relied on inheritance are identified; none depend on secrets"

  validators:
    - ci
    - tests
    - security

  implementation_notes: |
    - `env_clear()` first, then add the allowlist and the caller overrides. Order matters: the
      overrides are currently applied without clearing, which is why inheritance is invisible.
    - An allowlist is safer than a denylist: new secrets appear in the environment over time,
      and a denylist silently stops covering them.
    - Note the honest limit in the doc: clearing the environment removes the easiest path to the
      key, it does not sandbox the process. The child still runs as the engine's user and can
      read files the engine can read, including `.rigorix/`. Say so, and point at the deployment
      guidance.
    - Audit whether `run_command` is even reachable in the default configuration; if it is, this
      is reachable by any agent that can emit that tool call.

  file_changes:
    - "modify: engine/src/tools/application/run_command_tool.rs"
    - "create: engine/tests/unit/tools/run_command_env_test.rs"
    - "modify: docs (tool contract / deployment guidance)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-COMMAND-INHERITS-ENGINE-SECRETS

## The defect

```rust
// run_command_tool.rs:125
let mut cmd = tokio::process::Command::new("/bin/sh");
cmd.arg("-c").arg(&command).current_dir(&cwd);

for (key, value) in &env_overrides {
    cmd.env(key, value);      // additions only — nothing was cleared
}
```

No `env_clear`, no `env_remove` anywhere in `src/tools/` or `src/execution_engine/`. The child
inherits everything the engine process has.

The engine reads `RIGORIX_HMAC_KEY` from the environment (`factory.rs:212`) to sign audit
envelopes and bind approvals. So an executed command can:

1. read `RIGORIX_HMAC_KEY` from its own environment,
2. and forge envelopes that verify against the same key.

The audit control and the thing it is supposed to constrain share one secret, and the engine
hands it over on every command it runs.

## Suggested fix

```rust
cmd.env_clear();
for (k, v) in allowlist { cmd.env(k, v); }
for (k, v) in env_overrides { cmd.env(k, v); }
```

with a small allowlist (`PATH`, `HOME`, `LANG`, `LC_ALL`, `TMPDIR`, `SHELL`) and a test that
runs a command printing the environment and asserts the key is absent.

## Do not overclaim

Clearing the environment closes the trivial path. It does not sandbox the child: it still runs
as the engine's user with the engine's filesystem access, so it can read `.rigorix/` and any
credential file the engine can read. The doc must say that, and the deployment guidance must
point at a real isolation boundary. This issue removes a gift; it does not build the wall.
