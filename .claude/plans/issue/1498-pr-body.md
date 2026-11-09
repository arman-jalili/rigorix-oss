# #1498 (SINK-11) — a git step is a code-execution surface, so it declares its boundary

`exec_git_read` splits the step's `args` on whitespace and hands the argv to `git` through
`git_output` — program resolved in the engine's PATH *before* any child env, argv never a string,
environment cleared. All three are correct, and none of them constrains what git does with the
argv: **git's argv can name a program.**

Measured on the parent commit (`d834cc41`), with the argv the dispatcher builds from
`args: "-c alias.pwn=!touch pwn /tmp/x"`:

```
$ git -c alias.pwn='!touch /tmp/x/PWNED' pwn        → PWNED created
$ git -c alias.pwn='!curl pwn -s -o /tmp/x/f https://…'
                                                    → f created
```

A `!`-prefixed alias runs through a shell. `split_whitespace` is not a constraint on it: `!curl`
plus ordinary arguments is a download, `!base64`/`!tar` are exfiltration, `!sh` takes a single
argument. So a `git_read` step ran arbitrary programs in the engine's filesystem — and unlike
`run_command` it required **no** `RIGORIX_SANDBOX`. The asymmetry was the finding:

```
service_impl.rs:374   "run_command" => { match sandbox { Some → declared, None → refused } }
service_impl.rs:441   "git_read"     => { … no sandbox check … }
```

Which made ADR-017's amendment — *"contains one that cannot execute a `run_command` step"* —
true and materially misleading.

---

## The fix, in two halves that answer different questions

### 1. Filtering: the argument allowlist

`shell_policy::validate_git_read_args` (with `GIT_READ_SUBCOMMANDS`, `REFUSED_GIT_FORMS`):

- the subcommand must be one of 21 **read** subcommands (`log`, `show`, `diff`, `status`,
  `rev-parse`, `ls-files`, `ls-tree`, `blame`, `describe`, `shortlog`, `cat-file`, `show-ref`,
  `for-each-ref`, `grep`, `symbolic-ref`, `merge-base`, `name-rev`, `rev-list`, `branch`, `tag`,
  `whatchanged`). Anything else is refused naming the list — deny by default, so the dangerous
  forms are not an enumeration someone must keep complete;
- the forms that name a program are refused **by name, with the reason**: `-c` and its attached
  form (`-calias.x=!cmd`), `--config`, `--config-env`, `--exec-path`, `--upload-pack`,
  `--receive-pack`, `--paginate`, `--open-files-in-pager`, `--git-dir`, `--work-tree`, `ext::`.

A read step needs none of them, and the refusal says *which* one it was rather than "invalid
argument" — a refusal that teaches nothing gets worked around.

`git_stage` also gains `--` before its path, so a path beginning with `-` is a path.

### 2. The declaration: the gate is on the capability

`EXECUTING_STEP_KINDS = [run_command, git_read, git_stage, git_commit]`, checked before the
dispatch match **on both routes**, with one `undeclared_step_refusal` shared by all four kinds so
they cannot describe the same refusal differently. `exec_run_command_undeclared` is gone; its
test now exercises the shared function.

The half that stops this recurring is structural: `every_spawning_step_kind_is_gated` scans the
production section for `Command::new` / `git_output` call sites, maps each to its enclosing
function, and requires the kind to be in the list. Adding a git-spawning step kind without
gating it fails — the rule is a rule, not a habit. This is the same lesson as #1487 (a gate
keyed on a name) one level up.

## The record

A residual list is what someone relies on when deciding whether to deploy, so the fix ships with
its correction:

- **ADR-023 §6.4** rewritten: the primitive with the command that proves it, not the
  `core.pager` example (a pager needs a terminal, and a multiword `-c core.pager=…` is broken
  apart by `split_whitespace` anyway — the described risk was a different, smaller thing).
- **ADR-017's amendment** extends the "cannot execute" sentence to every spawning kind.
- **`engine:exec-git` row**: `requires` names the capability and the allowlist; `does_not` names
  what remains.
- `docs/deployment.md`, `CHANGELOG.md` (with the breaking consequence).

| # | Acceptance | Proof |
|---|---|---|
| 1 | `args: "-c alias.pwn=!touch pwn /tmp/x"` refused by name, nothing created | `a_git_read_step_cannot_name_a_program` — asserts `git_args_refused` **and** that the marker is absent |
| 2 | the `!curl` form refused before any spawn | same test, same assertion for all four refused forms (alias, attached config, `--exec-path`, `ext::`) |
| 3 | ordinary reads still work | `an_ordinary_git_read_step_still_runs` — `status --short` reaches git |
| 4 | a git step declares its boundary | `a_git_step_without_a_declaration_is_refused_before_any_spawn` (three kinds, each naming itself and `RIGORIX_SANDBOX`) + `every_spawning_step_kind_is_gated` |
| 5 | the record is accurate | ADR-023 §6.4 carries the measured command; `grep -n pager` finds no residual claim |

## Found while validating this PR: the Audit job was skipping

`engine/.pi/**` mapped to `engine,test,portable,conformance,coverage` — and none of those jobs
runs the guards. The **Audit** job does (`validate-guards.sh` executes every `run_here=yes`
guard, and `engine/.pi/scripts/validate-sink-register.sh` is one). So a change to the register,
or to the guard that reads it, narrowed the pipeline *away* from the only job that enforces it,
and CI stayed green because the job was skipped rather than passing. Reported by `gh pr checks`
on the first run of this PR (`🔒 Audit  skipping`).

Fixed in the same PR: `audit` added to the `engine/.pi/**` row, with the reason in the comment.
Same class as the defect this PR fixes — a control whose record says it runs, and a pipeline
that decided not to ask.

## What this does not do

- **It does not remove the workspace path.** An alias in the repository's `.git/config` or a hook
  in `.git/hooks` runs a program, and both are inside the agent-writable workspace — `.git` is not
  `.rigorix`, so the resolved-path protection does not cover them. The declared isolation is the
  boundary there. Refusing git entirely would be the only way to remove it, and then there is no
  git step. Recorded in ADR-023 §6.4 and in the row's `does_not`.
- It does not make `split_whitespace` a parser. The allowlist constrains the first token and the
  refused forms; git still receives a flat argv built from a string.
- Out of scope per the issue: `git_read_tool.rs`, which builds a fixed argv and is not reachable
  from a plan step.

## Validation

```
cargo test --workspace --all-targets --no-fail-fast        PASS (98 test binaries)
cargo test -p rigorix-engine --lib                         PASS (2341)
cargo clippy --workspace --all-targets -- -D warnings      PASS
cargo fmt --check                                          PASS
cargo audit                                                no advisories against the lockfile
.pi/scripts/validate-guards.sh (+ --self-test)              PASS
.pi/scripts/validate-unsafe-sites.sh / -panic-sites.sh      PASS (18 / 37, all baselined)
.pi/scripts/validate-docs.sh                                PASS
engine/.pi/scripts/validate-sink-register.sh (+ self-test)   PASS (46 rows, 87 hits, 11 seeds)
engine/.pi/scripts/validate-{ci,tests,security,operations,integration,canonical,architecture}.sh   PASS
.github/scripts/check_affected_map.sh                       PASS (43 rows, all 15 jobs)
.github/scripts/validate-affected-graph.sh                  PASS
```

CI: **27/27 passing**, including `🔒 Audit` (which was skipping before the map fix).

Closes #1498.