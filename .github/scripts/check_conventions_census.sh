#!/usr/bin/env bash
# check_conventions_census.sh — the conventions suite runs once, and it runs somewhere (#1438).
#
# The `check_*.sh` suite is a verdict on the tree: contract implementations, DDD structure,
# architecture conformance. It ran in five jobs — `lint`, `build`, `test`, `docs`, `integration`
# — which is five executions of one answer, and the copy in `build` was the whole reason five
# stages waited ~11 minutes for a job that uploaded nothing. Deleting `build` removes one copy;
# a census is what keeps the other four from growing back.
#
# Three rules, each with a seed:
#
#   1. **exactly one job** invokes the suite. Two jobs means two verdicts on one subject, and a
#      reader cannot tell which one the gate meant.
#   2. that job **runs the suite per crate** (`working-directory: ${{ matrix.crate }}`), because
#      the repo-root copy of each check has no `src/` to inspect and skips out with a green
#      summary while the real check never ran — that is how the suite passed for months.
#   3. **no directory the loop visits is empty.** A glob that matches nothing passes for the
#      wrong reason: it exits 0 having checked nothing.
#
#   bash .github/scripts/check_conventions_census.sh
#   bash .github/scripts/check_conventions_census.sh --self-test
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORKFLOW="$REPO_ROOT/.github/workflows/ci.yml"
SELF_TEST=0

while [ $# -gt 0 ]; do
  case "$1" in
    --self-test) SELF_TEST=1; shift ;;
    --workflow)
      [ $# -ge 2 ] || { echo "check_conventions_census: --workflow requires a file" >&2; exit 2; }
      WORKFLOW="$2"; shift 2 ;;
    --root)
      [ $# -ge 2 ] || { echo "check_conventions_census: --root requires a directory" >&2; exit 2; }
      REPO_ROOT="$2"; shift 2 ;;
    -h|--help)
      sed -n '2,25p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *) echo "check_conventions_census: unknown argument '$1'" >&2; exit 2 ;;
  esac
done

