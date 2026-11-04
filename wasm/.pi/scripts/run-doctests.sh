#!/usr/bin/env bash
# ============================================================================
# run-doctests.sh — the doctest channel `cargo nextest` does not run (#1373)
# ============================================================================
# WHY THIS EXISTS
#
# `cargo nextest` runs no doc tests. Adopting it as the test runner therefore
# removes a coverage channel, and removing a channel while the job stays green is
# the defect class this repository keeps finding: a gate that passes because it
# lost its subject. So the doctests get their own **required** step, in the same
# CI job as nextest, and this script is what makes it required rather than
# decorative.
#
# WHAT IS ASSERTED
#
#   1. Any doctest failure fails the step (the runner's exit status is not enough
#      on its own — see 3).
#   2. The number of doctests is PRINTED, per crate, and the total is printed.
#   3. `executed = 0` fails. Not `discovered = 0`: measured 2026-11-04, engine has
#      16 discovered doctests of which 10 are ```ignore — it would print
#      "running 16 tests" and execute six. A count of discovered tests is not a
#      count of executed tests, and only the second one is a test run.
#   4. A crate that executes fewer than its floor in .pi/doctest-floors.tsv fails.
#      The floor is a RATCHET over `executed`, so a crate cannot lose its doctests
#      and stay green; raising it is a deliberate edit with a measurement beside it.
#
# USAGE
#   .pi/scripts/run-doctests.sh                 # whole workspace
#   .pi/scripts/run-doctests.sh -p rigorix-engine   # one crate (CI test-stage matrix)
#   .pi/scripts/run-doctests.sh --self-test     # prove each verdict fires
# ============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FLOORS="$ROOT/.pi/doctest-floors.tsv"

