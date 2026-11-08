#!/usr/bin/env bash
# validate-affected-graph.sh — the affected map's edges must match the workspace's edges (#1468).
#
# `check_affected_map.sh` asks whether the map is *total*: every path matches a row, every
# subject is a job. That is a property of the table alone. This guard asks the other question:
# is the table consistent with what the crates actually depend on?
#
# The gap it exists for: `engine/src/**` reached only `portable`, which runs
# `cargo check --features portable` — proof that the slice still *compiles*, not that a decision
# is still the same one. Both artifacts link `rigorix-engine`, and both are judged by five
# recorded decision digests. So an engine PR that moved a decision left both artifacts
# untested, and the failure surfaced on the next PR that touched `wasm/` or `wasi/` — the fault
# landing on whoever edited the adjacent file.
#
# Three checks, each derived from `cargo metadata` rather than from a list in this file:
#
#   1. **every member is reachable.** A workspace member with `src/` that no row matches is a
#      crate whose changes narrow to nothing.
#   2. **a dependent's stages are its dependency's too.** If `P` depends on `D`, every stage
#      that a change to `P/src/**` triggers must also be triggered by a change to `D/src/**`:
#      those stages link `D`, so `D` can break them. Written as `S(P) ⊆ S(D)`.
#   3. **a comment that names a stage lists it.** Comments are how a reader learns what a row
#      is for; a comment naming a stage the row does not run is worse than no comment. Only
#      names that cannot be prose are trusted — hyphenated job ids, and `wasi` — so the check
#      does not fire on the word "test" inside a sentence.
#
#   bash .github/scripts/validate-affected-graph.sh
#   bash .github/scripts/validate-affected-graph.sh --self-test
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MAP="$REPO_ROOT/.github/ci-affected.tsv"
SELF_TEST=0

while [ $# -gt 0 ]; do
  case "$1" in
    --self-test) SELF_TEST=1; shift ;;
    --map) [ $# -ge 2 ] || { echo "validate-affected-graph: --map requires a file" >&2; exit 2; }; MAP="$2"; shift 2 ;;
    --root) [ $# -ge 2 ] || { echo "validate-affected-graph: --root requires a directory" >&2; exit 2; }; REPO_ROOT="$2"; shift 2 ;;
    -h|--help) sed -n '2,28p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "validate-affected-graph: unknown argument '$1'" >&2; exit 2 ;;
  esac
done

