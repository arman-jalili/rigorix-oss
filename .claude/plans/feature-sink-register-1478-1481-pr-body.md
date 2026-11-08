# A control is present and is not the control the sentence claims

Four issues, one shape. Each was findable by reading, and only by reading:

| # | Finding | The sentence vs the control |
|---|---|---|
| #1478 `SINK-1` | `run-command` filters | the isolation it depends on was a source comment; the allowlist matched an executable, and `sh -c` still ran a shell string |
| #1479 `SINK-2` | `file-write` validates the path | the check and the mutation were separate events: a component swapped between them redirected the write |
| #1480 `EVID-1` | the evidence path sends an envelope | five results were dropped, and the fallback queue died with the process |
| #1481 `SINK-3` | — | lands the mechanism: a register, three class rules, one guard |

Order is deliberate: the three instances land first, so the register records
mitigations that exist rather than intentions.

---

## #1478 — run-command declares the isolation it relies on, and executes argv

`SandboxDeclaration` is a construction precondition: `RIGORIX_SANDBOX` must say
`container` or `vm`; unset, unrecognized and `none` are refused with the reach named
(*"the child runs as the engine's user, with the engine's filesystem access…"*).
There is no `RunCommandTool::new` without it.

A proven-single command is executed as **argv** (`argv_from_simple`), not through
`sh -c`: quoting follows the shell's rules for one simple command, and anything a
shell would expand is refused by name instead of passing through unexpanded.

The allowlist's grain is now stated where an operator reads it — `docs/deployment.md`,
including the executable table whose honest summary is: **an allowlist containing a
build tool or an interpreter is an allowlist of arbitrary code execution.**

| Acceptance | Proof |
|---|---|
| no sandbox declaration, no tool | `the_sink_cannot_be_built_without_a_declared_boundary`, `an_undeclared_sandbox_is_refused_and_names_the_reach` |
| the doc says it | `grep -n 'runs as the engine' docs/deployment.md` |
| argv, not a shell | `the_source_path_has_no_shell_command_flag` + `c2_shell_interpreters_are_confined_to_validated_sites` (its inventory lost the entry, as it should) |

## #1479 — file-write validates and mutates through one handle

New `engine/src/tools/infrastructure/fs_at.rs`: `resolve_for_write` walks each
component with `openat(…, O_DIRECTORY|O_NOFOLLOW)` and keeps the **opened parent
directory**; `write_atomic` creates the temp file there (`O_CREAT|O_EXCL|O_NOFOLLOW`),
fsyncs, and `renameat`s within the same handle. `ValidatedWritePath` has private
fields and no `PathBuf` accessor — a caller cannot hold the path and hand it to
`fs::write`. The type is the guard.

Non-unix targets refuse rather than fall back to a path-based write: the fallback
*is* the defect.

| Acceptance | Proof |
|---|---|
| no re-resolution | `the_tool_does_not_touch_the_filesystem_itself` (source scan, comments/test module excluded) |
| the race is tested | `a_component_swapped_for_a_symlink_cannot_redirect_the_write` |
| symlinks refused, not followed | `a_symlinked_component_is_refused`, `test_write_through_a_symlinked_component_is_denied`, `append_refuses_a_symlink_target` |

Recorded non-finding: the dangling-symlink variant does **not** escape
(`create_dir_all` refuses a non-directory component), so nothing here "fixes" it.

## #1480 — the evidence path states facts instead of dropping them

- `#[must_use]` on `build_and_send`, `enqueue`, `save`; `#![deny(clippy::let_underscore_must_use)]` at the
  `audit/`, `orchestrator/` and `precondition/` module roots.
- `BuildEnvelopeOutput.degraded: Option<DegradedEvidence>` — the reason names the queue:
  "lost on restart" for the in-memory one, "durably" for the file-backed one.