usage() {
    sed -n '3,32p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

# Folds a `cargo test --doc` log and prints the verdict.
# Exit 0 = channel intact, 1 = a named failure.
#
# The log is passed by PATH, not on stdin. It was stdin once, and the heredoc that
# carries this program IS stdin — so `sys.stdin.read()` returned the program's own
# text, every run reported "no Doc-tests section found", and the only reason that
# did not ship is that the self-test's *passing* cases could not pass.
verdict() { # $1 floors file (may be absent), $2 label, $3 log path, $4 strict (1|0)
    python3 - "$1" "$2" "$3" "${4:-1}" <<'PY'
import re, sys

floors_path, label, strict = sys.argv[1], sys.argv[2], sys.argv[4] == '1' 
with open(sys.argv[3], encoding='utf-8', errors='replace') as fh:
    text = fh.read()

crate, rows = None, []
for line in text.splitlines():
    m = re.match(r'\s*Doc-tests\s+(\S+)', line)
    if m:
        crate = m.group(1)
        continue
    m = re.match(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored', line)
    if m and crate:
        status, passed, failed, ignored = m.group(1), int(m.group(2)), int(m.group(3)), int(m.group(4))
        rows.append({
            'crate': crate, 'status': status, 'passed': passed, 'failed': failed,
            'ignored': ignored, 'discovered': passed + failed + ignored,
            'executed': passed + failed,
        })
        crate = None

if not rows:
    print("❌ no Doc-tests section found in the output — the command did not run doc "
          "tests at all, so a green exit status would mean nothing")
    sys.exit(1)

width = max(len(r['crate']) for r in rows)
print(f"{'crate':{width}s}  {'discovered':>10s} {'executed':>8s} {'ignored':>7s}  status")
for r in rows:
    print(f"{r['crate']:{width}s}  {r['discovered']:10d} {r['executed']:8d} "
          f"{r['ignored']:7d}  {r['status']}")

total_d = sum(r['discovered'] for r in rows)
total_e = sum(r['executed'] for r in rows)
total_i = sum(r['ignored'] for r in rows)
print(f"{'TOTAL':{width}s}  {total_d:10d} {total_e:8d} {total_i:7d}")
print(f"doctests ({label}): {total_d} discovered, {total_e} executed, {total_i} ignored "
      f"across {len(rows)} crates")

problems = []
for r in rows:
    if r['failed'] or r['status'] != 'ok':
        problems.append(f"{r['crate']}: {r['failed']} doctest(s) failed")
# Strict (workspace) runs must execute something. A per-crate run is judged by that
# crate's floor instead: five of the six workspace crates have never had a doctest, and
# a matrix job must not invent a requirement the crate never met.
if total_e == 0 and strict:
    problems.append("zero doctests EXECUTED — a channel that runs nothing is not a passing "
                    "channel, whatever the discovered count says")

try:
    with open(floors_path, encoding='utf-8') as fh:
        floors = [ln.rstrip('\n').split('\t') for ln in fh
                  if ln.strip() and not ln.lstrip().startswith('#')]
except FileNotFoundError:
    floors = []

by_crate = {r['crate']: r for r in rows}
# A crate with no floor is reported but not judged: five of six workspace crates have
# never had a doctest, and failing them for that would be inventing a requirement. The
# crate that carries the channel (engine) has a floor, so its zero is caught below.
for parts in floors:
    if len(parts) < 2:
        problems.append(f"{floors_path}: malformed row {parts!r}")
        continue
    name, floor = parts[0], int(parts[1])
    seen = by_crate.get(name)
    if seen is None and not strict:
        # Per-crate run: this crate simply is not the one this floor describes.
        # (In a workspace run an absent crate IS a regression, and falls through.)
        continue
    executed = seen['executed'] if seen else 0
    if executed < floor:
        problems.append(f"{name}: {executed} doctest(s) executed, floor is {floor} "
                        f"(.pi/doctest-floors.tsv) — the channel regressed")

if problems:
    for p in problems:
        print(f"❌ {p}")
    sys.exit(1)
print(f"✅ doctest channel intact ({label})")
PY
}

run_workspace() {
    local log
    log="$(mktemp -t doctests)"
    local rc=0
    cargo test --doc --workspace --no-fail-fast >"$log" 2>&1 || rc=$?
    # The log is always printed: a failing run is the one whose numbers are needed.
    cat "$log"
    local verdict_rc=0
    verdict "$FLOORS" "workspace" "$log" 1 || verdict_rc=$?
    rm -f "$log"
    if [ "$rc" -ne 0 ] || [ "$verdict_rc" -ne 0 ]; then
        return 1
    fi
}

run_package() { # $1 crate name
    local pkg="$1" log
    log="$(mktemp -t doctests)"
    local rc=0
    cargo test --doc -p "$pkg" --no-fail-fast >"$log" 2>&1 || rc=$?
    cat "$log"
    local verdict_rc=0
    verdict "$FLOORS" "$pkg" "$log" 0 || verdict_rc=$?
    rm -f "$log"
    if [ "$rc" -ne 0 ] || [ "$verdict_rc" -ne 0 ]; then
        return 1
    fi
}

# ---------------------------------------------------------------------------
# --self-test: each verdict is proven by making it fire.
#
# Three throwaway crates, one per outcome, run through the SAME `verdict` used in
# CI. A guard whose failure paths are unexercised is a guard that has never been
# observed to fail.
# ---------------------------------------------------------------------------
self_test() {
    local tmp pass=0 fail=0
    tmp="$(mktemp -d -t doctest-selftest)"
    # NOTE: no `trap … RETURN` here. bash fires a RETURN trap after EVERY function
    # return, which deleted the throwaway crates right after `mk` created them and
    # left every case with an empty log — the self-test "passed" four assertions by
    # reading nothing. Cleanup is explicit at the end instead.
    export CARGO_TARGET_DIR="$tmp/target"

    mk() { # $1 name, $2 doc body
        mkdir -p "$tmp/$1/src"
        printf '[package]\nname = "%s"\nversion = "0.0.0"\nedition = "2021"\n' "$1" >"$tmp/$1/Cargo.toml"
        printf '%s\n' "$2" >"$tmp/$1/src/lib.rs"
    }

    expect() { # $1 want (pass|fail), $2 label, $3 log, $4 floors, [$5 = nofix]
        local want="$1" label="$2" log="$3" floors="$4" tag="${5:-}" rc=0
        # The self-test must not pass by reading nothing: if the fixture log has no
        # Doc-tests section, the case proves nothing about the verdict.
        if [ "$tag" != "nofix" ] && ! grep -q "Doc-tests" "$log" 2>/dev/null; then
            fail=$((fail + 1))
            echo "  FAIL $label → fixture produced no Doc-tests section, so the case is unproven"
            return
        fi
        ( cd "$tmp" && verdict "$floors" "self-test" "$log" 1 ) >"$tmp/out" 2>&1 || rc=$?
        local got=pass
        [ "$rc" -ne 0 ] && got=fail
        if [ "$got" = "$want" ]; then
            pass=$((pass + 1))
            echo "  ok   $label → $got"
        else
            fail=$((fail + 1))
            echo "  FAIL $label → $got, wanted $want"
            sed 's/^/       /' "$tmp/out"
        fi
    }

    mk ok '/// ```
/// assert_eq!(2 + 2, 4);
/// ```
pub fn adds() {}'
    mk broken '/// ```
/// assert_eq!(2 + 2, 5);
/// ```
pub fn adds() {}'
    mk allignored '/// ```ignore
/// this never compiles, so it can never fail
/// ```
pub fn adds() {}'

    for c in ok broken allignored; do
        ( cd "$tmp/$c" && cargo test --doc >"$tmp/$c.log" 2>&1 ) || true
    done

    # 1. a real doctest passes
    expect pass "one executed, passing doctest" "$tmp/ok.log" "$tmp/nofloors.tsv"
    # 2. a broken doctest fails
    expect fail "a broken doctest" "$tmp/broken.log" "$tmp/nofloors.tsv"
    # 3. discovered but not executed: exactly the trap nextest-adjacent counting sets
    expect fail "doctests present but all ignored" "$tmp/allignored.log" "$tmp/nofloors.tsv"
    # 4. a floor above what executed
    printf 'ok\t2\n' >"$tmp/floor-high.tsv"
    expect fail "floor 2 against 1 executed" "$tmp/ok.log" "$tmp/floor-high.tsv"
    # 5. a floor met exactly
    printf 'ok\t1\n' >"$tmp/floor-met.tsv"
    expect pass "floor 1 against 1 executed" "$tmp/ok.log" "$tmp/floor-met.tsv"
    # 6. no Doc-tests section at all — the output of a command that ran no doc tests
    printf 'Compiling foo v0.1.0\nFinished\n' >"$tmp/empty.log"
    expect fail "no Doc-tests section" "$tmp/empty.log" "$tmp/nofloors.tsv" nofix

    find "$tmp" -type f -delete 2>/dev/null || true
    find "$tmp" -depth -type d -exec rmdir {} + 2>/dev/null || true

    echo "run-doctests self-test: $pass/$((pass + fail)) assertions passed"
    [ "$fail" -eq 0 ]
}

case "${1:-}" in
    --self-test) self_test ;;
    -h | --help) usage ;;
    -p | --package)
        [ $# -ge 2 ] || { echo "run-doctests.sh: $1 needs a crate name" >&2; exit 2; }
        run_package "$2"
        ;;
    "") run_workspace ;;
    *) echo "run-doctests.sh: unknown argument '$1' (try --help)" >&2; exit 2 ;;
esac
