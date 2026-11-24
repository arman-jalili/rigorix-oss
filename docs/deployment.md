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

### The same declaration governs plan steps (#1487)

The path that actually runs a plan's `run_command` step is not the tool — it is
`execution_engine`'s dispatcher, which executes a shell string by design (there is no
allowlist there; the boundary is the declaration). Since #1487 it requires the **same**
declaration:

- `RIGORIX_SANDBOX` unset, unrecognized, or `none` ⇒ every `run_command` step fails with
  the failure type `sandbox_undeclared`, before anything is spawned. The step's error names
  the reach and the variable.
- The hosts (`cli`, `mcp`, the Action) print the remedy once at startup and keep running:
  a plan with no `run_command` step is unaffected, so a missing declaration is a per-step
  refusal rather than a broken installation.

The declared regime is recorded in the envelope's `metadata` (`isolation.regime`,
`isolation.requires`), so two runs whose steps ran under different boundaries are
distinguishable in the audit record.

**Breaking change on upgrade:** a deployment that runs `run_command` steps must set
`RIGORIX_SANDBOX`. Previously the steps ran with no declaration at all.

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
cleared before spawn.

Per-call `env` overrides are **not** a free channel (#1489). Two rules:

| Refused | Why |
|---|---|
| `PATH`, `LD_*`, `DYLD_*`, `GCONV_PATH`, `IFS` | they change **what executes**: the allowlist compared a name, the kernel runs a path, and the loader runs code before `main` |
| `BASH_ENV`, `ENV`, `PYTHONSTARTUP`, `NODE_OPTIONS`, `PERL5OPT`, `RUBYOPT` | they make an allowlisted interpreter run a file before the program it was given |
| any key the tool did not declare | an undeclared channel is how `PATH` became an executable selector; the tool declares the set it needs (`with_declared_env`) |

The program is resolved in the **engine's** `PATH` before any override is applied, and the
resolved path is returned on the result (`resolved_executable`), so the evidence names the
file that ran rather than the word that was approved. The step dispatcher's shell path
cannot make that record: the shell resolves its own argv — that is one of the things the
`run_command` step's `does_not` column says.

### `cwd` is part of the boundary (#1488)

An allowlisted word plus a caller-chosen directory is still arbitrary execution:
`cargo build` runs that directory's `build.rs`, `npm run` its lifecycle scripts, `git`
honours repo-local config. So `cwd` is resolved against the workspace root through
`tools/infrastructure/fs_at.rs` — each component opened with `O_NOFOLLOW` from the opened
root — and the child enters the **validated inode** with `fchdir` before `execve`, not a
path that is resolved again.

| `cwd` | Result |
|---|---|
| `/tmp`, or any absolute path outside the root | `PathDenied`, naming the root, before any spawn |
| anything containing `..` | `PathDenied` (accepted only by re-resolving, which is the defect) |
| a symlinked component | `PathDenied`, naming the link |
| a directory inside the root | runs there |

What it does not do: it does not stop a process with write access inside the root from
swapping a component **before** the resolution runs — the control is over the window
between validation and spawn, which is the window that existed.

---

## Permission mode: an unrecognized value is refused (#1487)

`RIGORIX_PERMISSION_MODE` (or `rigorix.toml`'s `permission_mode`) accepts
`read_only`, `workspace_write`, `danger_full_access` and the `action.yml` spelling
`dangerous_full_access`. Unset means the documented default, `workspace_write`.

Anything else — `read-only`, `readOnly`, a typo — is a **startup error** naming the
accepted values. It used to be absorbed by `_ => WorkspaceWrite`, so a near-miss silently
became a permission grant. The enforcer itself is not optional either: a host that cannot
build it fails to start, rather than continuing with mode gating skipped.

---

## `file-write`, `file-edit`, `file-patch`: handle-coupled, not path-revalidated

All three file-mutating tools resolve a target once, through
`tools/infrastructure/fs_at.rs`, which walks each component with `openat(…, O_NOFOLLOW)` and
returns the **opened parent directory handle**. The temp file is created in that directory and `renameat`-ed within it, so a
component swapped for a symlink after validation cannot redirect the write. A symlinked
component is refused by name, not followed.

Editing and patching read through the same handle (`read_file`) and write through
`write_atomic`, so the bytes transformed and the file written are one inode family — before
#1490 those two tools re-resolved the path for the write, which was SINK-2 verbatim in the
tools next door to the one #1479 fixed.

What they do not do: survive a full disk, or provide durability guarantees beyond the
`fsync` on the temp file before rename.

## Register rows with `mitigation=none` need an owner (#1490)

A register row is a description; a finding needs a ticket. `validate-sink-register.sh` now
refuses a `none` row whose `requires` does not cite an issue (`#1490`), and the register has
no `none` rows left: this batch fixed the five it found (append/edit/patch re-resolution, the
two non-atomic stores, and the placeholder API key), plus the two the guard surfaced while
the rule was being written. The self-test seeds an unmitigated row without an owner and
requires the guard to reject it.

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

## Evidence is a run outcome, not a debug field (#1491)

`RunOutput.evidence` reports `not_requested`, `complete`, or `degraded { reason, undelivered }`.
The engine computes it (#1480); since #1491 both surfaces show it:

- **CLI** — the status bar prints `evidence: complete` / `evidence: degraded — <reason>
  (<n> undelivered)` next to the run's phase, and the degraded form is red and bold. A
  completed run with a lost envelope is not the same outcome as a completed run with a
  complete one, so the two must not render the same.
- **MCP** — `rigorix_execute` returns `evidence` as `{ "status": …, "reason": …,
  "undelivered": … }`; `reason` and `undelivered` exist only for `degraded`.

`not_requested` is always stated, never omitted: an absent field cannot be told apart from
a client that does not know the field, which is how the original defect stayed invisible.

## The state directory is owner-only (#1658 item 2)

Measured before the change: with `umask 022`, `std::fs::write` creates **0644** and `create_dir_all`
**0755**, so every envelope under `.rigorix/audit/` — which records the planning prompt, the file paths and
the findings — was readable by every account on the host. Templates (`.rigorix/templates/`, operator
configuration, and therefore the gates a run is judged by) and receipts and attestations were in the same
state. The agent's own writes never were: `fs_at` opens them `0600`.

Now:

| what | mode | created by |
|---|---|---|
| `.rigorix/audit/`, `…/receipts/`, `…/attestations/`, `.versions/` | `0700` | `common::private_fs::create_dir_all` |
| an envelope, a receipt, an attestation, a template, a kept version | `0600` | the same module, through the temp file that the atomic rename makes the destination |

**What this is not.** It is not a permission model: it does not defend against another process running as
the owner, and it does not rewrite files written before it existed — those are tightened the next time the
store replaces them, because the temp file carries the mode. A deployment that needs the old files closed
now must walk them itself. It also does not make the audit trail tamper-evident; that is the anchor's job
(ADR-016), and `RIGORIX_ANCHOR_KEY` being absent is a *state* the trail reports, not a mode.

## The HTTP front door: one origin decision, and free mode as a non-claim (#1650)

`rigorix-server` mounts four routes — `/health`, `/rpc`, `/mcp`, `/events` — and **one** function
decides whether a browser may reach any of them (`server/src/origin.rs`). It is one function on
purpose: the origin guard used to exist only in `mcp_http.rs`, so `/rpc` had none and a page on any
origin could drive a free-mode executor on `127.0.0.1:3001`, and `/events` had none either.

What the decision refuses, and why each clause exists:

| Request | Decision | Why |
|---|---|---|
| no `Origin` header | **allowed** | a non-browser client — curl, an SDK, the Action entrypoint. It is not the attacker this defends against |
| `Origin: null` | **refused** | a sandboxed iframe, a `data:` document or a redirect. "null" is not the absence of an origin, and allowing it let a hostile page through |
| origin host is loopback, or equals the request's `Host` | allowed | the shipping deployment and a same-origin page |
| any other origin | **refused**, naming that origin | a page you merely have open must not execute anything here |
| `Origin` present and content type is not `application/json` | **refused** | that is the "simple request" a browser sends **without a preflight**; the JSON shape is the only one a browser may use |
| `OPTIONS` on `/rpc` or `/mcp` | **403, no `Access-Control-*` headers** | a preflight is refused explicitly. This route used to answer 405, which reads as "wrong shape" and leaves the preflight-free path open. Permissive CORS is what turns an absent preflight into a readable response, so there is none |

**Free mode is a supported choice, and it is a non-claim, not a control.** With no IdP configured:

- `AuthLevel::Session` methods (`rigorix.execute`, `rigorix.approve`, …) answer **any** caller that
  reaches the port. Identity is the caller's claim; there is no principal.
- `GET /events` streams every event **unauthenticated**. The origin decision applies to it, so a page
  on another origin cannot subscribe — but a client that sends no `Origin` needs no credential.
- The engine's own gates (preconditions, sequence policy, approvals, receipts) still run. They bind
  the *consequence*; they do not authenticate the person asking for it.

Because that is a non-claim rather than a control, the server prints one loud line when free mode is
bound beyond loopback, naming what becomes reachable (`rigorix.execute`, `rigorix.approve`, `/events`)
rather than saying "insecure". It never refuses to start: the quickstart must keep working, and a
warning that stops the server is a configuration change disguised as a check. Put a token or an IdP in
front of the port before it faces anything but a throwaway network.

What this decision does **not** do: it does not authenticate anyone, and it does not stop a **local**
attacker who can send a request with no `Origin` at all — that is a token/binding problem, a separate
work item. It also does not stop DNS rebinding against a request whose `Host` the attacker controls;
the loopback bind default is what keeps that unreachable, which is why the non-loopback warning exists.

## The boundary is enumerated, and the register says what each control is not (#1496)

`engine/.pi/architecture/decisions/ADR-023-sink-boundary.md` is the rationale for every
crossing out of the engine's types; `engine/.pi/architecture/sink-register.tsv` is the
enumeration, one row per call site. Three things an operator should take from it:

- **`RIGORIX_SANDBOX` is a declaration, not a proof.** The engine refuses any step that can
  start a process — `run_command`, `git_read`, `git_stage`, `git_commit` — when no declaration
  exists, and records which regime it was told it runs under. It does not inspect the namespace
  it is in. If you declare `container` and run without one, the audit record says `container`
  and the truth is otherwise. `git_read` additionally runs only read subcommands, refuses the forms that
  name a program (`-c` before the subcommand, `--exec-path`, `ext::`, …) and the forms that
  write (`--output`, `-d`/`-D`, `--set-upstream-to`, …), but **the repository still names programs git
  runs for ordinary reads** — `core.fsmonitor` on `git status`, `diff.external` on `git diff`,
  textconv drivers, aliases, hooks. `.git` is inside the workspace the agent can write, so the
  declaration is the boundary for that, not the allowlist.
- **`.rigorix/` protection is a resolved-path decision.** A write that lands inside it is
  refused, including through an alias (a symlinked directory). It does not make the directory
  unwritable: an operator who widens its mode, or a status quo where the agent's user owns the
  repository, still has a hook boundary in name only.
- **Spawned programs are resolved to a path and executed by path.** The evidence names the
  binary (`.resolved_executable`, or the `engine:exec-git` row); a writer in that directory can
  still swap the file between resolution and exec. The row states this rather than the comment
  claiming otherwise.