# `--map` may point outside the tree during the self-test; resolve it before the core runs.
CORE=$(cat <<'PY'
import fnmatch
import json
import os
import re
import subprocess
import sys

root, map_path = sys.argv[1:3]
findings = []

# ── the map ──────────────────────────────────────────────────────────────────
rows = []
for number, line in enumerate(open(map_path, encoding="utf-8"), start=1):
    if not line.strip() or line.lstrip().startswith("#"):
        continue
    fields = line.rstrip("\n").split("\t")
    if len(fields) < 3:
        findings.append(f"`{map_path}`:{number}: a row has {len(fields)} field(s) — the map "
                        f"needs path, subjects and a reason")
        continue
    rows.append({
        "glob": fields[0].strip(),
        "subjects": {s.strip() for s in fields[1].split(",") if s.strip()},
        "why": fields[2].strip(),
        "line": number,
    })

# An empty map is a broken scan, not a tree with no rows.
if not rows:
    print("  ✗ the affected map has no rows — nothing to compare, so nothing passes")
    sys.exit(1)


def subjects_of(path):
    """The subjects a change to `path` triggers, following the map's own rule."""
    found = set()
    for row in rows:
        if fnmatch.fnmatchcase(path, row["glob"]):
            if row["subjects"] == {"ALL"}:
                return {"ALL"}
            found |= row["subjects"]
    return found


# ── the workspace ────────────────────────────────────────────────────────────
proc = subprocess.run(
    ["cargo", "metadata", "--no-deps", "--format-version", "1",
     "--manifest-path", os.path.join(root, "Cargo.toml")],
    capture_output=True, text=True,
)
if proc.returncode != 0:
    print("  ✗ cargo metadata failed — the workspace's edges cannot be read", file=sys.stderr)
    print("    " + (proc.stderr.strip().splitlines() or ["?"])[0], file=sys.stderr)
    sys.exit(1)

meta = json.loads(proc.stdout)
members = {}
for package in meta["packages"]:
    directory = os.path.dirname(os.path.abspath(package["manifest_path"]))
    name = os.path.relpath(directory, root)
    if not os.path.isdir(os.path.join(directory, "src")):
        continue
    members[package["name"]] = {
        "dir": name,
        "src": f"{name}/src/**",
        "dependencies": {d["name"] for d in package.get("dependencies", [])
                         if d.get("path")},
    }

# ── 1. every member is reachable ─────────────────────────────────────────────
for name, member in sorted(members.items()):
    if not subjects_of(member["src"]):
        findings.append(f"`{member['dir']}/src/**` is a workspace member ({name}) that no row "
                        f"matches — a change there would narrow to nothing")
if not findings:
    print(f"  ✓ all {len(members)} member(s)' sources match a row")

# ── 2. a dependent's stages are its dependency's too ────────────────────────
edges = 0
for name, member in sorted(members.items()):
    dependent = subjects_of(member["src"])
    if not dependent:
        continue
    for dependency in sorted(member["dependencies"]):
        upstream = members.get(dependency)
        if upstream is None:
            continue
        edges += 1
        provider = subjects_of(upstream["src"])
        if provider == {"ALL"} or dependent == {"ALL"}:
            continue
        missing = dependent - provider
        if missing:
            findings.append(
                f"`{member['dir']}` depends on `{dependency}`, but a change to "
                f"`{upstream['dir']}/src/**` does not reach "
                f"{', '.join(sorted(missing))} — those stages link {dependency}, so it can "
                f"break them and only the next edit to {member['dir']} would say so")
if not edges:
    findings.append("no intra-workspace dependency edge was found — a check with no edges "
                    "cannot be a check")
elif not any("does not reach" in f for f in findings):
    print(f"  ✓ all {edges} dependency edge(s) are reachable from their dependency's row")

# ── 3. a comment that names a stage lists it ────────────────────────────────
# Names that cannot be prose: a hyphenated job id, or `wasi`, which no sentence uses by
# accident. The words `test`, `lint`, `docs` and `coverage` are ordinary English here, and a
# check that fired on them would be deleted within a week.
prose_safe = {"test", "lint", "docs", "coverage", "audit", "shell", "gate", "portable",
              "workspace", "affected", "integration", "conformance", "ci-report"}
named = set()
for row in rows:
    for subject in row["subjects"]:
        named.add(subject)
checkable = {s for s in named if "-" in s or s == "wasi"}
for row in rows:
    for token in re.findall(r"[a-z][a-z-]*[a-z]", row["why"]):
        if token in checkable and token not in row["subjects"]:
            findings.append(f"`{map_path}`:{row['line']}: the comment names `{token}` but the "
                            f"row does not run it for `{row['glob']}` — a comment that names a "
                            f"stage is a promise the row does not keep")
            break

if findings:
    for finding in findings:
        print(f"  ✗ {finding}")
    sys.exit(1)
sys.exit(0)
PY
)

run_core() {
  local root="$1" map="$2"
  python3 -c "$CORE" "$root" "$map"
}

if [ "$SELF_TEST" -eq 0 ]; then
  echo "── the map against the workspace's edges ──"
  if run_core "$REPO_ROOT" "$MAP"; then
    echo "✅ affected graph: every member reachable, every dependency edge covered, comments honest"
  else
    echo "❌ affected graph: the map and the workspace disagree" >&2
    exit 1
  fi
  exit 0
