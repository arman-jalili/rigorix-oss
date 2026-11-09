# Three findings about one filter: writers in the allowlist, an untested table, and a record that names the wrong trigger

Follow-ups to #1498 (SINK-11). All three are about `validate_git_read_args` — the filter that
decides what a `git_read` step may ask git to do — and about the two record lines that describe
it. Delivered together because they are one mechanism: #1501 changes the allowlist, #1500 freezes
it with a test, #1502 fixes the sentence.

Measured on git 2.50.1 before any code was written. Every row below was **allowed** by the
validator that merged in #1498:

| Vector | Result |
|---|---|
| `git symbolic-ref HEAD refs/heads/extra` | HEAD moved |
| `git branch -D main` / `git tag -d v1` | the branch / tag was deleted |
| `git branch newb` | the branch was created |
| `git log --output=/tmp/x` | **wrote the file**, anywhere the engine's uid can write |
| `git grep -c TODO` | exits 0 — and was refused, with a message about config values |
| `git config core.fsmonitor <script>` then `git status` | the script **ran** |
| `git config diff.external <script>` then `git diff` | the script **ran** |
| `git config diff.d.textconv <script>` then `git diff` | did **not** fire on 2.50.1 — not claimed anywhere |

---

## #1501 — a read cannot write

`GIT_READ_SUBCOMMANDS` was documented as *"reads, and nothing that executes"*, and executing was
the only question it answered. Writing is a different question, and four entries answered it
wrongly. `symbolic-ref` is the sharp one: the step's stated purpose is reading and it moves the
repository's HEAD, so the next `git_commit` commits to another branch, or to none. `--output`
writes anywhere the engine's uid can write — a write sink with no register row, because the write
happens inside git and the row says `filtering`.

A step kind is part of the consequence gate's input (ADR-017): a step called `git_read` presents
as a read to every policy that looks at the name, so the name has to be true. Three changes,
ordered by how much they have to be trusted:

* **`REFUSED_GIT_READ_FLAGS`** — long options refused for every read: `--output`, `--ext-diff`
  (names the program the repository configures), and the ref-writing long forms.
* **`REFUSED_REF_WRITE_FLAGS`** — short flags refused **only** for `branch`, `tag`,
  `symbolic-ref`. Short letters are family-specific in git, and refusing them globally is the bug
  in the other direction: `-c`, `-C` and `-m` are read flags for `log`, `show` and `diff`.
* **`GIT_READ_FORM_SPECS`** — `branch`/`tag` must **name** their read form (`--list`,
  `--contains`, …), because `branch` lists and `branch newb` creates. The selector is an *allow*
  marker: a read flag nobody listed causes a refusal, not a write.

The validator was also restructured around git's real grammar — global options precede the
subcommand — which is #1502's finding, fixed here because it is the same table. `-C` and
`--namespace` join the refused set; the doc had already claimed `--namespace` without refusing it.

## #1500 — the filter has a subject

`cargo test -p rigorix-engine --lib validate_git_read` ran **0 tests**. Under this project's own
rule, a check that cannot show its property is vacuously satisfied.

Two tests, and **both first drafts were wrong in the way that matters**:

* *The tables.* Iterating `REFUSED_GIT_FORMS` and asserting each row is refused is vacuous:
  delete a row and its case disappears with it. Measured — removing `--paginate` left it green. The
  expectation now lives in the test and is compared to the tables as sets, both directions.
* *The allowlist.* One `vec![subcommand]` per entry passes trivially — seeding `"pwn"` left it
  green, because any single word is a valid argv once allowlisted. Each subcommand now has an argv
  the test owns, verified against git 2.50.1, and the comparison is set equality.

Then the part that decides how much the filter is worth: **the repository names the program, not
the argv.** Two tests hold that property from both ends — the unit test runs the `core.fsmonitor`
vector and asserts the script *did* run; the step test asserts the same repository is refused
**before any spawn** when no isolation is declared, marker absent. Containment is the declaration;
the filter is what makes the ordinary case expressible, not what makes it safe.

## #1502 — the record, and `-c`

`-c` was refused as a word, so `git grep -c` (count) and `git log -c` (combined diff) were refused
with a message about setting a config value. The refusal was defensible; the *explanation* was
wrong. git's grammar settles it: `git -c k=v status` sets a config value, `git status -c k=v` is
`unknown switch 'c'`, and after the subcommand `-c` is that subcommand's own flag. The table is
consulted in the leading position only, and the refusal now says which position it means and
suggests `--count` when that was the intent.

The record's examples named *"an alias in `.git/config` and a hook in `.git/hooks`"* — the two keys
that require the step to name something, which reads as *"safe unless the step names an alias"*.
The measured trigger is an ordinary read subcommand. The row and ADR-023 §6.4 now name the class
with the measured commands beside it. This is the sentence #1500 asks for in its end state too, so
it lands once and is cited by both.

| # | Acceptance | Proof |
|---|---|---|
| 1 | HEAD cannot be moved; refs cannot be deleted | `a_read_step_cannot_write` — 14 measured write forms refused |
| 2 | `--output` is not a write path | refused by name; the register row's `requires` states the write clause |
| 3 | real reads still read | `an_ordinary_read_still_reads` — 21 argv lists, incl. `grep -c` and `log -m` |
| 4 | the filter has a subject | `every_refused_form_is_refused_and_named` + `every_allowed_subcommand_has_a_case_the_test_owns` |
| 5 | a removed form is caught | seeds: `--paginate`, `--output`, `-e` → red |
| 6 | a new subcommand is deliberate | seed `"pwn"` → red |
| 7 | the vectors are recorded | `the_filter_does_not_see_a_program_the_repository_names` (script runs) + `the_repository_names_the_program_not_the_argv` (refused before spawn, marker absent) |
| 8 | the record names the class | `awk -F'\t' '$1=="engine:exec-git"{print $6}' sink-register.tsv` — fsmonitor/diff.external named |
| 9 | the ADR agrees | ADR-023 §6.4 carries the same class and the same measured commands |

## What this does not do

- **It does not stop a repository-named program.** `.git` is inside the agent-writable workspace,
  so the resolved-path protection does not cover it; the declared isolation is the boundary, and
  both the row and the ADR say so. The only way to remove it is to remove git steps.
- It does not give `git_read` a structured argv. `split_whitespace` is still the parser; the
  allowlist constrains the first word and the refused forms.
- `branch`/`tag` must name their read form, so `git branch foo --list`-shaped combinations are not
  expressible; `for-each-ref` covers the read.
- Out of scope, as filed: `git_stage`/`git_commit`, which build their own argv.

## Validation

```
cargo build --workspace                                  PASS
cargo test --workspace --all-targets --no-fail-fast       PASS (98 test binaries)
cargo test -p rigorix-engine --lib                        PASS (2350)
cargo clippy --workspace --all-targets -- -D warnings     PASS
cargo fmt --check                                         PASS
cargo audit                                               no advisories against the lockfile
.pi/scripts/validate-guards.sh (+ --self-test)             PASS
engine/.pi/scripts/validate-sink-register.sh (+ self-test)  PASS (46 rows, 87 hits, 11 seeds)
.pi/scripts/validate-unsafe-sites.sh / -panic-sites.sh     PASS (18 / 37, all baselined)
.pi/scripts/validate-script-drift.sh                       PASS (0 unrecorded)
.pi/scripts/validate-docs.sh                               PASS
engine validators (ci, tests, security, operations, integration, canonical, architecture)  PASS
.github/scripts/check_affected_map.sh                      PASS
.github/scripts/validate-affected-graph.sh                 PASS
```

Closes #1500, #1501, #1502.