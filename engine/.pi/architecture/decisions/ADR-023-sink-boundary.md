# ADR-023: The sink boundary — what leaves the type system, and what each mitigation actually promises

**Status:** Accepted (implemented by SINK-1…SINK-10, #1478–#1495)
**Date:** 2026-11-09
**Supersedes:** nothing. **Superseded by:** nothing.
**Related:** ADR-006 (atomic write-rename — amended here), ADR-017 (consequence gating — amended
here), ADR-016 (audit integrity anchor), ADR-012 (identity attestation), ADR-014 (effect
identity), ADR-022 (the binding boundary)

## Context

Twenty-two ADRs describe how the engine *decides* things: which tool, which identity, which
capacity, which gate. None described where the engine stops deciding and the operating system
starts — `Command::new`, `openat`, `TcpStream::connect`, `env::var`. That is where every defect
of the last two release cycles lived:

| defect | sentence it contradicted |
|---|---|
| #1478 | `run-command` "filtered" commands; the sandbox it relied on was a source comment |
| #1479 | `file-write` "validated" a path, then resolved it again to mutate |
| #1480 | the evidence path "recorded" a result it dropped |
| #1487 | the dispatcher's only gate was `Option`, read as "skip the check" |
| #1489 | the allowlist compared an executable *word*; the kernel ran a *file* |
| #1493 | `.rigorix` was protected by a substring test; the write followed a symlink into it |
| #1494 | one `atomic` row described five call sites with three guarantees |
| #1495 | the resolver "pinned" the file; it pinned the path |

The register (`.pi/architecture/sink-register.tsv`) made the boundary enumerable. A TSV can
carry a fact but not a rationale, so the taxonomy lives here and the register cites this ADR.

## Decision

### 1. The register is one row per call site, and every call site names its row

A row states: which sink, in which module, of which kind, mitigated how, relying on what, and —
the column no reviewer was ever forced to write — what it does **not** do. A production call
site carries `// sink: <id>` naming its row; the guard
(`engine/.pi/scripts/validate-sink-register.sh`, rule 5) refuses a hit without a claim, and an
anchor naming a row for another module or kind describes nothing.

The unit is deliberately *not* the file. #1494 is the proof: `tools.rs` had five write sites,
one row said `atomic`, one of the five was atomic. A row written about a call site cannot make
that mistake, because the next call site has no row to inherit.

### 2. The mitigation kinds, and what each actually guarantees

| kind | guarantees | does not guarantee |
|---|---|---|
| `filtering` | a matching decision was made about what may proceed, and it is enforced where the call is made | anything about what the admitted operation does afterwards |
| `type` | the wrong use does not compile — the resolved handle is the only way to mutate | anything about a path that is resolved elsewhere |
| `handle` | the inode mutated is the inode validated (`openat`/`renameat`, `fchdir`) | that the directory's owner is trustworthy |
| `atomic` | a reader sees the old bytes or the new bytes, never a mixture | durability beyond `fsync`, or anything on a non-POSIX filesystem |
| `env-clear` | the child's environment is constructed, not inherited | that the child cannot read the same secrets another way |
| `trust-boundary` | a verified principal or authority gated the operation | that the operation's inputs were verified |
| `retry+queue` | a delivery failure has a durable second chance | that the durable queue survives its own disk |
| `operator-declared` | the deployment stated a fact the engine cannot check | that the statement is true |
| `none` | nothing mitigates this crossing | — and a `none` row must cite the issue that owns the fix, or it becomes permanent |

**`isolation` is not a value in this vocabulary.** Isolation is a deployment property; the
engine records *which* fact it relies on (`RIGORIX_SANDBOX=container|vm`) in `requires` and
states in `does_not` that it did not verify it. Recording isolation as a mitigation is how
filtering came to be described as a boundary (#1478).

### 3. The reachable path is the subject, not the module

Learned twice, in the same shape both times:

* `RunCommandTool` was hardened to require a `SandboxDeclaration` (#1478) while the
  **dispatcher** that runs every plan step executed `sh -c` with an optional gate (#1487);
* `file_write` was routed through `fs_at` (#1490) while `file_append`, `file_patch` and
  `edit_file` in the same file wrote paths (#1494).

So: **register and harden the call site.** "The module is safe" is a statement about the call
sites that existed when someone looked.

### 4. One owner per sink

A register row is a description; a finding needs a ticket. `mitigation: none` must cite the open
issue that owns the fix (#1490's rule, guard rule 4), and a residual that this ADR accepts must
be named here with the reason it was accepted — as §6 does.

### 5. Fail-closed defaults

| question | default |
|---|---|
| a `run_command` step with no `RIGORIX_SANDBOX` | refused (`sandbox_undeclared`) before any spawn, naming the reach |
| `RIGORIX_SANDBOX=none`, or an unrecognized value | refused at construction |
| an unrecognized `RIGORIX_PERMISSION_MODE` | a startup error naming the accepted set, not `WorkspaceWrite` |
| a permission enforcer that cannot be built | the host does not start; a dispatcher without one refuses every step |
| a mutable write on a non-unix target | refused — a path fallback *is* the defect (#1479) |
| the file's mode requires a decision | the engine refuses rather than guessing |

### 6. Residuals this ADR accepts, with reasons

1. **`sh -c` grain.** The dispatcher's `run_command` executes a shell string. Its boundary is
   the declared deployment isolation; a shell resolves its own argv, so that path cannot record
   the resolved binary. Row: `engine:exec-run-command`.
2. **Path, not inode, for spawned programs.** `resolve_program` pins the path; `execve` opens it
   again (#1495). Accepted because `tokio::process::Command` execs by path, and the
   alternatives (`execveat`, `/proc/self/fd`) change the spawn machinery or the platform
   behaviour for a window that requires write access to the binary's directory. The fix, if the
   cost is ever worth it, is `execveat` on a descriptor opened in `resolve_program` — recorded
   here so the decision is visible rather than the omission being invisible.
3. **The trusted base is trusted.** `fs_at::resolve_under` canonicalizes its base once, because
   an `O_NOFOLLOW` walk from `/` is impossible on macOS (`/var`, `/tmp` are links). A base a
   caller can name must not be passed to it; the two functions document the difference.
4. **`git_read`'s argument string is not parsed.** `git -c core.pager=…` runs a program, and
   `split_whitespace` is not a parser. The row says so; the argv is the caller's.

### 7. What does not need an ADR

WASI refinements belong in ADR-022's amendment log (§6 there already carries the clock story). A
fix reversible by editing one function — #1495's resolve-then-exec, a `file_append` routing — is
code, a register row and a CHANGELOG line. Release-train membership is release-note material
(ADR-011 does not enumerate members).

## Consequences

### Positive
- The boundary is enumerable: `bash engine/.pi/scripts/validate-sink-register.sh --list`.
- Every sink states what it does not do, in the place a reviewer reads.
- A new call site fails the guard instead of inheriting a neighbour's guarantee.

### Negative
- 94 `// sink:` anchors in `engine/src`. The cost is deliberate: the anchor is the claim, and a
  claim without a line is what the previous register was.
- Splitting a row is now a code change in the module it describes, not a register edit alone.

## Implementation

- `engine/.pi/architecture/sink-register.tsv` — the rows, citing this ADR.
- `engine/.pi/scripts/validate-sink-register.sh` — rules 1–5, and a self-test that seeds each.
- `engine/.pi/architecture/modules/tool-system.md#sink-register` — the reader-facing summary.
- `docs/deployment.md` — what a deployment must supply (`RIGORIX_SANDBOX`) and what it may rely on.

---

*Decision date: 2026-11-09*