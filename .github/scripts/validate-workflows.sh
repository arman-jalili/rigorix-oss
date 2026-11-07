#!/usr/bin/env bash
# validate-workflows.sh — every cargo job restores the workspace cache, once, and nothing that
# resolves the lockfile is allowed to resolve it differently (#1439).
#
# The defect this exists for: `.github/actions/setup` cached `target/` with
# `actions/cache@v4` under `${{ inputs.cache-key }}-<lockfile>`, and the key is **per job**
# (`lint-<crate>`, `test-<crate>`, …). Fourteen jobs saved fourteen copies of the same compiled
# workspace and not one of them could restore another's, so every cargo job compiled five crates
# from cold, every push. `Swatinem/rust-cache@v2` with a constant `shared-key` is one cache that
# all of them share; `save-if: main` is what stops a PR from writing fourteen copies of a tree
# it did not finish building.
#
# Four rules, each with a seed:
#
#   1. a job that runs cargo restores the cache — through the shared action, or directly;
#   2. a cache step declares `shared-key: workspace-v1`. `restore-keys` alone is a cache that
#      helps one job and calls itself shared;
#   3. no `actions/cache` step caches `target/` — that is the per-job copy this issue removed;
#   4. every cargo invocation carries `--locked`, because a cache that hides a lockfile drift is
#      worse than a cold build. `cargo fmt` and `cargo audit` do not accept the flag; they are
#      the only exceptions, and they are written here so a new command cannot inherit one.
#
#   bash .github/scripts/validate-workflows.sh
#   bash .github/scripts/validate-workflows.sh --self-test
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORKFLOW="$REPO_ROOT/.github/workflows/ci.yml"
SETUP="$REPO_ROOT/.github/actions/setup/action.yml"
SELF_TEST=0

while [ $# -gt 0 ]; do
  case "$1" in
    --self-test) SELF_TEST=1; shift ;;
    --workflow)
      [ $# -ge 2 ] || { echo "validate-workflows: --workflow requires a file" >&2; exit 2; }
      WORKFLOW="$2"; shift 2 ;;
    --setup)
      [ $# -ge 2 ] || { echo "validate-workflows: --setup requires a file" >&2; exit 2; }
      SETUP="$2"; shift 2 ;;
    -h|--help)
      sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *) echo "validate-workflows: unknown argument '$1'" >&2; exit 2 ;;
  esac
done