- `RunOutput.evidence: EvidenceStatus { NotRequested | Complete | Degraded { reason, undelivered } }`.
- `AuditQueueImpl::durable(dir, capacity)` — one JSON file per pending envelope,
  temp + fsync + rename before `enqueue` returns, FIFO re-read on construction.
  CLI and MCP wire it beside `.rigorix/audit` and warn with the loss window when
  they cannot.

| Acceptance | Proof |
|---|---|
| a dropped outcome fails | `#[must_use]` + the module-level deny (the compiler, not a reviewer) |
| the queue survives a restart | `a_durable_queue_survives_a_restart`, `a_failed_delivery_into_a_durable_queue_is_recoverable_and_says_so` |
| degraded is visible | `test_run_reports_degraded_evidence_instead_of_inferring_success` |
| the local save is not best-effort | `a_rejected_local_save_is_degraded_not_success` |

## #1481 — the register, three class rules, one guard

`engine/.pi/architecture/sink-register.tsv`: 44 rows over process / fs-write /
network / env-read. Columns `sink, module, kind, mitigation, requires, does_not` —
`does_not` is mandatory, because the sentence that would have prevented all three
findings is the one nobody was forced to write.

`engine/.pi/scripts/validate-sink-register.sh` (gating, `.pi/guards.tsv`,
`run_here=yes`):

1. every boundary crossing maps to a row — an unregistered `Command::new`,
   `fs::write`, `TcpStream::connect` or `env::var` fails naming `file:line`;
   `mitigation=filtering` must name the deployment fact it relies on, and the value
   `isolation` is refused by name;
2. a bare `let _ =` under `audit/`, `orchestrator/`, `precondition/` fails unless it
   carries `// sink: <id>` naming a registered row;
3. an `fs-write` row mitigated by a `type` must name the handle it depends on.

`--self-test` seeds six defects and requires five rejections and one pass.

The rows are honest about the gaps they found, and each is a `does_not`, not a TODO:
`execution_engine`'s `sh -c` sink has no declared sandbox yet; `file-edit` /
`file-patch` still re-resolve between check and write (the SINK-2 shape);
`template-generation` turns a missing API key into a placeholder instead of failing
closed.

### Two mechanism decisions, stated

- **Rule 2 is denied at three module roots, not workspace-wide.** Denying
  `let_underscore_must_use` workspace-wide fires on ~50 pre-existing best-effort
  sites (event publishes, temp cleanup, channel sends), each needing an exception. A
  control that becomes a list of exceptions is noise. `Cargo.toml` records this.
- **Rule 3 is enforced by module registration, not `clippy::disallowed_methods`.**
  197 fs-mutation call sites exist, most in tests; a row per site would be the same
  noise. The module is the unit a reviewer can audit, and the type
  (`fs_at::ValidatedWritePath`) is the guard.

---

## What this does not do

- It does not implement a sandbox. It makes the deployment state which one exists,
  and refuses to run the sink when it does not.
- It does not fix `file-edit` / `file-patch`, which have the SINK-2 shape. The
  register says so in their `does_not` column; they are the next instance of the same
  class, not a closed one.
- It does not make the queue survive a full disk, and it does not deduplicate.
- It does not verify that a `RIGORIX_SANDBOX=container` deployment is actually
  containerised. A declaration is not a proof — it is a recorded assumption that now
  fails loudly instead of silently.

## Validation

```
cargo build --workspace                                        PASS
cargo test --workspace --all-targets                           PASS (engine 2309 + 27 binaries, cli 98, mcp, actions)
cargo clippy --workspace --all-targets -- -D warnings          PASS
cargo fmt --check                                              PASS
.pi/scripts/validate-guards.sh            (+ --self-test)      PASS
.pi/scripts/validate-docs.sh                                   PASS
.pi/scripts/validate-panic-sites.sh / validate-unsafe-sites.sh PASS (ratchets; two `.expect`s became typed errors)
engine/.pi/scripts/validate-{canonical,architecture,architecture-readiness,ci,tests,security,operations,integration}.sh   PASS
```

Closes #1478, #1479, #1480, #1481.
