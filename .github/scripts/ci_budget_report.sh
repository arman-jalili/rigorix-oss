#!/usr/bin/env bash
# ci_budget_report.sh — compare a run's cost against declared budgets, and fail when it exceeds
# one (#1437). The verdict is the exit status; a number in a log is not a check.
#
# `.github/ci-budgets.tsv` is the only place a budget is typed. Every row carries the date and
# the measurement it was set from — a budget that was not set from a number is a preference,
# and it cannot be compared to the number it should be.
#
# Three rules, each with a seed in `--self-test`:
#
#   1. a stage whose slowest leg exceeds its budget fails, naming the stage and the overage;
#   2. a total that exceeds its ceiling fails even when every stage is inside its own budget —
#      the sum is the subject of the total, not of any one stage;
#   3. a budget may be **lowered** and may not be **raised** without a fresh `set_from`. That is
#      the same two-half ratchet as `.pi/coverage-layers.tsv`: the lower half is enforced by the
#      comparison, the upper half by comparing the file with the base commit. A row that is
#      deleted to make a breach disappear fails as well.
#
#   ci_budget_report.sh                      # the last completed run on main
#   ci_budget_report.sh --run 42229151064
#   ci_budget_report.sh --self-test
#   ci_budget_report.sh --base origin/main   # the ratchet against another ref
#
# A stage the run reports and the file does not price fails: an unpriced cost is how 159
# job-minutes stayed invisible in the first place.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BUDGETS="$ROOT/.github/ci-budgets.tsv"
MEASURED=""
RUN="latest"
BASE="origin/main"
BASE_FILE=""
SELF_TEST=0

while [ $# -gt 0 ]; do
  case "$1" in
    --self-test) SELF_TEST=1; shift ;;
    --run)
      [ $# -ge 2 ] || { echo "ci_budget_report: --run requires a run id" >&2; exit 2; }
      RUN="$2"; shift 2 ;;
    --measured)
      [ $# -ge 2 ] || { echo "ci_budget_report: --measured requires a json file" >&2; exit 2; }
      MEASURED="$2"; shift 2 ;;
    --budgets)
      [ $# -ge 2 ] || { echo "ci_budget_report: --budgets requires a file" >&2; exit 2; }
      BUDGETS="$2"; shift 2 ;;
    --base)
      [ $# -ge 2 ] || { echo "ci_budget_report: --base requires a ref" >&2; exit 2; }
      BASE="$2"; shift 2 ;;
    --base-file)
      [ $# -ge 2 ] || { echo "ci_budget_report: --base-file requires a file" >&2; exit 2; }
      BASE_FILE="$2"; shift 2 ;;
    -h|--help)
      sed -n '2,28p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    -*) echo "ci_budget_report: unknown argument '$1'" >&2; exit 2 ;;
    *) echo "ci_budget_report: unexpected argument '$1'" >&2; exit 2 ;;
  esac
done

