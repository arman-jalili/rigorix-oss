# Deployment — what the engine's boundaries actually are

This file exists because a security sentence lived in a source comment
(`engine/src/tools/application/shell_policy.rs`) where the operator — the person who
decides whether a feature is safe to enable — would never read it. Everything the engine
*relies on* the deployment to provide is stated here instead (#1478, SINK-1).

The register that records each boundary sink, what enforces it, what it relies on, and
**what it does not do** is `engine/.pi/architecture/sink-register.tsv`. The `does_not`
column is mandatory and is checked by `.pi/scripts/validate-sink-register.sh`, which runs
in the CI audit job.

---

## `run-command`: filtering is not isolation

`RunCommandTool` executes one executable with literal arguments. It is not a sandbox and
never was:

> Filtering decides **which executable starts**, not what it can do once started. The child
> runs as the engine's user, with the engine's filesystem access, network reach and any
> credential it can find on disk.

What the tool does enforce, and what it does not:

| Control | Enforces | Does **not** do |
|---|---|---|
| allowlist | the executable is one of the configured names | constrain the arguments (see the table below) |
| control-operator refusal | a compound shell string is rejected before execution | prevent the executable's own behaviour |
| argv execution (no `sh -c`) | no expansion, globbing, `$VAR`, or quoting semantics | sandbox the process |
| cleared environment | `RIGORIX_HMAC_KEY`, anchors and cloud credentials do not reach the child | hide files readable by the engine's uid |
| declared sandbox (`RIGORIX_SANDBOX`) | the deployment stated the boundary it provides | prove the boundary exists |

### The declaration is required

`run-command` cannot be constructed without a `SandboxDeclaration`, and the declaration is
read from the deployment:

```
RIGORIX_SANDBOX=container   # the child runs inside a container boundary
RIGORIX_SANDBOX=vm          # the child runs inside a VM boundary
RIGORIX_SANDBOX=none        # refused: no boundary exists, so the sink stays off
```

Unset, unrecognized, or `none` refuses to construct the tool with a message naming what
the child can reach. A declaration is not a proof — but it converts a silent assumption
into a recorded one, and it makes "we forgot" a startup failure instead of a finding.

### The allowlist's grain is the executable

`is_command_allowed` matches the first word. `<allowlisted> <arbitrary arguments>` is
accepted, and for the tools a build agent actually needs the arguments *are a program*.
Treat this column as the answer to "may an untrusted agent have this name?":

| Executable | What an operator allowlisting it grants | Safe for an untrusted agent? |
|---|---|---|
| `echo`, `ls`, `cat`, `pwd`, `env`, `wc`, `head`, `tail` | read/inspect; `cat` reads anything the engine can read | only with the filesystem boundary below |
| `git` | `git config`, hooks, aliases, `-c core.pager=…`, `git submodule` — arbitrary execution | **no** |
| `cargo` | `build.rs` runs on build; arbitrary code | **no** |
| `npm`, `pnpm`, `yarn` | lifecycle scripts run on install | **no** |
| `python`, `node`, `ruby`, `perl`, `sh`, `bash` | an interpreter with a source file | **no** |
| `make` | the Makefile runs | **no** |
| `curl`, `wget` | network egress with the engine's reach | **no** |
| `tar`, `unzip` | writes through archive paths | **no** |

The honest summary: **an allowlist containing a build tool or an interpreter is an
allowlist of arbitrary code execution.** The allowlist is a convenience filter for a
trusted caller, and only the container/VM boundary makes it a control.

### Environment

`INHERITED_ENV_ALLOWLIST` = `PATH HOME LANG LC_ALL TMPDIR SHELL TERM`. Everything else is
cleared before spawn. Per-call `env` overrides are applied last and are the caller's
responsibility — they are not part of the isolation story.

---

## `file-write`: handle-coupled, not path-revalidated

`FileWriteTool` resolves a target once, through `tools/infrastructure/fs_at.rs`, which walks
each component with `openat(…, O_NOFOLLOW)` and returns the **opened parent directory
handle**. The temp file is created in that directory and `renameat`-ed within it, so a
component swapped for a symlink after validation cannot redirect the write. A symlinked
component is refused by name, not followed.

What it does not do: survive a full disk, or provide durability guarantees beyond the
`fsync` on the temp file before rename.

---

## Audit evidence: durability and the loss window

The signed local trail (`.rigorix/audit/`) is the evidence of record for cross-run policy
history. A run whose envelope was not built, signed, saved and delivered reports
`evidence: Degraded { reason, undelivered }` on its output — it is never inferred from a
log line, and the result of the evidence path is not discardable (`#[must_use]`, and
`clippy::let_underscore_must_use` is denied workspace-wide).

The retry queue is file-backed when the composition root configures a directory
(`AuditQueueImpl::durable`); the in-memory default's loss window is stated in the
`AuditQueue` trait: **an undelivered envelope is lost on restart and on overflow**.
Configure the durable queue when the backend is expected to be unreachable for longer than
a process lifetime.

---

## What this file does not cover

- Provisioning the container or VM itself. The engine records which regime produced the
  evidence; it cannot verify that the regime is what the operator claims.
- Secret distribution. The engine withholds `RIGORIX_HMAC_KEY` from children; it does not
  decide where the key is stored.
