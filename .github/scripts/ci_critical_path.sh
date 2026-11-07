#!/usr/bin/env bash
# ci_critical_path.sh — what a run cost, and which chain decided when it ended (#1437).
#
# The numbers this prints were produced once, by hand, from the API: 43.5 min wall, 159
# job-minutes over 29 jobs, and a barrier where five jobs waited eleven minutes for a `build`
# job that handed them no file. Nobody knew them during a normal push, so every conversation
# about "CI is slow" restarted from zero (#1436). A cost that is not printed is not managed.
#
# What is measured, not modelled:
#
#   · a *stage* is a job id in `.github/ci-graph.tsv`; its legs are the matrix jobs the API
#     reports under that name ("🔎 Lint (cli)", "🔎 Lint (wasm)", …);
#   · a stage's window is min(leg start) → max(leg end), so a stage with six legs costs the
#     slowest leg in wall clock and the sum of all six in runner time — both are printed;
#   · the critical path is walked *backwards from the stage that finished last*, following the
#     declared `needs` edge that finished latest. That is the edge that actually held the run
#     back; the gap between a dependency's end and a dependent's start is printed as `wait`,
#     because a barrier is a cost too and it is the one nobody was measuring.
#
#   ci_critical_path.sh 42229151064                # the table, the path, the totals
#   ci_critical_path.sh                            # the last completed run on main
#   ci_critical_path.sh --json 42229151064         # the same, for ci_budget_report.sh
#   ci_critical_path.sh --self-test                # fixtures: the path must follow the edge
#                                                  # that finished last, and a run with no job
#                                                  # must fail rather than report a cheap run
#   ci_critical_path.sh --help
#
# Reading the API needs `gh` and a token (`GH_TOKEN` in CI). No number here is typed: every one
# comes from the run's own jobs, and `ci_budget_report.sh` compares them to declared budgets.

set -euo pipefail

SELF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
HERE="$(dirname "$SELF")"
ROOT="$(cd "$HERE/../.." && pwd)"

WORKFLOW="$ROOT/.github/workflows/ci.yml"
GRAPH="$ROOT/.github/ci-graph.tsv"
REPO="${GITHUB_REPOSITORY:-}"
RUN="latest"
JSON=0
SELF_TEST=0
RUN_JSON=""
JOBS_JSON=""

while [ $# -gt 0 ]; do
  case "$1" in
    --json) JSON=1; shift ;;
    --self-test) SELF_TEST=1; shift ;;
    --repo)
      [ $# -ge 2 ] || { echo "ci_critical_path: --repo requires owner/name" >&2; exit 2; }
      REPO="$2"; shift 2 ;;
    --graph)
      [ $# -ge 2 ] || { echo "ci_critical_path: --graph requires a file" >&2; exit 2; }
      GRAPH="$2"; shift 2 ;;
    --workflow)
      [ $# -ge 2 ] || { echo "ci_critical_path: --workflow requires a file" >&2; exit 2; }
      WORKFLOW="$2"; shift 2 ;;
    --run-json)
      [ $# -ge 2 ] || { echo "ci_critical_path: --run-json requires a file" >&2; exit 2; }
      RUN_JSON="$2"; shift 2 ;;
    --jobs-json)
      [ $# -ge 2 ] || { echo "ci_critical_path: --jobs-json requires a file" >&2; exit 2; }
      JOBS_JSON="$2"; shift 2 ;;
    -h|--help)
      sed -n '2,28p' "$SELF" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    -*) echo "ci_critical_path: unknown argument '$1'" >&2; exit 2 ;;
    *) RUN="$1"; shift ;;
  esac
done