fi

# ── self-test: each check must be able to fail ──────────────────────────────
scratch="$(mktemp -d)"
trap 'find "$scratch" -type f -delete; find "$scratch" -depth -type d -exec rmdir {} + 2>/dev/null || true' EXIT
[ -s "$MAP" ] || { echo "self-test: $MAP is missing or empty" >&2; exit 1; }

run_core "$REPO_ROOT" "$MAP" >/dev/null || { echo "self-test: the real map does not pass its own check" >&2; exit 1; }
echo "✓ self-test: the real map passes"

expect_failure() {
  local label="$1" needle="$2" seed="$3"
  cp "$MAP" "$scratch/case.tsv"
  python3 - "$scratch/case.tsv" "$needle" <<'PY'
import sys
path, needle = sys.argv[1], sys.argv[2]
lines = open(path, encoding="utf-8").read().splitlines()
out, dropped = [], False
for line in lines:
    if line.lstrip().startswith("#") or not line.strip():
        out.append(line)
        continue
    fields = line.split("\t")
    if not dropped and needle in fields[0]:
        dropped = True
        continue
    out.append(line)
open(path, "w", encoding="utf-8").write("\n".join(out) + "\n")
PY
  if run_core "$REPO_ROOT" "$scratch/case.tsv" >/dev/null 2>&1; then
    echo "self-test: $label still passed — that check has no subject" >&2
    exit 1
  fi
  echo "✓ self-test: $seed"
}

# S1: the member's own row is gone → the crate is unreachable.
expect_failure "a deleted member row" "wasi/src/**" \
  "a member with no row fails (a crate whose changes narrow to nothing)"

# S1b: `wasi` removed from the row that proves the wasm→wasi edge → the edge is unreported.
cp "$MAP" "$scratch/case.tsv"
python3 - "$scratch/case.tsv" <<'PY'
import sys
path = sys.argv[1]
out = []
for line in open(path, encoding="utf-8").read().splitlines():
    if line.startswith("wasm/src/**"):
        fields = line.split("\t")
        fields[1] = ",".join(s for s in fields[1].split(",") if s != "wasi")
        line = "\t".join(fields)
    out.append(line)
open(path, "w", encoding="utf-8").write("\n".join(out) + "\n")
PY
if run_core "$REPO_ROOT" "$scratch/case.tsv" >/dev/null 2>&1; then
  echo "self-test: dropping wasi from the wasm row passed — the dependency edge is unchecked" >&2
  exit 1
fi
echo "✓ self-test: dropping a dependent from its dependency's row fails, naming the edge"

# S2: a comment naming a stage the row does not run.
cp "$MAP" "$scratch/case.tsv"
python3 - "$scratch/case.tsv" <<'PY'
import sys
path = sys.argv[1]
out = []
for line in open(path, encoding="utf-8").read().splitlines():
    if line.startswith("engine/.pi/**"):
        fields = line.split("\t")
        fields[2] = fields[2] + "; wasm-gates also reads this"
        line = "\t".join(fields)
    out.append(line)
open(path, "w", encoding="utf-8").write("\n".join(out) + "\n")
PY
if run_core "$REPO_ROOT" "$scratch/case.tsv" >/dev/null 2>&1; then
  echo "self-test: a comment naming wasm-gates without listing it passed" >&2
  exit 1
fi
echo "✓ self-test: a comment naming a stage the row does not run fails"

# An empty map is a broken scan, not a tree with nothing to check.
printf '# only a comment\n' > "$scratch/empty.tsv"
if run_core "$REPO_ROOT" "$scratch/empty.tsv" >/dev/null 2>&1; then
  echo "self-test: an empty map passed — the check reports on a table it never read" >&2
  exit 1
fi
echo "✓ self-test: an empty map fails closed"

echo
echo "✅ affected-graph self-test: 5 seed(s), each one bites"