CORE=$(cat <<'PY'
import re
import sys

workflow, setup = sys.argv[1:3]
findings = []
state = {"jobs": 0, "cargo_jobs": 0, "cached_jobs": 0, "cargo_calls": 0}

text = open(workflow, encoding="utf-8").read()
setup_text = open(setup, encoding="utf-8").read()

CARGO = re.compile(r"\bcargo\s+(?!fmt\b|audit\b)([a-z-]+)")
EXEMPT = {"fmt", "audit"}          # neither accepts --locked; written here, not inherited


def job_blocks(text):
    """`{name: [lines]}` for every job under `jobs:`, comments included (they are stripped later)."""
    blocks, current, injobs = {}, None, False
    for line in text.split("\n"):
        if line.startswith("jobs:"):
            injobs = True
            continue
        if injobs and line and not line.startswith(" "):
            injobs = False
        if not injobs:
            continue
        m = re.match(r"^  ([A-Za-z0-9_-]+):\s*$", line)
        if m:
            current = m.group(1)
            blocks[current] = []
            continue
        if current is not None:
            blocks[current].append(line)
    return blocks


def code_lines(lines):
    return [l for l in lines if not l.lstrip().startswith("#")]


def shell_of(lines):
    """The shell text of a job: the bodies of its `run:` keys, and nothing else.

    A first version scanned the whole block, so a *step name* ("Every cargo job restores one
    cache") read as a `cargo job` invocation and the `workspace` job was accused of compiling
    nothing. Prose and commands are different subjects; only the commands are checked here.
    """
    out, in_run, indent = [], False, 0
    for line in lines:
        stripped = line.lstrip()
        if stripped.startswith("#"):
            continue
        # `- run: <cmd>` and `run: |` are both step keys; the list dash is part of the step.
        m = re.match(r"^(\s*)(?:-\s*)?run:\s*(.*)$", line)
        if m:
            rest = m.group(2).strip()
            in_run = rest in ("", "|", ">", "|-", ">-")
            if rest and rest not in ("|", ">", "|-", ">-"):
                out.append(rest)
            indent = len(m.group(1))
            continue
        if in_run:
            if line.strip() and len(line) - len(stripped) <= indent:
                in_run = False
                continue
            out.append(line)
    return "\n".join(out)


blocks = job_blocks(text)
state["jobs"] = len(blocks)

if not blocks:
    findings.append("`jobs:` has no jobs — a validator that scans nothing passes everything")

# The shared action must itself carry the shared cache, or rule 1 is satisfied by a lie.
setup_code = "\n".join(code_lines(setup_text.split("\n")))
if "Swatinem/rust-cache@" not in setup_code:
    findings.append("`.github/actions/setup` does not use `Swatinem/rust-cache` — the jobs that "
                    "restore through it restore nothing")
elif "shared-key: workspace-v1" not in setup_code:
    findings.append("`.github/actions/setup` has no `shared-key: workspace-v1` — a cache keyed "
                    "per job is fourteen copies of one workspace, not a shared cache")
elif "save-if:" not in setup_code:
    findings.append("`.github/actions/setup` has no `save-if:` — every PR would write a cache "
                    "it did not finish building")
if re.search(r"actions/cache@", setup_code) and re.search(r"^\s+target\s*$", setup_code, re.M):
    findings.append("`.github/actions/setup` caches `target/` with `actions/cache` — that key is "
                    "per job, which is the fourteen-copies defect (#1439)")

for name, raw in blocks.items():
    block_text = "\n".join(code_lines(raw))
    body = shell_of(code_lines(raw))
    # A job compiles the workspace if it calls cargo directly *or* runs a script that does:
    # `coverage.sh` (llvm-cov over the workspace) and `run-module-smoke.sh` (the wasm build)
    # never write the word `cargo` in the workflow, and a detector that misses them reports a
    # smaller census than the one that matters.
    runs_cargo = bool(CARGO.search(body) or "nextest" in body or "llvm-cov" in body
                      or "coverage.sh" in body or "run-module-smoke.sh" in body)
    if not runs_cargo:
        continue
    state["cargo_jobs"] += 1
    restores = "./.github/actions/setup" in block_text or "Swatinem/rust-cache@" in block_text
    if not restores:
        findings.append(f"`{name}` runs cargo and restores no cache — it compiles the workspace "
                        f"from cold, every push")
        continue
    state["cached_jobs"] += 1
    if "Swatinem/rust-cache@" in block_text:
        # Rule 2: the step must declare the shared key, not lean on `restore-keys`.
        window = block_text[block_text.index("Swatinem/rust-cache@"):]
        head = "\n".join(window.split("\n")[:8])
        if "shared-key: workspace-v1" not in head:
            findings.append(f"`{name}` has a cache step with no `shared-key: workspace-v1` — "
                            f"`restore-keys` alone helps one job and is not a shared cache")
    if re.search(r"actions/cache@", block_text) and re.search(r"^\s+target\s*$", block_text, re.M):
        findings.append(f"`{name}` caches `target/` with `actions/cache` — a per-job copy of the "
                        f"workspace")

for number, line in enumerate(text.split("\n"), start=1):
    stripped = line.lstrip()
    if stripped.startswith("#") or re.match(r"-?\s*name:", stripped):
        continue
    for match in CARGO.finditer(line):
        state["cargo_calls"] += 1
        sub = match.group(1)
        if sub in EXEMPT:
            continue
        if "--locked" not in line:
            findings.append(f"ci.yml:{number}: `cargo {sub}` without `--locked` — a cached build "
                            f"must not resolve differently from the lockfile")

if not re.search(r"^\s+CARGO_INCREMENTAL:\s*\"?0\"?\s*$", text, re.M):
    findings.append("ci.yml has no `CARGO_INCREMENTAL: \"0\"` — an incremental `target/` cannot "
                    "be shared between jobs, and restoring it produces rebuilds that look like hits")

for item in findings:
    print("  ✗ " + item)
if findings:
    print(f"❌ workflows: {len(findings)} finding(s)")
    sys.exit(1)
print(f"  scanned {state['jobs']} job(s): {state['cargo_jobs']} run cargo, "
      f"{state['cached_jobs']} restore the shared cache, {state['cargo_calls']} cargo call(s) checked")
print("✅ workflows: one workspace cache, restored by every cargo job, and `--locked` everywhere")
PY
)

run_check() { # run_check <workflow> <setup>
  python3 -c "$CORE" "$1" "$2"
}

