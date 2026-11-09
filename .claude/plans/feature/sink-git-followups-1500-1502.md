# feature/sink-git-followups-1500-1502 — three findings about one filter

Component batch: `engine/src/tools/application/shell_policy.rs` plus the two record lines all
three issues cite. Branch `feature/sink-git-followups-1500-1502`.

## Order (not priority order)

`#1500` is high priority and lands second: its deliverable is a table test over the refusal
tables and the allowlist, and a table written against the old allowlist would have frozen
`symbolic-ref`, `branch -D`, `tag -d` and `--output` as expected reads.

| # | Issue | Commit | What it settles |
|---|---|---|---|
| 1 | #1501 | `3e12c039` | the allowlist answers "writes" as well as "executes"; `-c` judged by position |
| 2 | #1500 | `8d519ea1` | the filter has a subject, and the filter is not the containment |
| 3 | #1502 | `5cc4390a` | the record names the class (`core.fsmonitor`, `diff.external`), not one key |

## Measured (git 2.50.1), before any code

| Vector | Result | Which issue |
|---|---|---|
| `git symbolic-ref HEAD refs/heads/extra` | HEAD moved — allowlisted | #1501 |
| `git tag v1` / `git tag -d v1` | creates / deletes — allowlisted | #1501 |
| `git branch newb` | created — allowlisted | #1501 |
| `git log --output=<abs path>` | **wrote the file** — allowlisted, no register row | #1501 |
| `git grep -c TODO` | exits 0 — refused by the word-scan | #1502 |
| `git status -c k=v` | `unknown switch 'c'` — `-c` is only global in the lead | #1502 |
| `git config core.fsmonitor <s>` + `git status` | script **ran** | #1500, #1502 |
| `git config diff.external <s>` + `git diff` | script **ran** (needs a real change) | #1500, #1502 |
| `git config diff.d.textconv <s>` + `git diff` | **did not** fire on 2.50.1 — not claimed | — |

## Validation

```
cargo build --workspace                                    PASS
cargo test --workspace --all-targets --no-fail-fast         PASS (98 test binaries)
cargo test -p rigorix-engine --lib                          PASS (2350)
cargo clippy --workspace --all-targets -- -D warnings       PASS
cargo fmt --check                                           PASS
cargo audit                                                 no advisories
.pi/scripts/validate-guards.sh (+ --self-test)               PASS
engine/.pi/scripts/validate-sink-register.sh (+ self-test)    PASS (46 rows, 87 hits, 11 seeds)
.pi/scripts/validate-{unsafe,panic}-sites.sh                 PASS (18 / 37)
.pi/scripts/validate-script-drift.sh                         PASS (0 unrecorded)
.pi/scripts/validate-docs.sh                                 PASS
engine validators (ci, tests, security, operations, integration, canonical, architecture)   PASS
.github/scripts/check_affected_map.sh                        PASS
```

## Seeds run against the new tests

| Seed | Expected | Observed |
|---|---|---|
| remove `--paginate` row | red, naming the set | red (`every_refused_form_is_refused_and_named`) |
| remove `--output` row | red | red |
| remove `-e` row | red | red |
| add `"pwn"` to the allowlist | red | red (first version of the test was green here — fixed) |
| drop the `core.fsmonitor` vector | red | red |

## What this does not do

- It does not stop a repository-named program. `.git` is inside the agent-writable workspace;
  that is what the declared isolation is for, and both the row and ADR-023 §6.4 say so.
- It does not give `git_read` a structured argv: `split_whitespace` is still the parser, and the
  allowlist constrains the first word and the refused forms.
- `git_read` still cannot express `branch <name> --list`-style combinations that need a
  positional grammar; the selector is the smaller requirement, and the remedy is `for-each-ref`.