# Print `<job>\t<working-directory>\t<glob>` for every job that invokes the suite.
suite_jobs() {
  awk '
    /^jobs:/ { injobs = 1; next }
    injobs && /^[^[:space:]#]/ { injobs = 0 }
    injobs && /^  [A-Za-z0-9_-]+:[[:space:]]*$/ {
      if (job != "" && found) printf "%s\t%s\t%s\n", job, wd, glob
      job = substr($0, 3); sub(/:[[:space:]]*$/, "", job)
      found = 0; wd = "-"; glob = "-"
      next
    }
    injobs && !/^[[:space:]]*#/ && /check_\*\.sh/ {
      found = 1
      line = $0; sub(/^[[:space:]]*/, "", line)
      if (line ~ /^for[[:space:]]/) {
        g = line; sub(/.*in[[:space:]]+/, "", g); sub(/;.*$/, "", g)
        gsub(/["\047]/, "", g)
        glob = g
      }
    }
    injobs && /^[[:space:]]+working-directory:/ {
      wd = $0; sub(/^[[:space:]]*working-directory:[[:space:]]*/, "", wd)
      gsub(/[[:space:]]+$/, "", wd)
    }
    END { if (job != "" && found) printf "%s\t%s\t%s\n", job, wd, glob }
  ' "$1"
}

check() {
  local workflow="${1:-$WORKFLOW}" root="${2:-$REPO_ROOT}"
  local failures=0 rows count

  [ -f "$workflow" ] || { echo "❌ no workflow: $workflow" >&2; return 1; }

  rows="$(suite_jobs "$workflow")"
  count="$(printf '%s\n' "$rows" | grep -c . || true)"

  # 1. exactly one job.
  if [ "$count" -eq 0 ]; then
    echo "❌ no job runs the \`check_*.sh\` suite — the conventions are enforced by nothing" >&2
    return 1
  fi
  if [ "$count" -gt 1 ]; then
    echo "❌ $count job(s) run the \`check_*.sh\` suite: $(printf '%s' "$rows" | cut -f1 | tr '\n' ' ')" >&2
    echo "   one subject, one verdict: pick the job whose subject the conventions are (#1438)" >&2
    failures=$((failures + 1))
  else
    echo "✅ exactly one job runs the conventions suite: $(printf '%s' "$rows" | cut -f1)"
  fi

  # 2. that job runs it per crate.
  local wd
  wd="$(printf '%s' "$rows" | cut -f2)"
  case "$wd" in
    *matrix.crate*) echo "✅ the suite runs per crate (working-directory: $wd)" ;;
    *)
      echo "❌ the suite runs with working-directory '$wd'" >&2
      echo "   from the repo root each per-crate check has no \`src/\` and skips with a green summary" >&2
      failures=$((failures + 1)) ;;
  esac

  # 3. no directory the loop visits is empty.
  local glob total visited=0
  glob="$(printf '%s' "$rows" | cut -f3)"
  case "$glob" in
    .pi/scripts/ci/check_*.sh) : ;;
    *)
      echo "❌ the suite is invoked as '$glob', not the \`.pi/scripts/ci/check_*.sh\` glob" >&2
      echo "   an explicit list silently omits the next check someone adds" >&2
      failures=$((failures + 1)) ;;
  esac
  total=0
  local dir n
  for dir in "$root"/*/; do
    [ -d "${dir}.pi/scripts/ci" ] || continue
    n="$(find "${dir}.pi/scripts/ci" -maxdepth 1 -name 'check_*.sh' -type f | wc -l | tr -d ' ')"
    visited=$((visited + 1))
    total=$((total + n))
    if [ "$n" -eq 0 ]; then
      echo "❌ ${dir} has a \`.pi/scripts/ci\` with no \`check_*.sh\` — the loop visits it and checks nothing" >&2
      failures=$((failures + 1))
    fi
  done
  if [ "$visited" -eq 0 ] || [ "$total" -eq 0 ]; then
    echo "❌ no \`check_*.sh\` found under any crate — 0 scripts is a broken census, not a clean tree" >&2
    failures=$((failures + 1))
  else
    echo "✅ the glob matches $total script(s) across $visited crate(s)"
  fi

  if [ "$failures" -ne 0 ]; then
    echo "❌ conventions census: $failures finding(s)" >&2
    return 1
  fi
  echo "✅ conventions census: one job, per crate, nothing empty"
}

if [ "$SELF_TEST" -eq 1 ]; then
  echo "── check_conventions_census --self-test ──"
  TMP="$(mktemp -d)"
  trap 'rm -rf "$TMP"' EXIT
  passes=0
  fails=0

  # A minimal tree: two crates, one with checks, one without.
  mkdir -p "$TMP/tree/engine/.pi/scripts/ci" "$TMP/tree/cli/.pi/scripts/ci"
  : > "$TMP/tree/engine/.pi/scripts/ci/check_ddd_structure.sh"
  : > "$TMP/tree/cli/.pi/scripts/ci/check_contracts.sh"

  wf() { # wf <file> <body-of-jobs>
    printf 'name: CI\non: [push]\njobs:\n%s' "$2" > "$1"
  }

  good='  lint:
    name: 🔎 Lint
    needs: [workspace]
    steps:
      - run: |
          for script in .pi/scripts/ci/check_*.sh; do
            [ -f "$script" ] && bash "$script"
          done
        working-directory: ${{ matrix.crate }}
'

  wf "$TMP/good.yml" "$good"
  set +e
  out="$(check "$TMP/good.yml" "$TMP/tree" 2>&1)"; got=$?
  set -e
  if [ "$got" -eq 0 ]; then
    echo "  ✅ one job, per crate, nothing empty: passes"
    passes=$((passes + 1))
  else
    echo "  ❌ one job, per crate, nothing empty: exit $got"
    printf '%s\n' "$out" | sed 's/^/     /'
    fails=$((fails + 1))
  fi

  # S1: the suite is invoked by nobody — the state #1438 exists to prevent in reverse.
  wf "$TMP/none.yml" '  lint:
    name: 🔎 Lint
    steps:
      - run: cargo clippy
'
  set +e
  out="$(check "$TMP/none.yml" "$TMP/tree" 2>&1)"; got=$?
  set -e
  if [ "$got" -eq 1 ] && printf '%s' "$out" | grep -q 'enforced by nothing'; then
    echo "  ✅ no job runs the suite: fails"
    passes=$((passes + 1))
  else
    echo "  ❌ no job runs the suite: exit $got"
    fails=$((fails + 1))
  fi

  # S2: the suite runs in two jobs — two verdicts on one subject.
  wf "$TMP/twice.yml" "$good"'
  test:
    name: 🧪 Test
    steps:
      - run: |
          for script in .pi/scripts/ci/check_*.sh; do bash "$script"; done
        working-directory: ${{ matrix.crate }}
'
  set +e
  out="$(check "$TMP/twice.yml" "$TMP/tree" 2>&1)"; got=$?
  set -e
  if [ "$got" -eq 1 ] && printf '%s' "$out" | grep -q '2 job(s) run'; then
    echo "  ✅ the suite in two jobs: fails, naming both"
    passes=$((passes + 1))
  else
    echo "  ❌ the suite in two jobs: exit $got"
    printf '%s\n' "$out" | sed 's/^/     /'
    fails=$((fails + 1))
  fi

  # S3: a crate whose directory the loop visits has no checks — a glob that matches nothing
  # passes for the wrong reason.
  mkdir -p "$TMP/empty-tree/engine/.pi/scripts/ci" "$TMP/empty-tree/cli/.pi/scripts/ci"
  : > "$TMP/empty-tree/engine/.pi/scripts/ci/check_ddd_structure.sh"
  set +e
  out="$(check "$TMP/good.yml" "$TMP/empty-tree" 2>&1)"; got=$?
  set -e
  if [ "$got" -eq 1 ] && printf '%s' "$out" | grep -q 'checks nothing'; then
    echo "  ✅ a visited crate with no check: fails"
    passes=$((passes + 1))
  else
    echo "  ❌ a visited crate with no check: exit $got"
    printf '%s\n' "$out" | sed 's/^/     /'
    fails=$((fails + 1))
  fi

  # S4: the suite running from the repo root — the shape that passed for months.
  wf "$TMP/root.yml" '  lint:
    name: 🔎 Lint
    steps:
      - run: |
          for script in .pi/scripts/ci/check_*.sh; do bash "$script"; done
'
  set +e
  out="$(check "$TMP/root.yml" "$TMP/tree" 2>&1)"; got=$?
  set -e
  if [ "$got" -eq 1 ] && printf '%s' "$out" | grep -q "working-directory"; then
    echo "  ✅ the suite from the repo root: fails"
    passes=$((passes + 1))
  else
    echo "  ❌ the suite from the repo root: exit $got"
    fails=$((fails + 1))
  fi

  echo "  self-test: $passes passed, $fails failed"
  [ "$fails" -eq 0 ] || exit 1
  echo "✅ check_conventions_census self-test"
  exit 0
fi

check
