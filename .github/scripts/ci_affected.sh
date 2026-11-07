#!/usr/bin/env bash
# ci_affected.sh — which subjects a change can reach (#1441).
#
# Every push ran 29 jobs regardless of the diff, so a three-file change in
# `engine/src/precondition/` paid for six crates, the wasm gates and the conformance replay. This
# derives the subjects from `.github/ci-affected.tsv`, the same table `.pi/scripts/local-ci.sh
# --affected` reads — one table, two consumers, so the pipeline stops being the first oracle.
#
# Fail closed, loudly. A path that matches no row is **not** skipped: it prints
# `unmapped: <path> → running ALL` and the full set runs. A shared path (`Cargo.lock`, any
# `Cargo.toml`, `.github/**`, `.pi/**`, `.config/**`, `deny.toml`) expands to ALL for the same
# reason: a change that can reach everything may not be assumed to reach one thing.
#
#   ci_affected.sh --base origin/main                 # diff the branch against a base
#   ci_affected.sh --base origin/main --files a b c   # decide about an explicit file list
#   ci_affected.sh --base origin/main --list          # one subject per line, for shell use
#   ci_affected.sh --base origin/main --matrix        # GitHub job outputs: all, subjects, crates
#
# Output (default):
#   {"subjects":["engine","portable","conformance"],"all":false,"unmapped":[]}
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MAP="$REPO_ROOT/.github/ci-affected.tsv"
BASE="origin/main"
LIST_ONLY=0
MATRIX=0
GITHUB_OUT=0
FORCE_ALL=0
FILES=()

while [ $# -gt 0 ]; do
  case "$1" in
    --base)
      [ $# -ge 2 ] || { echo "ci_affected: --base requires a ref" >&2; exit 2; }
      BASE="$2"; shift 2 ;;
    --map)
      [ $# -ge 2 ] || { echo "ci_affected: --map requires a file" >&2; exit 2; }
      MAP="$2"; shift 2 ;;
    --files)
      shift
      while [ $# -gt 0 ] && [ "${1#-}" = "$1" ]; do FILES+=("$1"); shift; done ;;
    --list) LIST_ONLY=1; shift ;;
    --matrix) MATRIX=1; shift ;;
    --github) MATRIX=1; GITHUB_OUT=1; shift ;;
    --all) FORCE_ALL=1; shift ;;
    -h|--help)
      sed -n '2,24p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *) echo "ci_affected: unknown argument '$1'" >&2; exit 2 ;;
  esac
done

[ -f "$MAP" ] || { echo "❌ ci_affected: $MAP does not exist — a narrowing with no map runs nothing" >&2; exit 1; }

if [ "${#FILES[@]}" -eq 0 ]; then
  # `--base X` diffs X...HEAD, which is what a branch means. A base that cannot be resolved is
  # not a reason to narrow: say so and run everything.
  if ! git -C "$REPO_ROOT" rev-parse --verify --quiet "$BASE" >/dev/null; then
    echo "  (base '$BASE' does not resolve — running ALL)" >&2
    if [ "$LIST_ONLY" -eq 1 ]; then echo "ALL"; else printf '{"subjects":[],"all":true,"unmapped":[]}\n'; fi
    exit 0
  fi
  while IFS= read -r path; do
    [ -n "$path" ] && FILES+=("$path")
  done < <(git -C "$REPO_ROOT" diff --name-only "$BASE...HEAD" 2>/dev/null || true)
fi

python3 - "$MAP" "$LIST_ONLY" "$MATRIX" "$GITHUB_OUT" "$FORCE_ALL" "$REPO_ROOT" "${FILES[@]+"${FILES[@]}"}" <<'PY'
import fnmatch
import json
import os
import sys

map_path = sys.argv[1]
list_only = sys.argv[2] == "1"
matrix_mode = sys.argv[3] == "1"
github_out = sys.argv[4] == "1"
force_all = sys.argv[5] == "1"
root = sys.argv[6]
files = sys.argv[7:]

rows = []
for line in open(map_path, encoding="utf-8"):
    if not line.strip() or line.lstrip().startswith("#"):
        continue
    fields = line.rstrip("\n").split("\t")
    if len(fields) < 2:
        continue
    rows.append((fields[0].strip(), [s.strip() for s in fields[1].split(",") if s.strip()]))

subjects, unmapped, everything = set(), [], force_all

# An empty diff is not a reason to narrow either: a run with no files to look at (a re-run, a
# scheduled trigger, a shallow checkout) has no evidence about what it can reach.
if not files and not force_all:
    everything = True

for path in files:
    hits = [subjects_of for glob, subjects_of in rows if fnmatch.fnmatchcase(path, glob)]
    if not hits:
        unmapped.append(path)
        everything = True
        continue
    for group in hits:
        if group == ["ALL"]:
            everything = True
        subjects.update(group)

if everything:
    subjects = set()

# The job ids the pipeline runs. A subject that is a crate directory narrows the `lint`/`test`
# matrices; every other subject is a job.
JOBS = ["lint", "test", "portable", "coverage", "audit", "docs", "integration", "conformance",
        "shell", "wasm-gates"]
CRATES = sorted(name for name in os.listdir(root)
                if os.path.isdir(os.path.join(root, name, ".pi", "scripts")))

if matrix_mode:
    # Three outputs, because a matrix job is narrowed by its matrix and a single job by its
    # `if`: `lint` and `test` run for the crates the diff can reach, everything else runs or
    # does not run as a whole.
    if everything:
        selected_jobs = list(JOBS)
        selected_crates = list(CRATES)
    else:
        chosen = set(subjects) - {"none"}
        selected_crates = [c for c in CRATES if c in chosen]
        selected_jobs = [j for j in JOBS if j in chosen]
        # A crate subject means "that crate's leg", which is `lint` and `test` for it.
        if selected_crates:
            for job in ("lint", "test"):
                if job not in selected_jobs:
                    selected_jobs.append(job)
    if github_out:
        # `$GITHUB_OUTPUT` lines, because a pipeline that narrows must not do it through a
        # JSON parser written inside YAML. `crates` is the matrix expression's input.
        print(f"all={'true' if everything else 'false'}")
        print("subjects=" + " " + " ".join(sorted(selected_jobs)) + " ")
        print("crates=" + json.dumps(selected_crates))
        sys.exit(0)
    print(json.dumps({
        "all": "true" if everything else "false",
        "subjects": " " + " ".join(sorted(selected_jobs)) + " ",
        "crates": selected_crates,
        "unmapped": unmapped,
    }, sort_keys=True))
    sys.exit(0)

if list_only:
    print("ALL" if everything else "\n".join(sorted(subjects)))
    sys.exit(0)

for path in unmapped:
    print(f"unmapped: {path} -> running ALL", file=sys.stderr)
print(json.dumps({
    "subjects": sorted(subjects),
    "all": everything,
    "unmapped": unmapped,
}, sort_keys=True))
PY