# ── the comparison, once, for fixtures and for runs ─────────────────────────
CORE=$(cat <<'PY'
import json
import os
import sys

budgets_path, measured_path, base_path = sys.argv[1:4]


def read_budgets(path):
    """stage -> (budget, set_at, set_from); `problems` keeps the rows that cannot be read."""
    rows, problems, order = {}, [], []
    for number, line in enumerate(open(path, encoding="utf-8"), start=1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        fields = line.rstrip("\n").split("\t")
        stage = fields[0].strip() if fields else ""
        if len(fields) < 4:
            problems.append(f"`{path}`:{number}: `{line.strip()}` has {len(fields)} field(s), "
                            f"needs stage, budget, set_at, set_from")
            continue
        try:
            value = float(fields[1])
        except ValueError:
            problems.append(f"`{path}`:{number}: `{stage}` has budget `{fields[1]}`, which is not a number")
            continue
        set_at, set_from = fields[2].strip(), fields[3].strip()
        if not set_from:
            problems.append(f"`{path}`:{number}: `{stage}` has no `set_from` — a budget that cannot be "
                            f"compared to the measurement it came from is not a check")
        if stage in rows:
            # Two rows for one subject: the last wins (so a later line can tighten a budget
            # without editing the file's history), and the duplicate is named.
            problems.append(f"`{stage}` is declared twice in `{path}` — the last row wins, and a "
                            f"second number for one subject is how a file and its reader disagree")
        else:
            order.append(stage)
        rows[stage] = (value, set_at, set_from)
    return rows, problems, order


budgets, problems, order = read_budgets(budgets_path)
measured = json.load(open(measured_path, encoding="utf-8"))
findings = list(problems)

STAGES = {s["stage"]: s for s in measured["stages"]}
TOTALS = {"TOTAL_MINUTES": measured["wall_minutes"], "TOTAL_JOB_MINUTES": measured["job_minutes"]}

# ── 1. every stage the run reports must be priced ───────────────────────────
for stage in sorted(STAGES):
    if stage not in budgets:
        findings.append(f"`{stage}` ran for {STAGES[stage]['minutes']:.1f} job-minute(s) and has no budget "
                        f"— an unpriced stage is a cost nobody is watching")
for stage in order:
    if stage not in STAGES and stage not in TOTALS:
        findings.append(f"`{stage}` has a budget and did not run — a budget for a stage that no longer "
                        f"exists cannot be breached, so it cannot be a check")

# ── 2. a slowest leg over budget fails ──────────────────────────────────────
for stage in order:
    if stage not in STAGES:
        continue
    value = budgets[stage][0]
    measured_value = STAGES[stage]["max_leg"]
    if measured_value > value:
        findings.append(f"`{stage}` took {measured_value:.1f} min on its slowest leg, budget {value:.0f} "
                        f"— over by {measured_value - value:.1f} min")

# ── 3. a total over its ceiling fails, even when every stage is inside ──────
for total, measured_value in TOTALS.items():
    if total not in budgets:
        findings.append(f"`{total}` has no ceiling — a sum nobody declared cannot be exceeded, or managed")
        continue
    value = budgets[total][0]
    if measured_value > value:
        findings.append(f"`{total}` is {measured_value:.1f} min, ceiling {value:.0f} "
                        f"— over by {measured_value - value:.1f} min")

# ── 4. the ratchet: lowered is a win, raised needs a new measurement ────────
if base_path:
    base, base_problems, base_order = read_budgets(base_path)
    findings.extend(f"base {p}" for p in base_problems)
    for stage in order:
        if stage not in base:
            continue  # a new row is a new subject, not a raise
        value, _, set_from = budgets[stage]
        old, _, old_from = base[stage]
        if value > old and set_from == old_from:
            findings.append(f"`{stage}` was raised {old:.0f} → {value:.0f} min with the same `set_from` — "
                            f"a raise must name what regressed, or it is a lower standard")
    for stage in base_order:
        if stage not in budgets:
            findings.append(f"`{stage}` lost its budget (base had {base[stage][0]:.0f} min) — deleting a "
                            f"row makes a breach invisible, not fixed")

# ── the table ───────────────────────────────────────────────────────────────
print(f"run {measured['run']}  {measured.get('branch')}  {measured.get('sha')}  "
      f"wall {measured['wall_minutes']:.1f} min  job-minutes {measured['job_minutes']:.1f}")
print(f"  {'row':<20} {'measured':>9} {'budget':>7}  verdict")
failed_rows = set()
for stage in order:
    if stage in STAGES:
        measured_value, column = STAGES[stage]["max_leg"], "max leg"
    elif stage in TOTALS:
        measured_value, column = TOTALS[stage], "total"
    else:
        print(f"  {stage:<20} {'—':>9} {budgets[stage][0]:>7.0f}  not in this run")
        failed_rows.add(stage)
        continue
    value = budgets[stage][0]
    over = measured_value > value
    if over:
        failed_rows.add(stage)
    mark = f"❌ over by {measured_value - value:.1f}" if over else "ok"
    print(f"  {stage:<20} {measured_value:>9.1f} {value:>7.0f}  {mark}   ({column})")

errors = [f for f in findings if not f.startswith("base ")]
if findings:
    print()
    for finding in findings:
        print(f"  ✗ {finding}")

print()
if errors:
    print(f"❌ ci budget: {len(errors)} breach(es)")
    sys.exit(1)
print(f"✅ ci budget: {len(STAGES)} stage(s) and {len(TOTALS)} total(s) inside their declared budgets")
PY
)

run_report() { # run_report <measured.json> <budgets.tsv> [base.tsv]
  python3 -c "$CORE" "$2" "$1" "${3:-}"
}

# ── self-test: every rule must fail for its own reason, and pass when it should ─
if [ "$SELF_TEST" -eq 1 ]; then
  echo "── ci_budget_report --self-test ──"
  TMP="$(mktemp -d)"
  trap 'rm -rf "$TMP"' EXIT
  passes=0
  fails=0

  write_measured() { # write_measured <file> <wall> <jobminutes> <stage:maxleg,...>
    local file="$1" wall="$2" jobmin="$3" stages="$4"
    python3 - "$file" "$wall" "$jobmin" "$stages" <<'PY'
import json
import sys

path, wall, jobmin, stages = sys.argv[1:5]
rows = []
for item in [s for s in stages.split(",") if s]:
    name, value = item.split(":")
    rows.append({"stage": name, "legs": 1, "minutes": float(value), "max_leg": float(value),
                 "start_offset": 0.0, "end_offset": float(value), "conclusions": ["success"],
                 "declared": True})
json.dump({"run": 1, "branch": "main", "sha": "0000000", "conclusion": "success",
           "wall_minutes": float(wall), "job_minutes": float(jobmin), "jobs": len(rows),
           "stages": rows, "critical_path": []}, open(path, "w"), indent=2)
PY
  }

  budgets_ok() { # budgets_ok <file> <lint> <total>
    printf '# stage\tbudget\tset_at\tset_from\n' > "$1"
    printf 'lint\t%s\t2026-11-06\t"run 42229151064: 7.7"\n' "$2" >> "$1"
    printf 'coverage\t16\t2026-11-06\t"run 42229151064: 15.3"\n' >> "$1"
    printf 'TOTAL_MINUTES\t%s\t2026-11-06\t"run 42229151064: 43.5"\n' "$3" >> "$1"
    printf 'TOTAL_JOB_MINUTES\t160\t2026-11-06\t"run 42229151064: 159.4"\n' >> "$1"
  }

  expect() { # expect <0|1> <label> <must-contain> <measured> <budgets> [base]
    local want="$1" label="$2" needle="$3" measured="$4" budgets="$5" base="${6:-}" got out
    set +e
    out=$(run_report "$measured" "$budgets" "$base" 2>&1)
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

  budgets_ok "$TMP/good.tsv" 8 44
  write_measured "$TMP/good.json" 43.5 159.4 "lint:7.7,coverage:15.3"
  expect 0 "a run inside every budget passes" "" "$TMP/good.json" "$TMP/good.tsv"

  budgets_ok "$TMP/tight.tsv" 8 44
  write_measured "$TMP/slowleg.json" 43.5 159.4 "lint:9.0,coverage:15.3"
  expect 1 "a slowest leg over budget fails, naming the stage" '`lint` took 9.0 min' \
      "$TMP/slowleg.json" "$TMP/tight.tsv"

  budgets_ok "$TMP/ceiling.tsv" 8 20
  write_measured "$TMP/sum.json" 30 159.4 "lint:7.7,coverage:15.3"
  expect 1 "a total over its ceiling fails with every stage inside" '`TOTAL_MINUTES` is 30.0 min' \
      "$TMP/sum.json" "$TMP/ceiling.tsv"

  budgets_ok "$TMP/nojust.tsv" 8 44
  python3 - "$TMP/nojust.tsv" <<'PY'
import sys
path = sys.argv[1]
lines = [l for l in open(path) if not l.startswith("coverage")]
open(path, "w").writelines(lines)
PY
  write_measured "$TMP/nocov.json" 43.5 159.4 "lint:7.7,coverage:15.3"
  expect 1 "a stage with no budget row fails" '`coverage` ran for 15.3 job-minute(s)' \
      "$TMP/nocov.json" "$TMP/nojust.tsv"

  printf '# stage\tbudget\tset_at\tset_from\nlint\t8\t2026-11-06\t\n' > "$TMP/blank.tsv"
  write_measured "$TMP/one.json" 43.5 159.4 "lint:7.7"
  expect 1 "a budget with no set_from fails" 'has no `set_from`' "$TMP/one.json" "$TMP/blank.tsv"

  printf 'lint\t9\t2026-11-05\t"run 41000000000: 8.4"\n' > "$TMP/base.tsv"
  printf '# stage\tbudget\tset_at\tset_from\nlint\t10\t2026-11-06\t"run 41000000000: 8.4"\n' \
      > "$TMP/raised.tsv"
  expect 1 "a budget raised with the same set_from fails" 'was raised 9 → 10 min' \
      "$TMP/one.json" "$TMP/raised.tsv" "$TMP/base.tsv"

  budgets_ok "$TMP/lowered.tsv" 8 44
  # Same `set_from` as the base row, a lower number: the ratchet's cheap half.
  python3 - "$TMP/lowered.tsv" <<'PY'
import sys
path = sys.argv[1]
lines = []
for line in open(path):
    if line.startswith("lint\t"):
        line = 'lint\t8\t2026-11-06\t"run 41000000000: 8.4"\n'
    lines.append(line)
open(path, "w").writelines(lines)
PY
  expect 0 "a budget lowered after a win passes" "" "$TMP/good.json" "$TMP/lowered.tsv" "$TMP/base.tsv"

  echo "  self-test: $passes passed, $fails failed"
  [ "$fails" -eq 0 ] || exit 1
  echo "✅ ci_budget_report self-test"
  exit 0
fi

# ── the real report ─────────────────────────────────────────────────────────
[ -f "$BUDGETS" ] || { echo "❌ ci_budget_report: $BUDGETS does not exist — no budgets, no check" >&2; exit 1; }

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

if [ -n "$MEASURED" ]; then
  [ -f "$MEASURED" ] || { echo "❌ ci_budget_report: $MEASURED does not exist" >&2; exit 1; }
else
  bash "$HERE/ci_critical_path.sh" "$RUN" --json > "$TMP/measured.json"
  MEASURED="$TMP/measured.json"
fi

BASE_TMP=""
if [ -z "$BASE_FILE" ] && [ -n "$BASE" ]; then
  # The ratchet needs the base commit's file. Absent history is not a failure of the run — say
  # so and compare what can be compared, rather than silently skipping the upper half.
  if git -C "$ROOT" rev-parse --verify --quiet "$BASE:.github/ci-budgets.tsv" >/dev/null 2>&1; then
    git -C "$ROOT" show "$BASE:.github/ci-budgets.tsv" > "$TMP/base.tsv"
    BASE_FILE="$TMP/base.tsv"
  else
    echo "  (no $BASE:.github/ci-budgets.tsv — comparing against the current file only)"
  fi
fi

run_report "$MEASURED" "$BUDGETS" "$BASE_FILE"
