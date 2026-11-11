# Issue #1291 — 167 arithmetic counters end their scripts silently on bash 4.4+

**Branch:** `issue/1291` · **Epic:** ci-integrity · **Component:** Validation · **Module:** tooling
**Priority:** high · **Depends on:** the enterprise repository #492 (the same fix + reference guard, merged there)

## Defect

`(( X++ ))` returns status 1 when the result is 0, so the **first** increment of a counter that
starts at 0 fails. Whether that ends the script depends on the shell:

```
set -e; x=0; ((x++)); echo survived
  bash 3.2.57 (macOS, this machine)  → exit 0, "survived"
  bash 4.4.23 / 5.3.12               → exit 1, no output
```

Measured on this tree, same input, both shells:

```
mcp/.pi/scripts/ci/validate_agent_output.sh --input /tmp/agent-out.md
  bash 3.2 → exit 1, 21 lines, "❌ Agent output validation failed (3 check(s))"
  bash 5.3 → exit 1,  0 lines, no output at all
```

## What the scan found — 177, not 167

The issue counts 167 standalone statements. Two shapes make a counter fail, and the second one
fails on **every** shell:

| shape | count | why it fails |
|---|---|---|
| standalone `(( X++ ))` on its own line | 167 | bash 4.4+ propagates the failed command through `errexit` |
| the same command as the tail of a one-line function: `log_pass() { echo …; ((PASS++)); }` | **10** | the *function* returns 1, and calling a function that fails is a failed command under `errexit` — bash 3.2 included |

Measured on the second shape, both shells, before the fix:

```
bash .pi/scripts/ci/stage_docs_policy.sh            → exit 1, 2 lines (dies after the first PASS)
bash5 .pi/scripts/ci/stage_docs_policy.sh           → exit 1, 2 lines (same)
```

So ten of these have been truncating stage scripts on the development machine too, where nothing
was blamed on the shell. The guard catches both shapes.

## What changes

| # | Change |
|---|---|
| 1 | Every site becomes the assignment form `X=$((X + 1))` / `X=$((X - 1))` — 177 sites in 41 file instances (22 distinct contents; five copies of the shared scripts) |
| 2 | New guard `.pi/scripts/validate-shell-exit-status.sh` in all five copies (byte-identical): a **6-case self-test runs first** (standalone, one-liner tail, last-line; clean on `&&`, `\|\|`, `if`, `while`, assignment form and `set +e` files) plus an assertion that the report carries `path:line`; then the tree scan |
| 3 | New CI stage `🐚 Shell exit status` (`🔐 errexit arithmetic …`), mandatory in the Final Gate and listed in the stage report like every other stage |
| 4 | `(( X++ )) \|\| true` is *not* used anywhere: it suppresses every failure, not just this one |

Every counter in scope is initialised before use (checked: 0 of 177 sites read an unset variable),
so the `$((X + 1))`-on-unset difference the issue puts out of scope cannot bite here.

## Acceptance criteria

| # | Criterion | Status |
|---|---|---|
| 1 | every standalone `(( X++ ))` in an errexit script becomes the assignment form | ✅ 167 + 10 one-liner tails = 177 |
| 2 | a guard with a self-test first (positive, negative, last-line), failing on `path:line` | ✅ `.pi/scripts/validate-shell-exit-status.sh` |
| 3 | the guard runs in CI and against every copy | ✅ `🐚 Shell exit status` stage; it scans from its own root, so the root copy covers all five, and each crate copy covers its own tree |
| 4 | seeded: restoring `((PASSED++))` in `run_preflight.sh` fails the guard naming file and line | ✅ `✗ .pi/scripts/ci/run_preflight.sh:68` → exit 1; restored → exit 0 |
| 5 | `validate_agent_output.sh` prints the same 21-line verdict on bash 3.2 and bash 5 | ✅ 21 lines on both, identical verdict; the only diff is bash's own wording of its own arithmetic error (`syntax error` vs `arithmetic syntax error`) |

## Seeds

```
guard, clean tree                          → exit 0, self-test green
seed: ((PASSED++)) back in run_preflight   → ✗ .pi/scripts/ci/run_preflight.sh:68 … exit 1
restore                                    → exit 0

stage_docs_policy.sh   before: 2 lines, exit 1 on bash 3.2 AND bash 5
                       after : 5 lines, exit 0, byte-identical on both
validate_agent_output  before: 21 lines (bash 3.2) / 0 lines (bash 5)
                       after : 21 lines on both
check_architecture_conformance seeded with a domain→infrastructure import → still FAIL
                       (Fail: 1, names module and file) — the counters still count
bash -n over all 41 edited scripts → 42/42 parse
```