# ── the renderer: one code path for fixtures and for a real run ─────────────
# Fixtures exist because the critical path is the one number nobody can check by eye: a wrong
# walk still prints a plausible chain. The seeds are the only thing that can tell.
render() { # render <run.json> <jobs.json>
  python3 - "$1" "$2" "$WORKFLOW" "$GRAPH" "$JSON" <<'PY'
import datetime as dt
import json
import re
import sys

run_path, jobs_path, workflow_path, graph_path, as_json = sys.argv[1:6]
as_json = as_json == "1"


def stamp(value):
    return dt.datetime.fromisoformat(value.replace("Z", "+00:00"))


run = json.load(open(run_path, encoding="utf-8"))


def read_jobs(path):
    """`gh api --paginate` writes one JSON object per page; read them all, never just the first.

    A run with more than 100 jobs is the case this exists for: taking the first page would
    report a cheaper run than the one that happened, which is the one error a cost report
    cannot make.
    """
    text = open(path, encoding="utf-8").read()
    decoder = json.JSONDecoder()
    collected, index = [], 0
    while index < len(text):
        while index < len(text) and text[index].isspace():
            index += 1
        if index >= len(text):
            break
        chunk, index = decoder.raw_decode(text, index)
        if isinstance(chunk, list):
            collected.extend(chunk)
        elif isinstance(chunk, dict):
            collected.extend(chunk.get("jobs", []))
    return collected


jobs = read_jobs(jobs_path)

# ── job key → display name, read from the workflow ──────────────────────────
# The API reports a display name ("🔎 Lint (cli)") and never the workflow's job id, so the
# mapping has to come from somewhere. Reading it from the workflow keeps it derived: rename a
# job and the report follows, instead of a table that drifts until someone notices.
names = {}
key = None
for line in open(workflow_path, encoding="utf-8"):
    m = re.match(r"^  ([A-Za-z0-9_.-]+):\s*$", line)
    if m:
        key = m.group(1)
        continue
    m = re.match(r"^    name:\s*(.+?)\s*$", line)
    if m and key:
        names.setdefault(key, m.group(1))


def stem(display):
    # "🔎 Lint (cli)" and "🔎 Lint (${{ matrix.crate }})" are the same stage: the leg is the
    # parenthesised part, and it is the only difference between them.
    return re.sub(r"\s*\([^()]*\)\s*$", "", display).strip()


stem_to_key = {stem(display): k for k, display in names.items()}


def stage_of(display):
    match = stem_to_key.get(stem(display))
    if match:
        return match
    # A job the workflow no longer has (a renamed stage, or an older run): keep it, and name it
    # so the report can show a stage that exists in the run and not in the graph.
    return re.sub(r"[^a-z0-9]+", "-", stem(display).lower()).strip("-") or display


# ── the declared graph ──────────────────────────────────────────────────────
needs = {}
for line in open(graph_path, encoding="utf-8"):
    if not line.strip() or line.lstrip().startswith("#"):
        continue
    fields = line.rstrip("\n").split("\t")
    if len(fields) < 2:
        continue
    stage, need = fields[0].strip(), fields[1].strip()
    needs[stage] = [] if need in ("-", "") else [t.strip() for t in need.split(",") if t.strip()]

# ── stages, from the run's own jobs ─────────────────────────────────────────
stages = {}
for job in jobs:
    if not job.get("started_at") or not job.get("completed_at"):
        continue  # never ran, or still running: not a cost that can be priced
    start = stamp(job["started_at"])
    end = stamp(job["completed_at"])
    name = job.get("name", "?")
    stage = stage_of(name)
    entry = stages.setdefault(stage, {
        "stage": stage, "legs": 0, "minutes": 0.0, "max_leg": 0.0,
        "start": start, "end": end, "conclusions": set(),
    })
    minutes = (end - start).total_seconds() / 60
    entry["legs"] += 1
    entry["minutes"] += minutes
    entry["max_leg"] = max(entry["max_leg"], minutes)
    entry["start"] = min(entry["start"], start)
    entry["end"] = max(entry["end"], end)
    entry["conclusions"].add(job.get("conclusion") or "?")

run_start = stamp(run["run_started_at"])
wall = (stamp(run["updated_at"]) - run_start).total_seconds() / 60
job_minutes = sum(s["minutes"] for s in stages.values())

if not stages:
    print("❌ ci_critical_path: the run has no completed job — 0 jobs is a broken scan, not a fast run",
          file=sys.stderr)
    sys.exit(1)

# ── the critical path: backwards from the stage that finished last ──────────
finishing = max(stages.values(), key=lambda s: s["end"])
chain = [finishing]
seen = {finishing["stage"]}
current = finishing
while True:
    declared = [n for n in needs.get(current["stage"], []) if n in stages]
    if not declared:
        break
    binding = max(declared, key=lambda n: stages[n]["end"])
    if binding in seen:
        break
    seen.add(binding)
    chain.append(stages[binding])
    current = stages[binding]
chain.reverse()

path = []
for index, stage in enumerate(chain):
    row = {
        "stage": stage["stage"],
        "minutes": round(stage["max_leg"], 1),
        "legs": stage["legs"],
        "start_offset": round((stage["start"] - run_start).total_seconds() / 60, 1),
    }
    if index:
        previous = chain[index - 1]
        row["waited"] = round(max(0.0, (stage["start"] - previous["end"]).total_seconds() / 60), 1)
    path.append(row)

report = {
    "run": int(run["id"]),
    "branch": run.get("head_branch"),
    "sha": (run.get("head_sha") or "")[:8],
    "conclusion": run.get("conclusion"),
    "url": run.get("html_url"),
    "wall_minutes": round(wall, 1),
    "job_minutes": round(job_minutes, 1),
    "jobs": len(jobs),
    "stages": [
        {
            "stage": s["stage"],
            "legs": s["legs"],
            "minutes": round(s["minutes"], 1),
            "max_leg": round(s["max_leg"], 1),
            "start_offset": round((s["start"] - run_start).total_seconds() / 60, 1),
            "end_offset": round((s["end"] - run_start).total_seconds() / 60, 1),
            "conclusions": sorted(s["conclusions"]),
            "declared": s["stage"] in needs,
        }
        for s in sorted(stages.values(), key=lambda s: (s["start"], s["stage"]))
    ],
    "critical_path": path,
}

if as_json:
    print(json.dumps(report, indent=2, sort_keys=True))
    sys.exit(0)

print(f"run {report['run']}  {report['branch']}  {report['sha']}  {report['conclusion']}")
print(f"wall {report['wall_minutes']:.1f} min   job-minutes {report['job_minutes']:.1f}   "
      f"jobs {report['jobs']}   stages {len(report['stages'])}")
print()
print(f"  {'stage':<22} {'legs':>4} {'minutes':>8} {'max leg':>8} {'start':>7} {'end':>7}  verdict")
for s in report["stages"]:
    verdict = ",".join(s["conclusions"])
    if not s["declared"]:
        verdict += " (not in ci-graph.tsv)"
    print(f"  {s['stage']:<22} {s['legs']:>4} {s['minutes']:>8.1f} {s['max_leg']:>8.1f} "
          f"{s['start_offset']:>7.1f} {s['end_offset']:>7.1f}  {verdict}")

print()
print("  critical path (the chain that decided when the run ended):")
for index, row in enumerate(path):
    arrow = "→" if index else " "
    waited = f"   wait {row['waited']:.1f} min after the previous stage" if index and row.get("waited") else ""
    print(f"  {arrow} {row['stage']:<20} {row['minutes']:>6.1f} min"
          f"  starts +{row['start_offset']:.1f}{waited}")

spent = sum(row["minutes"] for row in path)
print()
print(f"  path {spent:.1f} min of {report['wall_minutes']:.1f} min wall "
      f"({job_minutes:.1f} job-minutes over {report['jobs']} jobs)")
PY
}

