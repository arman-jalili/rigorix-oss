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
#
# Output (default):
#   {"subjects":["engine","portable","conformance"],"all":false,"unmapped":[]}
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MAP="$REPO_ROOT/.github/ci-affected.tsv"
BASE="origin/main"
LIST_ONLY=0
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

python3 - "$MAP" "$LIST_ONLY" "${FILES[@]+"${FILES[@]}"}" <<'PY'
import fnmatch
import json
import sys

map_path, list_only = sys.argv[1], sys.argv[2] == "1"
files = sys.argv[3:]

rows = []
for line in open(map_path, encoding="utf-8"):
    if not line.strip() or line.lstrip().startswith("#"):
        continue
    fields = line.rstrip("\n").split("\t")
    if len(fields) < 2:
        continue
    rows.append((fields[0].strip(), [s.strip() for s in fields[1].split(",") if s.strip()]))

subjects, unmapped, everything = set(), [], False

# An empty diff is not a reason to narrow either: a run with no files to look at (a re-run, a
# scheduled trigger, a shallow checkout) has no evidence about what it can reach.
if not files:
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
