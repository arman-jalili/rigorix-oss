#!/usr/bin/env bash
# check_test_census.sh — the suite is partitioned, and the partition is total (#1440).
#
# `test` and `integration` used to run the same tests. The matrix ran
# `cargo nextest run -p <crate>` six times (30.2 job-minutes) and `integration` then ran
# `cargo nextest run --workspace` (14.9) — the union of exactly what the matrix had just run,
# under a different label. Two executions of one verdict.
#
# The split is by subject: a binary that stays inside one crate is that crate's, and a binary
# that spawns a process, drives a transport, or reads another crate's catalog is the boundary's.
# The partition is declared once, in `.config/nextest.toml`, as `[profile.unit]` and
# `[profile.cross-crate]` filters.
#
# This census is what keeps it total. Four rules, each with a seed:
#
#   1. the two filters describe the **same** partition — `unit` is the exact complement of
#      `cross-crate`. A binary missing from the complement runs in both jobs (doubled); one
#      listed in neither would run in no job at all (orphaned).
#   2. every binary the cross list names **exists** in `cargo metadata`. A stale entry makes a
#      filter that silently matches less than it says.
#   3. neither side is empty. A filter that matches nothing passes for the wrong reason.
#   4. the workflow uses both profiles, and `integration` does not run the workspace without
#      one — the defect itself, checked rather than remembered.
#
#   bash .github/scripts/check_test_census.sh
#   bash .github/scripts/check_test_census.sh --self-test
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CONFIG="$REPO_ROOT/.config/nextest.toml"
WORKFLOW="$REPO_ROOT/.github/workflows/ci.yml"
SELF_TEST=0

while [ $# -gt 0 ]; do
  case "$1" in
    --self-test) SELF_TEST=1; shift ;;
    --config)
      [ $# -ge 2 ] || { echo "check_test_census: --config requires a file" >&2; exit 2; }
      CONFIG="$2"; shift 2 ;;
    --workflow)
      [ $# -ge 2 ] || { echo "check_test_census: --workflow requires a file" >&2; exit 2; }
      WORKFLOW="$2"; shift 2 ;;
    -h|--help)
      sed -n '2,32p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *) echo "check_test_census: unknown argument '$1'" >&2; exit 2 ;;
  esac
done

CORE=$(cat <<'PY'
import json
import re
import subprocess
import sys

config_path, workflow_path, root = sys.argv[1:4]
findings = []


def profile_filter(name):
    """The `default-filter` of `[profile.<name>]`, or None when the profile has none."""
    text = open(config_path, encoding="utf-8").read()
    m = re.search(rf"^\[profile\.{re.escape(name)}\]\s*$(.*?)(?=^\[|\Z)", text, re.M | re.S)
    if not m:
        return None
    f = re.search(r"^default-filter\s*=\s*\"(.*)\"\s*$", m.group(1), re.M)
    return f.group(1) if f else None


def binary_ids(expr):
    return re.findall(r"binary_id\(\s*([A-Za-z0-9_.-]+::[A-Za-z0-9_.-]+)\s*\)", expr)


cross_expr = profile_filter("cross-crate")
unit_expr = profile_filter("unit")

if cross_expr is None:
    findings.append("`.config/nextest.toml` has no `[profile.cross-crate]` filter — the boundary "
                    "is not declared, so `integration` runs whatever it is told to")
if unit_expr is None:
    findings.append("`.config/nextest.toml` has no `[profile.unit]` filter — `test` would run "
                    "the boundary binaries a second time")

cross = binary_ids(cross_expr or "")
unit = binary_ids(unit_expr or "")

# 1. the same partition, in both directions
if cross or unit:
    if set(cross) != set(unit):
        for name in sorted(set(cross) - set(unit)):
            findings.append(f"`{name}` is on the boundary but is not excluded from `unit` — it "
                            f"runs in both jobs (doubled), and the second run decides nothing")
        for name in sorted(set(unit) - set(cross)):
            findings.append(f"`{name}` is excluded from `unit` but is not on the boundary — no "
                            f"job runs it (orphaned)")
    if len(cross) != len(set(cross)):
        findings.append("the boundary lists a binary twice — a filter that names it twice cannot "
                        "say which job owns it")

# 2. every named binary exists
proc = subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1",
                       "--manifest-path", f"{root}/Cargo.toml"],
                      capture_output=True, text=True)
if proc.returncode != 0:
    print("❌ cargo metadata failed — the binary list cannot be read", file=sys.stderr)
    sys.exit(2)
meta = json.loads(proc.stdout)
known = set()
for package in meta["packages"]:
    for target in package["targets"]:
        if "test" in target["kind"]:
            known.add(f"{package['name']}::{target['name']}")

for name in sorted(set(cross) - known):
    findings.append(f"`{name}` is on the boundary and no test binary has that id — a filter "
                    f"that matches nothing runs green having checked nothing")

# 3. neither side empty
if not cross:
    findings.append("the boundary filter names no binary — `integration` would run 0 tests and "
                    "pass")
if not known:
    findings.append("`cargo metadata` reports no test binary at all — 0 binaries is a broken "
                    "census, not a clean tree")