if [ "$SELF_TEST" -eq 1 ]; then
  echo "── validate-workflows --self-test ──"
  TMP="$(mktemp -d)"
  trap 'rm -rf "$TMP"' EXIT
  passes=0
  fails=0

  setup_ok() {
    cat > "$1" <<'YAML'
name: Setup Rust
inputs:
  cache-key:
    required: false
runs:
  using: composite
  steps:
    - name: Restore the workspace cache
      uses: Swatinem/rust-cache@v2
      with:
        shared-key: workspace-v1
        save-if: ${{ github.ref == 'refs/heads/main' }}
YAML
  }

  wf_ok() {
    cat > "$1" <<'YAML'
name: CI
on: [push]
env:
  CARGO_TERM_COLOR: always
  CARGO_INCREMENTAL: "0"
jobs:
  lint:
    name: Lint
    steps:
      - uses: ./.github/actions/setup
      - run: cargo clippy --locked -p rigorix-engine -- -D warnings
  wasm-gates:
    name: wasm gates
    steps:
      - uses: Swatinem/rust-cache@v2
        with:
          shared-key: workspace-v1
          save-if: ${{ github.ref == 'refs/heads/main' }}
      - run: cargo nextest run --locked -p rigorix-gate-wasm
YAML
  }

  expect() { # expect <0|1> <label> <needle> <workflow> <setup>
    local want="$1" label="$2" needle="$3" wf="$4" st="$5" got out
    set +e
    out="$(run_check "$wf" "$st" 2>&1)"
    got=$?
    set -e
    if [ "$got" -ne "$want" ]; then
      echo "  ❌ $label (exit $got, wanted $want)"
      printf '%s\n' "$out" | sed 's/^/     /'
      fails=$((fails + 1))
      return
    fi
    if [ -n "$needle" ] && ! printf '%s' "$out" | grep -qF -- "$needle"; then
      echo "  ❌ $label (exit $got, but nothing named '$needle')"
      printf '%s\n' "$out" | sed 's/^/     /'
      fails=$((fails + 1))
      return
    fi
    echo "  ✅ $label"
    passes=$((passes + 1))
  }

  setup_ok "$TMP/setup.yml"
  wf_ok "$TMP/good.yml"
  expect 0 "a shared cache with --locked passes" "" "$TMP/good.yml" "$TMP/setup.yml"

  # S1: `restore-keys` and no `shared-key` — a cache that helps one job (#1439's own seed).
  python3 - "$TMP/good.yml" "$TMP/noshared.yml" <<'S1'
import sys
src, dst = sys.argv[1], sys.argv[2]
text = open(src).read().replace("          shared-key: workspace-v1\n", "", 1)
open(dst, "w").write(text)
S1
  expect 1 "a cache step without shared-key fails" "no \`shared-key: workspace-v1\`" \
    "$TMP/noshared.yml" "$TMP/setup.yml"

  # S2: a cargo job that restores nothing.
  python3 - "$TMP/good.yml" "$TMP/nocache.yml" <<'S2'
import sys
src, dst = sys.argv[1], sys.argv[2]
text = open(src).read().replace("      - uses: ./.github/actions/setup\n", "", 1)
open(dst, "w").write(text)
S2
  expect 1 "a cargo job with no cache fails" "runs cargo and restores no cache" \
    "$TMP/nocache.yml" "$TMP/setup.yml"

  # S3: `--locked` dropped from one invocation.
  python3 - "$TMP/good.yml" "$TMP/nolocked.yml" <<'S3'
import sys
src, dst = sys.argv[1], sys.argv[2]
text = open(src).read().replace("cargo clippy --locked -p", "cargo clippy -p", 1)
open(dst, "w").write(text)
S3
  expect 1 "a cargo call without --locked fails" "without \`--locked\`" \
    "$TMP/nolocked.yml" "$TMP/setup.yml"

  # S4: the per-job `actions/cache` on `target/` growing back in the shared action.
  python3 - "$TMP/setup.yml" "$TMP/percache.yml" <<'S4'
import sys
src, dst = sys.argv[1], sys.argv[2]
text = open(src).read().replace(
    "    - name: Restore the workspace cache\n      uses: Swatinem/rust-cache@v2\n      with:\n        shared-key: workspace-v1\n",
    "    - name: Cache cargo\n      uses: actions/cache@v4\n      with:\n        path: |\n          ~/.cargo/registry\n          target\n        key: ${{ inputs.cache-key }}-${{ hashFiles('Cargo.lock') }}\n        restore-keys: |\n          ${{ inputs.cache-key }}-\n")
open(dst, "w").write(text)
S4
  expect 1 "a per-job actions/cache on target/ fails" "caches \`target/\`" \
    "$TMP/good.yml" "$TMP/percache.yml"

  echo "  self-test: $passes passed, $fails failed"
  [ "$fails" -eq 0 ] || exit 1
  echo "✅ validate-workflows self-test"
  exit 0
fi

run_check "$WORKFLOW" "$SETUP"