# ── self-test ───────────────────────────────────────────────────────────────
if [ "$SELF_TEST" -eq 1 ]; then
  echo "── ci_critical_path --self-test ──"
  TMP="$(mktemp -d)"
  trap 'rm -rf "$TMP"' EXIT
  passes=0
  fails=0

  # fixture <dir> <spec>: spec is "stage:start:end:needs" per line, minutes from run start.
  fixture() { # fixture <dir> <"name:start_min:end_min|…">, minutes from the run's start
    mkdir -p "$1"
    python3 - "$1" "$2" <<'FIX'
import json
import sys

out, spec = sys.argv[1], sys.argv[2]


def at(minutes):
    seconds = int(round(float(minutes) * 60))
    return f"2026-11-06T10:{seconds // 60:02d}:{seconds % 60:02d}Z"


rows = [line.split(":") for line in spec.split("|") if line.strip()]
jobs = [{"name": name, "status": "completed", "conclusion": "success",
         "started_at": at(start), "completed_at": at(end)} for name, start, end in rows]
last = max(float(end) for _, _, end in rows)
json.dump({"id": 1, "head_branch": "main", "head_sha": "0" * 40, "conclusion": "success",
           "run_started_at": at(0), "updated_at": at(last)}, open(f"{out}/run.json", "w"))
json.dump({"jobs": jobs}, open(f"{out}/jobs.json", "w"))
FIX
  }

  # The display names are the workflow's, so the stage mapping is exercised too.
  path_of() { # path_of <dir> -> "stage,stage,…"
    bash "$SELF" --json --run-json "$1/run.json" --jobs-json "$1/jobs.json" 2>/dev/null \
      | python3 -c 'import json,sys; print(",".join(r["stage"] for r in json.load(sys.stdin)["critical_path"]))'
  }

  expect_path() { # expect_path <label> <dir> <expected>
    local got
    set +e
    got="$(path_of "$2")"
    set -e
    if [ "$got" = "$3" ]; then
      echo "  ✅ $1"
      passes=$((passes + 1))
    else
      echo "  ❌ $1 (path '$got', wanted '$3')"
      fails=$((fails + 1))
    fi
  }

  # S1: a straight chain. Each stage starts when its dependency ends, so the path is the chain.
  # The stages are the ones the graph declares now — the fixture follows `.github/ci-graph.tsv`,
  # and the seed fails if a stage is renamed or a job deleted (#1438 took `build`).
  fixture "$TMP/chain" \
    "📋 Workspace:0:0.2|🧪 Test (cli):0.2:6|🚪 Final Gate:6:6.5"
  expect_path "a straight chain: workspace → test → gate" "$TMP/chain" \
    "workspace,test,gate"

  # S2: the binder. The gate needs both `lint` and `test`; `lint` ends earlier, so the path must
  # follow `test`. A walk that took the first declared need would name lint.
  fixture "$TMP/binder" \
    "📋 Workspace:0:0.2|🔎 Lint (cli):0.2:4|🧪 Test (cli):0.2:14|🚪 Final Gate:14:14.5"
  expect_path "two needs, one binding edge: the later one is the path" "$TMP/binder" \
    "workspace,test,gate"

  # S3: a run with no completed job is a broken scan, not a 0-minute run.
  fixture "$TMP/empty" "🧪 Test (cli):0:0"
  python3 - "$TMP/empty/jobs.json" <<'PY'
