# Batch 3: make the boundary's check, mutation and record agree

Batch 1 recorded the boundary. Batch 2 hardened the paths a plan actually reaches. This batch
closes the three gaps the record still described and gives the boundary the ADR it never had.

All four issues are the same question from four angles: **is the record of the boundary true of
the boundary?** A check that judged a string the write did not use (#1493), a row that claimed
`atomic` for five call sites (#1494), a comment that promised an inode (#1495), an ADR that
stated a default the code no longer has (#1496).

| Issue | ID | What was actually wrong |
|---|---|---|
| #1493 | SINK-9 | `.rigorix` protection was a substring test; the write resolved through symlinks — `ln -s .rigorix tmp` installed an executable hook |
| #1494 | SINK-8 | the dispatcher's `append`/`patch`/`edit` were raw path writes, and one register row covered five sites with three guarantees |
| #1495 | SINK-10 | `resolve_program` pins a path; `execve` opens a path; the comment promised the file |
| #1496 | ADR-1 | the boundary had no ADR, ADR-006's scope had moved, ADR-017 stated a default the dispatcher no longer has |

---

## #1493 — protection is decided on the path the write lands at

Two halves of one chain, closed independently.

**The check.** `is_protected_config_path` judged the requested string; `resolve_absolute_for_write`
then `create_dir_all(parent)` + `parent.canonicalize()` — which follows links. Measured, with the
functions verbatim: `ln -s .rigorix ws/tmp` from a `run_command` step, then `file_write` to
`ws/tmp/hooks.toml` passes the check and lands in `.rigorix/hooks.toml`. That file is a command
list the next host start runs with `sh -c` and the engine's privileges, before and after every
tool call. `protected_config_refusal` now decides on the **effective** path: the requested string
first (exact for the ordinary case, and it names what was asked for), then the deepest existing
ancestor resolved once with the remainder appended, with the segment rule applied to the result.
Following links in a *check* is correct — the thing to recognise is where the write lands; the
mutation still does not follow them.

**The write.** `fs_at::resolve_under(base, path)` walks every component below a trusted base with
`openat(O_DIRECTORY|O_NOFOLLOW)`, refuses `..` outright, and refuses an absolute path outside the
base by name. The base is canonicalized once and may be created — it came from configuration, not
from the caller. An `O_NOFOLLOW` walk from `/` is not an option (macOS `/var` and `/tmp` are
links), which is why the base exists rather than a global root. `resolve_absolute_for_write` is
now a delegation whose doc states its limit: its base is derived from the same string, so it must
not be given a path a caller can name.

The dispatcher's four file steps resolve under the engine's working directory, so the alias fails
at the write even if the enforcer's decision were reverted.

**Also fixed in the same function:** `is_within_workspace` canonicalized a *relative* path against
the process's working directory rather than the workspace root — right for a CLI started in the
repo, silently wrong everywhere else, including every test.

| # | Acceptance | Proof |
|---|---|---|
| 1 | `ln -s .rigorix ws/tmp` then write `ws/tmp/hooks.toml` | `test_a_symlinked_alias_of_rigorix_is_denied_like_rigorix` — denied, the reason names `.rigorix`, nothing created |
| 2 | direct `.rigorix/hooks.toml` | same test's sibling assertions; `test_workspace_write_denies_rigorix_config_writes_by_default` unchanged |
| 3 | the seed (revert the fix) | criterion 1's first assertion fails — that is what makes it a seed |
| 4 | the siblings: `sequence-policy.toml`, `approvals.json`, `effect-commitment.toml` | all four aliases in the same loop |
| — | the resolved check invents nothing | `test_the_resolved_check_does_not_invent_refusals`: an alias of an ordinary directory stays writable; one outside is denied for being outside |

## #1494 — the reachable writes go through a handle, and a row is written about a call site

`file_write` was routed through `fs_at` in #1490; its three siblings in the same file were not:

```
tools.rs:459   OpenOptions::new().append(true).create(true).open(path)   // follows a symlink leaf
tools.rs:593   tokio::fs::write(path, &new_content)                     // file_patch, anchor mode
tools.rs:692   tokio::fs::write(path, &new_content)                     // file_patch, search mode
tools.rs:786   tokio::fs::write(path, &updated)                         // edit_file
```

Append wrote *through* a link — reproduced: `victim.txt` gained `INJECTED` — and `fs::write`
truncates in place, so a concurrent reader saw partial files: the failure `write_atomic` exists to
remove, still present three branches away from the branch where it was fixed.

- All four resolve once and mutate the handle: `write_atomic`, `append_or_create` (new —
  `O_APPEND|O_CREAT|O_NOFOLLOW`, so the step keeps its create-if-missing contract and still refuses
  a link; `append_to_file` keeps the existing-file contract `FileAppendTool` documents), and
  `read_file` for the bytes an edit transforms.
- `grep -c 'tokio::fs::write\|OpenOptions::new()' tools.rs` is 0 outside tests, asserted by a
  source-scan test anchored on the file the fix is in.

**Found while splitting the row:** `tools.rs` had **six** `Command::new` sites under **one** row
whose `requires` claimed the sandbox declaration covered them. It covered the `run_command`
branch; the five `git` spawns inherited the engine's **whole environment — `RIGORIX_HMAC_KEY`
included**, the key that signs the envelopes meant to constrain them (#1068, #1082 C3) — and
resolved `git` through that inherited `PATH`, which is #1489's word-vs-file defect one layer up.
They now go through one `git_output` helper: program resolved in the engine's PATH before any
child env, argv (never a string), `apply_child_env` with no overrides.

**The guard learns the rule the defect hid behind.** Rule 5: every production sink names its row
with `// sink: <id>` on or above the line; an anchor naming a row for another module or kind
describes nothing. Required *everywhere*, not only where a module already has two sinks — that is
what makes a new sink in a registered file fail, because the row it would inherit was written
about the site that was there.

| # | Acceptance | Proof |
|---|---|---|
| 1 | no path writes | `criterion_one_of_sink_8` (source scan) |
| 2 | append refuses a symlink leaf | `a_file_append_step_refuses_a_symlink_leaf` (victim unchanged) + `append_or_create_creates_the_file_and_refuses_a_link`; control: `an_ordinary_file_append_step_still_appends_and_creates` |
| 3 | edit/patch atomic | `an_edit_never_exposes_a_truncated_file` — a reader thread samples a 2 MB file during the edit and must never see a length other than old or new |
| 4 | a new write in a registered file fails, naming the line | proved on the real tree: adding `std::fs::write` to the dispatcher produced `tools.rs:1114: unclaimed fs-write sink`; seeds H (unclaimed second site) and I (anchor naming another module's row) |
| 5 | the git child has no signing key | `a_git_step_child_does_not_inherit_the_signing_key` |

## #1495 — name the gap, do not claim the inode

Declined `execveat`: `tokio::process::Command` execs by path, so pinning would replace the spawn
machinery or add a per-platform `/proc/self/fd` trick — on a `low` issue whose own acceptance says
the honest sentence is sufficient. The residual is now a decision with a place to point at:

- `resolve_program`'s doc says it pins the **path**, names `execveat` as the fix if the cost is
  ever worth it;
- `ToolResult::resolved_executable` says it is not a fingerprint, and a reader who needs the bytes
  needs a digest;
- `tools:run-command-spawn` and `engine:exec-git` `does_not` columns state the swap window.

| # | Acceptance | Proof |
|---|---|---|
| 1 | the row says it | `awk -F'\t' '$1=="tools:run-command-spawn"{print $6}'` contains "inode" |
| 2 | the comment does not overclaim | `the_resolver_does_not_claim_to_pin_the_file` reads the doc comment and fails if the old sentence returns |

## #1496 — the boundary gets an owner

**ADR-023 — the sink boundary (new).** The register was the only place the taxonomy existed, and a
TSV cannot carry a rationale. Records: the mitigation kinds and what each guarantees (and does
not); that `isolation` is not one of them — it is a deployment property, recorded in `requires`
with a `does_not` saying the engine did not verify it; that **the reachable path is the subject**
(learned twice in the same shape — a hardened tool no host constructs while the dispatcher ran
`sh -c`; a routed `file_write` while three siblings wrote paths); one owner per sink; the
fail-closed defaults in one table; and **§6, the residuals this design accepts, with reasons** —
the `sh -c` grain, path-vs-inode, the trusted base, `git_read`'s argument string.

**ADR-006 — amendment.** It decided atomic write-rename for *engine-owned state* against a crash.
`fs_at` extends the property to *caller-supplied trees with an adversary on the other side*, which
is a different threat model — atomicity does not address a component swapped between check and use
at all. Records the extended rule and the corollary this cycle finally wrote down: **a check and a
use in separate events are not a control.**

**ADR-017 — amendment, mandatory.** Keeps the default caveat **verbatim** (the file's own
maintenance note says never delete or weaken it), then states what changed and what did not: path
is stronger for `.rigorix` (resolved-path refusal), the hook half is conditional on that refusal,
and `RIGORIX_SANDBOX` is a declaration, not a proof. A reader can no longer conclude that a
default install contains an agent that cannot execute code.

| # | Acceptance | Proof |
|---|---|---|
| 1 | the boundary has an owner | `ADR-023-sink-boundary.md` exists; the register header cites it |
| 2 | no ADR contradicts the code | the ADR-017 amendment quotes the caveat and states the dispatcher's new default |
| 3 | the taxonomy is one place | ADR-023 §2 enumerates the kinds the register uses |
| 4 | the guard names the rule | seed C2 invents a `seccomp` kind → refused, naming `ADR-023-sink-boundary.md §2` |

---

## What this does not do

- It does not implement a sandbox and does not verify `RIGORIX_SANDBOX`. It makes the deployment
  state the fact and refuses the step when it does not (ADR-023 §6.1).
- The dispatcher's `run_command` still executes a shell string. A shell resolves its own argv, so
  that path cannot record the resolved binary the way `run_command_tool` does.
- It does not pin the executable's inode (#1495, decided above). A writer in the binary's
  directory still has a window between resolve and exec.
- `git_read`'s argument string is still `split_whitespace`d into argv. `git -c core.pager=…` runs a
  program; the row says so rather than pretending the argv is parsed.
- The trusted base is trusted: `resolve_under` canonicalizes it once. A base a caller can name must
  not be passed to it — both functions document the difference rather than enforcing it
  structurally.
- **`git` steps lose `GIT_*` and `SSH_AUTH_SOCK`.** The child now gets the standard sink allowlist
  (`PATH`, `HOME`, `LANG`, `LC_ALL`, `TMPDIR`, `SHELL`, `TERM`). A deployment whose commits or
  pushes authenticate through the agent will need that configured elsewhere. This is a deliberate
  fail-closed consequence, recorded in `CHANGELOG.md`.
- Not fixed, and not in this batch: `file_read` still opens a caller path directly (a read, not a
  mutation), and `hooks:hook-shell` remains a `sh -c` sink whose boundary is the resolved-path
  refusal plus the deployment.

## Validation

```
cargo build --workspace                                       PASS
cargo test --workspace --all-targets --no-fail-fast           PASS (98 test binaries)
cargo test -p rigorix-engine --lib                            PASS (2337)
cargo clippy --workspace --all-targets -- -D warnings         PASS
cargo fmt --check                                             PASS
cargo audit                                                   no advisories against the lockfile
cargo check -p rigorix-engine --features portable             PASS
validate-portable-inventory.sh --gates                        PASS
.pi/scripts/validate-guards.sh (+ --self-test)                 PASS (gating + 11 seeds)
.pi/scripts/validate-unsafe-sites.sh                           PASS (18 sites, all baselined)
.pi/scripts/validate-panic-sites.sh                            PASS (37 sites)
.pi/scripts/validate-docs.sh                                   PASS
.pi/scripts/validate-script-drift.sh                           100 files, 0 unrecorded
engine/.pi/scripts/validate-sink-register.sh (+ --self-test)    PASS (46 rows, 87 hits, 11 seeds)
engine/.pi/scripts/validate-{ci,tests,security,operations,integration,canonical,architecture,architecture-readiness}.sh   PASS
```

`gitnexus detect_changes` against `main`: 50 files, 107 symbols, **medium** risk — the bulk of the
symbol churn is the 94 `// sink:` anchors, which change no behaviour. The two behavioural changes
on a shared path are the dispatcher's write routing (four steps) and `is_within_workspace` (every
`file_write` permission decision).

Closes #1493, #1494, #1495, #1496.