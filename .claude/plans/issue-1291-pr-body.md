Implements **#1291** (`ISSUE-ARITHMETIC-COUNTERS-END-SCRIPTS-SILENTLY-ON-LINUX`).

## Measured

```
set -e; x=0; ((x++)); echo survived
  bash 3.2.57 (macOS)      → exit 0, "survived"
  bash 5.3.12 (this run)   → exit 1, no output

mcp/.pi/scripts/ci/validate_agent_output.sh --input /tmp/agent-out.md
  bash 3.2 → 21 lines, "❌ Agent output validation failed (3 check(s))", exit 1
  bash 5.3 →  0 lines, exit 1
```

## 177 sites, not 167 — the second shape fails on every shell

| shape | count | why |
|---|---|---|
| standalone `(( X++ ))` | 167 | bash 4.4+ propagates the failed command through `errexit` |
| tail of a one-line function: `log_pass() { echo …; ((PASS++)); }` | **10** | the *function* returns 1; calling a failing function is a failed command under `errexit` — bash 3.2 included |

```
stage_docs_policy.sh, before:   bash 3.2 → 2 lines, exit 1     bash 5 → 2 lines, exit 1
                                (dies after the first PASS — on both shells)
stage_docs_policy.sh, after:    bash 3.2 → 5 lines, exit 0     bash 5 → 5 lines, exit 0, identical
```

So ten of these had been truncating stage output on the development machine too, where nothing
was blamed on the shell.

## Changed

- **177 sites → assignment form** (`X=$((X + 1))`, `X=$((X - 1))`) across 41 file instances
  (22 distinct contents — the five-copy layout multiplies every file). `(( X++ )) || true` is used
  nowhere: it suppresses every failure, not just this one. Every counter in scope is initialised
  before use (checked: 0 of 177 read an unset var), so the `$((…))`-on-unset difference the issue
  puts out of scope cannot bite.
- **Guard** `.pi/scripts/validate-shell-exit-status.sh`, in all five copies byte-identical:
  a **6-case self-test runs first** (standalone, one-liner tail, last line; clean on `&&`, `\|\|`,
  `if`, `while`, assignment form and `set +e` files) plus an assertion that the report carries
  `path:line`; then the tree scan. It scans from its own root, so the root copy covers all five
  copies and each crate copy covers its own tree.
- **CI**: new `🐚 Shell exit status` stage (`🔐 errexit arithmetic (no script may end on `(( x++ ))`)`),
  mandatory in the Final Gate and listed in the `📋 Stage report` table like every other stage.

## Seeds

```
clean tree                                      → guard exit 0, self-test green
restore ((PASSED++)) in run_preflight.sh        → ✗ .pi/scripts/ci/run_preflight.sh:68 … exit 1
restore the file                                → guard exit 0

bash -n over all 41 edited scripts              → 42/42 parse
check_architecture_conformance, seeded with a domain→infrastructure import
                                                → still Fail: 1, naming module and file (the counters still count)
validate_agent_output.sh                        → 21 identical lines on bash 3.2 and bash 5
                                                  (the only diff is bash's own wording of bash's own
                                                  arithmetic error: "syntax error" vs "arithmetic syntax error")
```

## Note on "the five copies stay byte-identical"

Four of the eight shared scripts in scope were already byte-identical, and remain so. The other
five had drifted in ways unrelated to arithmetic — `run_hardening_stages.sh` has **five distinct
copies**, differing in which proofing stages they run (`capacity-ledger`/`effect-commitment` in the
root vs `execution-tools`/`template-tools` in the crate). Unifying those is a behaviour change of
its own, so this PR does not: it makes the *arithmetic* identical everywhere and makes the guard
scan every copy, so a fix cannot land in one and miss four.