# 4. the workflow uses both profiles
workflow = open(workflow_path, encoding="utf-8").read()
if "--profile unit" not in workflow:
    findings.append("ci.yml never runs `--profile unit` — the matrix would run the boundary "
                    "binaries as well")
if "--profile cross-crate" not in workflow:
    findings.append("ci.yml never runs `--profile cross-crate` — the boundary is declared and "
                    "nothing runs it")
if re.search(r"cargo nextest run --locked --workspace(?!\s+--profile)", workflow):
    findings.append("ci.yml runs `cargo nextest run --workspace` with no profile — that is the "
                    "whole suite a second time, the defect #1440 exists for")

for item in findings:
    print("  ✗ " + item)
if findings:
    print(f"❌ test census: {len(findings)} finding(s)")
    sys.exit(1)
print(f"  {len(known)} test binary/binaries: {len(cross)} on the boundary, "
      f"{len(known) - len(cross)} inside their crate")
print("✅ test census: every binary claimed once, both sides non-empty, both profiles in use")
PY
)

run_check() { # run_check <config> <workflow> [root]
  python3 -c "$CORE" "$1" "$2" "${3:-$REPO_ROOT}"
}

if [ "$SELF_TEST" -eq 1 ]; then
  echo "── check_test_census --self-test ──"
  TMP="$(mktemp -d)"
  trap 'rm -rf "$TMP"' EXIT
  passes=0
  fails=0

  cat > "$TMP/workflow.yml" <<'YAML'
jobs:
  test:
    steps:
      - run: cargo nextest run --locked -p x --profile unit
  integration:
    steps:
      - run: cargo nextest run --locked --workspace --profile cross-crate
YAML

  cat > "$TMP/nextest.toml" <<'YAML'
[profile.default]
slow-timeout = { period = "5s", terminate-after = 60 }

[profile.unit]
default-filter = "not (binary_id(rigorix-cli::gate_envelope) | binary_id(rigorix-server::catalog_parity))"

[profile.cross-crate]
default-filter = "binary_id(rigorix-cli::gate_envelope) | binary_id(rigorix-server::catalog_parity)"
YAML

  expect() { # expect <0|1> <label> <needle> <config>
    local want="$1" label="$2" needle="$3" cfg="$4" got out
    set +e
    out="$(run_check "$cfg" "$TMP/workflow.yml")"
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

  expect 0 "a total partition passes" "" "$TMP/nextest.toml"

  # S1: a boundary entry dropped from `unit`'s complement — that binary now runs in both jobs.
  sed 's/ | binary_id(rigorix-server::catalog_parity))"$/)"  # dropped/' "$TMP/nextest.toml" \
    > "$TMP/doubled.toml"
  if ! cmp -s "$TMP/nextest.toml" "$TMP/doubled.toml"; then
    expect 1 "a binary missing from the complement is doubled" "runs in both jobs" "$TMP/doubled.toml"
  else
    echo "  ❌ seed — doubled: the mutation matched nothing"
    fails=$((fails + 1))
  fi

  # S2: a boundary entry that names no binary — a filter that matches less than it says.
  sed 's/binary_id(rigorix-server::catalog_parity)/binary_id(rigorix-server::catalog_parity_gone)/g' \
    "$TMP/nextest.toml" > "$TMP/stale.toml"
  expect 1 "a boundary entry with no binary fails" "no test binary has that id" "$TMP/stale.toml"

  # S3: an empty boundary — a filter that matches nothing passes for the wrong reason.
  python3 - "$TMP/nextest.toml" "$TMP/empty.toml" <<'S3'
import sys
src, dst = sys.argv[1], sys.argv[2]
text = open(src).read()
text = text.replace(
    'default-filter = "not (binary_id(rigorix-cli::gate_envelope) | binary_id(rigorix-server::catalog_parity))"',
    'default-filter = "all()"')
text = text.replace(
    'default-filter = "binary_id(rigorix-cli::gate_envelope) | binary_id(rigorix-server::catalog_parity)"',
    'default-filter = "none()"')
open(dst, "w").write(text)
S3
  expect 1 "an empty boundary fails" "names no binary" "$TMP/empty.toml"

  # S4: `integration` back to the whole workspace with no profile.
  sed 's/ --profile cross-crate$//' "$TMP/workflow.yml" > "$TMP/noprofile.yml"
  set +e
  out="$(run_check "$TMP/nextest.toml" "$TMP/noprofile.yml" 2>&1)"; got=$?
  set -e
  if [ "$got" -eq 1 ] && printf '%s' "$out" | grep -q "whole suite a second time"; then
    echo "  ✅ workspace without a profile fails"
    passes=$((passes + 1))
  else
    echo "  ❌ workspace without a profile fails (exit $got)"
    printf '%s\n' "$out" | sed 's/^/     /'
    fails=$((fails + 1))
  fi

  echo "  self-test: $passes passed, $fails failed"
  [ "$fails" -eq 0 ] || exit 1
  echo "✅ check_test_census self-test"
  exit 0
fi

run_check "$CONFIG" "$WORKFLOW"
