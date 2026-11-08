# Batch plan — `feature/sink-register-1478-1481`

Four issues, one shape: **a control is present and is not the control the sentence claims.**
Landed on one branch, instances first, mechanism last, so the register (#1481) can
record mitigations that actually exist in the code.

| Order | Issue | Priority | Component | Why here |
|---|---|---|---|---|
| 1 | #1478 `SINK-1` | high | `tools/application/{run_command_tool,shell_policy}.rs` | introduces the declared sandbox fact the register row cites |
| 2 | #1479 `SINK-2` | medium | `tools/application/file_write_tool.rs` + new `tools/infrastructure/fs_at.rs` | replaces re-resolution with a pinned handle the register can name |
| 3 | #1480 `EVID-1` | high | `orchestrator/...`, `audit/application/*` | makes "no dropped evidence" true before the rule that forbids it |
| 4 | #1481 `SINK-3` | high | `.pi/architecture/sink-register.tsv`, `.pi/scripts/validate-sink-register.sh` | records the other three; its own acceptance requires rows to match reality |

Dependency notes:
- #1481's acceptance #5 ("the register matches reality — every claimed mitigation found
  in the named file") is only satisfiable *after* #1478/#1479/#1480 exist. Hence the order.
- No issue blocks another's branch; all four touch disjoint files except
  `tool-system.md` (documentation, touched once at the end).

---

## #1478 · run-command filters; the isolation is a source comment

Files: `engine/src/tools/application/shell_policy.rs`,
`engine/src/tools/application/run_command_tool.rs`,
`engine/src/tools/application/registry_impl.rs` (test ctor), new `docs/deployment.md`.

1. `shell_policy::SandboxDeclaration` — the declared isolation fact.
   - `RIGORIX_SANDBOX` ∈ {`none`, `container`, `vm`}; unset / unrecognized → refused.
   - `none` → refused at construction with the reach named ("runs as the engine's user,
     with the engine's filesystem access").
   - It is the type `RunCommandTool` cannot be built without: no declaration, no sink.
2. `shell_policy::argv_from_simple` — split a proven-single command into argv.
   - quoting: `'…'` literal, `"…"` literal except `\"`/`\\`, `\` escapes one char;
   - refuse: unbalanced quote, trailing `\`, empty command, and any character that a
     shell would expand (`$` backtick `*` `?` `[` `]` `{` `}` `~` `!`) — outside quotes
     always, inside double quotes for `$`/backtick only (single quotes expand nothing).
3. `RunCommandTool::execute` runs `argv[0]` + `argv[1..]` — no `sh`, no `-c`.
   The allowlist's grain becomes explicit: it matches the **executable**, arguments are
   unconstrained (so `cargo`/`npm`/`python` remain arbitrary execution — now said out loud).
4. `docs/deployment.md` carries the paragraph that lived at `shell_policy.rs:23-28` plus the
   executable → what it can do → safe-to-allowlist table.

Acceptance: unset `RIGORIX_SANDBOX` ⇒ construction refused naming the reach; the doc's
paragraph present; a test asserting the source has no `arg("-c")` on the command path.

## #1479 · file-write validates, then re-resolves

Files: new `engine/src/tools/infrastructure/fs_at.rs`, `file_write_tool.rs`,
`tools/infrastructure/mod.rs`.

1. `fs_at::resolve_for_write(root, rel) -> ValidatedWritePath` — walks each component with
   `openat(…, O_DIRECTORY|O_NOFOLLOW)`, `mkdirat` for a missing component, and returns the
   **opened parent directory handle** + the final name. A symlinked component is refused,
   not followed. Fields are private: the only consumer is this module.
2. `fs_at::write_atomic(&ValidatedWritePath, bytes)` — temp file created in the pinned
   directory (`openat` `O_CREAT|O_EXCL|O_NOFOLLOW`), written, `fsync`, then `renameat`
   **within that handle**. The rename cannot cross a directory the check never saw.
3. `fs_at::append_to_file(&ValidatedWritePath, bytes)` — `openat` `O_APPEND|O_NOFOLLOW` on
   the pinned parent, so `file-append` stops re-resolving too.
4. `file_write_tool.rs` keeps only policy (traversal refusal, `ToolError` mapping) — it can
   no longer hold a `PathBuf` and call `fs::write` on it, because the resolve returns a type
   it cannot destructure.

Acceptance: no `fs::write`/`create_dir_all` in `file_write_tool.rs`; a test that swaps a
component for a symlink between validate and mutate is refused; an existing symlink
component yields `PathDenied`.

## #1480 · the evidence path drops five results

Files: `audit/application/{service.rs,dto/mod.rs,audit_service_impl.rs,audit_queue_impl.rs}`,
`orchestrator/application/{dto/mod.rs,orchestrator_impl.rs,orchestrator_mocks.rs}`,
`actions/.../router_impl.rs` (struct literal sites).

1. `#[must_use]` on `AuditService::build_and_send`, `AuditQueue::enqueue`,
   `AuditEnvelopeRepository::save`; `clippy::let_underscore_must_use` denied workspace-wide.
2. `BuildEnvelopeOutput.degraded: Option<DegradedEvidence>` — set when the local save fails,
   when delivery fails and the queue rejects, and when delivery failed but was queued.
3. `RunOutput.evidence: EvidenceStatus { NotRequested, Complete, Degraded { reason, undelivered } }`
   — the orchestrator consumes the outcome instead of `let _ =`.
4. `AuditQueueImpl::durable(dir, capacity)` — one JSON file per pending envelope, written
   atomically into `<dir>`, re-read on construction. In-memory remains the default and its
   loss window is stated in the trait docs and `status()`.

Acceptance: dropping the result fails to compile; enqueue → new instance → still pending;
audit backend unreachable ⇒ `Degraded`; unwritable `.rigorix/audit` ⇒ `Degraded`.

## #1481 · the register, three class rules, one guard

Files: `engine/.pi/architecture/sink-register.tsv`,
`.pi/scripts/validate-sink-register.sh`, `.pi/guards.tsv`,
`engine/.pi/architecture/modules/tool-system.md`, `clippy.toml`/workspace lints.

1. Register: `sink, module, kind, mitigation, requires, does_not` — `does_not` mandatory,
   because the missing sentence is what produced all three findings.
2. Guard `validate-sink-register.sh` (gating, `run_here=yes`):
   - every `Command::new` / `TcpStream::connect` / `fs::write|rename|create_dir_all|remove_file`
     / `env::var` hit in `engine/src` maps to a row by module; an unregistered hit fails
     naming `file:line`;
   - a row without `does_not` fails; `mitigation=filtering` must name a `requires`
     (deployment fact) and may not read as isolation;
   - `--self-test` plants an unregistered `Command::new` and a filtering-row-claims-isolation
     and requires both to be rejected.
3. Rule 2 enforcement: `#[must_use]` (from #1480) + workspace `let_underscore_must_use = "deny"`
   + the guard refusing a bare `let _ =` on the evidence path unless the line names a sink id.
4. Rule 3 enforcement: fs mutation registered at module granularity (197 call sites, mostly
   tests — an allow on each would be noise; the module is the unit a reviewer can audit),
   with `tools/infrastructure/fs_at.rs` the only module allowed to *re-resolve* a path.

Acceptance: an unregistered new sink fails; a dishonest `isolation` row fails; a dropped
evidence result fails the build; a new fs-mutating module fails; every row's mitigation is
findable in the file it names.

---

## Validation gate (Phase 6)

```bash
cargo build --workspace
cargo test --workspace --lib
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
bash .pi/scripts/validate-sink-register.sh --self-test
bash .pi/scripts/validate-guards.sh
bash engine/.pi/scripts/validate-canonical.sh
bash engine/.pi/scripts/validate-architecture.sh
```

## MR body
`.claude/plans/feature-sink-register-1478-1481-pr-body.md` — four sections (one per issue),
the acceptance table from each issue with the command that proves it, and the
"what this does not do" paragraph the series itself is about.
