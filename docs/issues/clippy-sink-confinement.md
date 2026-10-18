---
guardian_issue:
  id: "ISSUE-CLIPPY-SINK-CONFINEMENT"
  title: "clippy.toml: ban process-spawn types and methods outside the approved module, so a new spawn site cannot bypass the shell policy"
  epic: "self-owned vulnerability checks"
  component: "Guards"
  module: "governance"
  status: planned
  priority: critical
  dependencies:
    - "#1067, #1068 — the two defects this would have caught"

  intent: |
    Close the hole that let a second, unguarded `sh -c` path exist unnoticed.

    `exec_run_command` executed any intent through `sh -c` with no allowlist and no control-operator
    check. It was not hidden — it was in the same file as the guarded path — but nothing in the
    build knew that spawning a process was a privileged operation requiring a policy. A new spawn
    site compiled and passed every gate by default.

    Make spawning fail closed at compile time: the process-spawn types are disallowed everywhere
    except the module that applies the policy, so adding a spawn site is a compile error until it
    either uses the approved helper or records an explicit, reviewer-visible `#[allow]`.

  in_scope:
    - "Add clippy.toml with disallowed-types for std::process::Command and tokio::process::Command"
    - "Add disallowed-methods for Command::new / arg where the lint can express it"
    - "Allow the lint only inside the approved shell-policy module(s)"
    - "Route every spawn through one helper that clears the environment and applies policy"
    - "Deny the lint on the workspace (clippy -D warnings already gates CI)"
    - "Seeded violation: add a spawn in a new module, assert the build fails, revert"

  out_of_scope:
    - "Sandboxing the child process (deployment concern)"
    - "Covering non-Rust spawns (shell scripts in .pi/ are covered by a different class guard)"

  affected_layers:
    domain:
      - "None"
    application:
      - "Modified: the approved shell-policy module — carries the only #[allow]"
    infrastructure:
      - "Modified: clippy.toml (new), Cargo.toml lint wiring if needed"
    api:
      - "None"

  canonical_references:
    - evidence: "engine/src/execution_engine/application/service_impl/tools.rs — exec_run_command spawned `sh -c` with no allowlist (#1067)"
    - evidence: "engine/src/tools/application/run_command_tool.rs:125 — the guarded path, same sink"
    - evidence: "engine/src/tools/application/shell_policy.rs — apply_child_env, the intended single helper"
    - precedent: "rigorix-enterprise core/.pi/scripts/ci/check_capacity-ledger_contracts.sh — forbid() guards that fail on a pattern's presence"

  acceptance_criteria:
    - "clippy.toml disallows std::process::Command and tokio::process::Command"
    - "Only the approved module carries #[allow(...)]; the allow is justified in a comment"
    - "A spawn added in any other module fails `cargo clippy -- -D warnings`"
    - "Every existing spawn routes through the helper (environment cleared)"
    - "The seeded violation is demonstrated in the PR (fail, revert, pass)"
    - "validate-guards.sh registers the class so it cannot be dropped silently"

  validators:
    - ci
    - security

  implementation_notes: |
    clippy's disallowed-types is the right lever because it is compile-time and workspace-wide.
    It cannot scope by module, so the shape is: deny globally, `#[allow]` locally in the module
    that owns the policy. That inverts the default — a new spawn site must justify itself rather
    than being trusted.

    The `#[allow]` is the audit point. One line, one place, with a comment naming the policy it
    relies on. Review then has something to look at, which it did not have before.

    If disallowed-methods cannot express `Command::arg`, that is fine: confining the type already
    forces every spawn through the helper, and the helper is where the policy lives.

    Do not add an allow for a test module without a reason: tests that spawn are how the policy is
    tested, but they should use the helper too, so the helper stays exercised.

  file_changes:
    - "create: clippy.toml"
    - "modify: engine/src/tools/application/shell_policy.rs"
    - "modify: engine/src/execution_engine/application/service_impl/tools.rs"
    - "modify: engine/src/tools/application/run_command_tool.rs"
    - "modify: .pi/guards.tsv"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-CLIPPY-SINK-CONFINEMENT

## The defect this prevents

Two `sh -c` sites existed in the same crate:

| site | policy |
|---|---|
| `run_command_tool.rs:125` | allowlist + control-operator refusal |
| `service_impl/tools.rs` (`exec_run_command`) | **none** — any intent, any operator |

The unguarded one was in the **real dispatch path**. It was found by reading, not by the build.

The build could not know that spawning a process is privileged, because nothing declared it. So
the default for a new spawn site was *ungoverned*, and the reviewer had to notice.

## The change

Invert the default:

```toml
# clippy.toml
disallowed-types = [
  "std::process::Command",
  "tokio::process::Command",
]
```

Workspace-wide deny (clippy already runs with `-D warnings`). The single approved module opts
back in:

```rust
// engine/src/tools/application/shell_policy.rs
#![allow(clippy::disallowed_types)] // owns the one spawn path: env cleared + policy applied
```

Now a spawn site outside that module is a **compile error**, and the fix is either "use the
helper" or "write an `#[allow]` a reviewer will see". That is the correct direction of default.

## Why this is the highest-leverage guard of the set

It is compile-time, workspace-wide, and needs no custom parser. It closes class C1 (unguarded
sibling path) for process spawning permanently, and it composes with C3 (environment clearing):
once every spawn goes through one helper, "the child gets a cleared environment" is a property of
one function rather than a thing nine call sites must each remember.

## Acceptance

Load-bearing: **add a spawn in a new module and watch the build fail.** A lint that was never
observed to fire is indistinguishable from no lint.