import json
import sys

path = sys.argv[1]
json.dump({"jobs": [{"name": "🧪 Test (cli)", "status": "queued", "conclusion": None,
                     "started_at": None, "completed_at": None}]}, open(path, "w"))
PY
  set +e
  bash "$SELF" --json --run-json "$TMP/empty/run.json" --jobs-json "$TMP/empty/jobs.json" \
    >"$TMP/empty.out" 2>&1
  got=$?
  set -e
  if [ "$got" -eq 1 ] && grep -q '0 jobs is a broken scan' "$TMP/empty.out"; then
    echo "  ✅ a run with no completed job fails instead of reporting a cheap run"
    passes=$((passes + 1))
  else
    echo "  ❌ a run with no completed job fails (exit $got)"
    sed 's/^/     /' "$TMP/empty.out"
    fails=$((fails + 1))
  fi

  echo "  self-test: $passes passed, $fails failed"
  [ "$fails" -eq 0 ] || exit 1
  echo "✅ ci_critical_path self-test"
  exit 0
fi

for f in "$WORKFLOW" "$GRAPH"; do
  [ -f "$f" ] || {
    echo "❌ ci_critical_path: $f does not exist — a report over a graph that was not read is not a report" >&2
    exit 1; }
done

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

if [ -n "$RUN_JSON" ] || [ -n "$JOBS_JSON" ]; then
  if [ ! -f "${RUN_JSON:-/nonexistent}" ] || [ ! -f "${JOBS_JSON:-/nonexistent}" ]; then
    echo "ci_critical_path: --run-json and --jobs-json must both name a readable file" >&2
    exit 2
  fi
  cp "$RUN_JSON" "$TMP/run.json"
  cp "$JOBS_JSON" "$TMP/jobs.json"
else
  if [ -z "$REPO" ]; then
    REPO="$(cd "$ROOT" && gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null || true)"
  fi
  [ -n "$REPO" ] || {
    echo "❌ ci_critical_path: no repository — pass --repo owner/name or set GITHUB_REPOSITORY" >&2
    exit 1; }

  if [ "$RUN" = "latest" ]; then
    RUN="$(gh api "repos/$REPO/actions/runs?branch=main&status=completed&per_page=1" \
      --jq '.workflow_runs[0].id // empty')"
    [ -n "$RUN" ] || { echo "❌ ci_critical_path: no completed run on main to price" >&2; exit 1; }
  fi

  gh api "repos/$REPO/actions/runs/$RUN" > "$TMP/run.json"
  gh api "repos/$REPO/actions/runs/$RUN/jobs?per_page=100" --paginate > "$TMP/jobs.json"
fi

render "$TMP/run.json" "$TMP/jobs.json"
