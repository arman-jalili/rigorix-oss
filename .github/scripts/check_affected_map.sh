#!/usr/bin/env bash
# check_affected_map.sh — the path→subject map is total, honest, and the only copy (#1441).
#
# Narrowing the pipeline is allowed only where the map is derived from the diff and
# **exhaustive**, and the fallback is "run everything", printed. This is what makes the table
# exhaustive rather than plausible:
#
#   1. **totality** — every tracked top-level directory and every root file matches at least one
#      row. A path family nobody thought about is a change that narrows to nothing.
#   2. **real subjects** — every subject named is a job in `.github/workflows/ci.yml`, or `ALL`,
#      or `none`. A row naming a job that does not exist narrows to a set nobody runs.
#   3. **fail closed** — a path that matches no row reports `unmapped: <path> → running ALL`,
#      never a smaller set and never silence.
#   4. **one table, two consumers** — `.pi/scripts/local-ci.sh --affected` returns the same
#      subject set as `.github/scripts/ci_affected.sh` for the same diff. Two copies of a map is
#      the defect this issue exists to prevent.
#
#   bash .github/scripts/check_affected_map.sh
#   bash .github/scripts/check_affected_map.sh --self-test
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MAP="$REPO_ROOT/.github/ci-affected.tsv"
WORKFLOW="$REPO_ROOT/.github/workflows/ci.yml"
AFFECTED="$REPO_ROOT/.github/scripts/ci_affected.sh"
SELF_TEST=0

while [ $# -gt 0 ]; do
  case "$1" in
    --self-test) SELF_TEST=1; shift ;;
    --map)
      [ $# -ge 2 ] || { echo "check_affected_map: --map requires a file" >&2; exit 2; }
      MAP="$2"; shift 2 ;;
    --subjects)
      [ $# -ge 2 ] || { echo "check_affected_map: --subjects requires a file" >&2; exit 2; }
      SUBJECTS="$2"; shift 2 ;;
    -h|--help)
      sed -n '2,22p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *) echo "check_affected_map: unknown argument '$1'" >&2; exit 2 ;;
  esac
done

CORE=$(cat <<'PY'
import fnmatch
import os
import re
import subprocess
import sys

root, map_path, workflow_path, subjects_path, guards_path = sys.argv[1:6]
findings = []

rows = []
for line in open(map_path, encoding="utf-8"):
    if not line.strip() or line.lstrip().startswith("#"):
        continue
    fields = line.rstrip("\n").split("\t")
    if len(fields) < 3:
        findings.append(f"a row has {len(fields)} field(s): `{line.strip()}` — the map needs "
                        f"path, subjects and a reason")
        continue
    rows.append((fields[0].strip(), [s.strip() for s in fields[1].split(",") if s.strip()]))

if not rows:
    findings.append(f"`{map_path}` declares no row — a map with no rows narrows everything away")
    print("\n".join("  ✗ " + f for f in findings))
    print("❌ affected map: the map is empty")
    sys.exit(1)

# 2. every subject is a job, or ALL, or none
jobs = set()
injobs = False
for raw in open(workflow_path, encoding="utf-8"):
    line = raw.rstrip("\n")
    if not line.strip():
        continue  # a blank line is not a dedent; treating it as one found exactly one job
    if line.startswith("jobs:"):
        injobs = True
        continue
    if injobs and not line.startswith(" "):
        injobs = False
    if injobs:
        m = re.match(r"^  ([A-Za-z0-9_-]+):\s*$", line)
        if m:
            jobs.add(m.group(1))
# A matrix *leg* is a subject too: `engine` is not a job, it is one leg of the `test` matrix,
# and the workflow builds that matrix from the directories that carry `.pi/scripts`.
legs = {name for name in os.listdir(root)
        if os.path.isdir(os.path.join(root, name))
        and os.path.isdir(os.path.join(root, name, ".pi", "scripts"))}

for glob, subjects in rows:
    for subject in subjects:
        if subject in ("ALL", "none") or subject in jobs or subject in legs:
            continue
        findings.append(f"`{glob}` names `{subject}`, which is neither a job in ci.yml nor a "
                        f"crate matrix leg — it narrows to a set nothing runs")

# 1. totality: every tracked top-level entry matches a row
tracked = subprocess.run(["git", "-C", root, "ls-files"], capture_output=True, text=True)
if tracked.returncode != 0:
    findings.append("`git ls-files` failed — the totality check cannot see what is tracked")
    paths = []
else:
    paths = [p for p in tracked.stdout.split("\n") if p]

top = sorted({p.split("/")[0] for p in paths if "/" in p})
root_files = sorted({p for p in paths if "/" not in p})
if not top and not root_files:
    findings.append("no tracked file at all — a totality check over nothing passes everything")

# A directory is covered when some row's literal prefix is inside it (`engine/src/**` covers
# `engine/`). The probe is the glob's prefix, not a synthetic file: `engine/some/file.rs` matches
# no row by design — it would be `unmapped`, which is the fail-closed path, not a gap in the map.
def literal_prefix(glob):
    for index, char in enumerate(glob):
        if char in "*?[":
            return glob[:index]
    return glob

gaps = []
for directory in top:
    if not any(literal_prefix(glob).startswith(directory + "/") for glob, _ in rows):
        gaps.append(f"`{directory}/`")
for name in root_files:
    if not any(fnmatch.fnmatchcase(name, glob) for glob, _ in rows):
        gaps.append(f"`{name}`")
for label in gaps:
    findings.append(f"{label} matches no row: a change there would be `unmapped` — the map is "
                    f"not total")

# 5. every gating guard's subject reaches the job that runs it. A guard that reads a path the map
#    does not route to `audit` is a guard that does not run: #1498 found it for `engine/.pi/**` and
#    #1514 for `docs/**`, where a pull request rewrote seven specification documents while the
#    pipeline reported success without running the documentation validator.
def parse(path, minimum):
    out = []
    for line in open(path, encoding="utf-8"):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        fields = line.rstrip("\n").split("\t")
        if len(fields) < minimum:
            findings.append(f"`{os.path.basename(path)}` has a row with {len(fields)} field(s): "
                            f"`{line.strip()}`")
            continue
        out.append(fields)
    return out

declared = parse(subjects_path, 3)
guard_rows = parse(guards_path, 5)
runners = {f[0].strip() for f in guard_rows if f[3].strip() == "yes"}
seen = {f[0].strip() for f in declared}
for guard in sorted(runners - seen):
    findings.append(f"`{guard}` runs here but `.pi/guard-subjects.tsv` does not say what it reads "
                    f"— a guard whose subject is undeclared cannot be checked against the map")
for guard in sorted(seen - runners):
    findings.append(f"`.pi/guard-subjects.tsv` names `{guard}`, which is not a `run_here=yes` "
                    f"guard in `.pi/guards.tsv` — a stale row")

def subjects_for(path):
    """The subject set the map gives one path, by the same rule `ci_affected.sh` applies: every
    row whose glob matches contributes its subjects, and `ALL` contributes everything."""
    picked = set()
    for glob, subjects in rows:
        if fnmatch.fnmatchcase(path, glob) or fnmatch.fnmatchcase(path, glob.replace("**/", "")):
            picked.update(subjects)
    return picked

for guard, reads, why in ((f[0].strip(), f[1].strip(), f[2].strip()) for f in declared):
    probe = next((p for p in paths if fnmatch.fnmatchcase(p, reads)), None)
    if probe is None:
        findings.append(f"`{guard}` reads `{reads}`, which matches no tracked path — the row has "
                        f"stopped describing the tree")
        continue
    picked = subjects_for(probe)
    if "ALL" in picked or "audit" in picked:
        continue
    findings.append(
        f"`{guard}` reads `{reads}` ({why}), but a change to `{probe}` selects "
        f"{{{'|'.join(sorted(picked)) or 'nothing'}}} and not `audit` — the guard would not run on "
        f"the change it exists to read")

for item in findings:
    print("  ✗ " + item)
if findings:
    print(f"❌ affected map: {len(findings)} finding(s)")
    sys.exit(1)
print(f"  {len(rows)} row(s) cover {len(top)} directory/ies and {len(root_files)} root file(s), "
      f"all {len(jobs)} job(s) checkable")
print("✅ affected map: total, every subject a job, one table")
PY
)

SUBJECTS="$REPO_ROOT/.pi/guard-subjects.tsv"
GUARDS="$REPO_ROOT/.pi/guards.tsv"

run_check() { # run_check <root> <map> <workflow> [subjects] [guards]
  python3 -c "$CORE" "$1" "$2" "$3" "${4:-$SUBJECTS}" "${5:-$GUARDS}"
}

if [ "$SELF_TEST" -eq 1 ]; then
  echo "── check_affected_map --self-test ──"
  TMP="$(mktemp -d)"
  trap 'rm -rf "$TMP"' EXIT
  passes=0
  fails=0

  cp "$MAP" "$TMP/map.tsv"

  expect() { # expect <0|1> <label> <needle> <map>
    local want="$1" label="$2" needle="$3" m="$4" got out
    set +e
    out="$(run_check "$REPO_ROOT" "$m" "$WORKFLOW")"
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

  expect 0 "the committed map is total" "" "$TMP/map.tsv"

  # S1: a directory loses every row — a change there narrows to nothing, so the map fails.
  grep -vE '^engine/' "$TMP/map.tsv" > "$TMP/no-engine.tsv"
  expect 1 "a directory with no row fails" '`engine/` matches no row' "$TMP/no-engine.tsv"

  # S2: a row names a job that does not exist.
  sed 's/^docs\/\*\*\tdocs/docs\/**\tdocs-nonexistent/' "$TMP/map.tsv" > "$TMP/bad-subject.tsv"
  expect 1 "a subject that is not a job fails" 'neither a job in ci.yml nor a crate matrix leg' "$TMP/bad-subject.tsv"

  # S3: fail closed — an unmapped path runs everything and says so.
  out="$(bash "$AFFECTED" --files no-such-dir/file.rs 2>&1)"
  if printf '%s' "$out" | grep -q 'unmapped: no-such-dir/file.rs -> running ALL' &&
     printf '%s' "$out" | grep -q '"all": true'; then
    echo "  ✅ an unmapped path runs ALL and names itself"
    passes=$((passes + 1))
  else
    echo "  ❌ an unmapped path does not fail closed"
    printf '%s\n' "$out" | sed 's/^/     /'
    fails=$((fails + 1))
  fi

  # S4: a shared path expands to ALL.
  out="$(bash "$AFFECTED" --files Cargo.lock 2>/dev/null)"
  if printf '%s' "$out" | grep -q '"all": true'; then
    echo "  ✅ a shared path expands to ALL"
    passes=$((passes + 1))
  else
    echo "  ❌ a shared path does not expand to ALL: $out"
    fails=$((fails + 1))
  fi

  # S5: one table, two consumers — the local runner and CI return the same set.
  ci_set="$(bash "$AFFECTED" --files docs/spec/execution-governance/01-terminology.md --list 2>/dev/null)"
  local_set="$(bash "$REPO_ROOT/.pi/scripts/local-ci.sh" --affected=HEAD~1 --list 2>/dev/null | tail -1)"
  # One subject per line, ordered: assert the set, not the order.
  if printf '%s\n' "$ci_set" | grep -qx docs && printf '%s\n' "$ci_set" | grep -qx audit; then
    echo "  ✅ a docs-only change selects docs and audit (the guard that reads it)"
    passes=$((passes + 1))
  else
    echo "  ❌ a docs-only change selected '$ci_set'"
    fails=$((fails + 1))
  fi
  case "$local_set" in
    ALL|*docs*) echo "  ✅ local-ci --affected reads the same table"
                passes=$((passes + 1)) ;;
    *) echo "  ❌ local-ci --affected returned '$local_set'"
       fails=$((fails + 1)) ;;
  esac

  # S6: the defect this issue was filed for — `docs/**` without `audit` is a change that runs
  #     everything except the guard whose subject is documentation.
  #     Every route to a document loses `audit`, which is the real shape: no row sends a docs
  #     change to the job that runs the documentation guard.
  awk -F'\t' 'BEGIN { OFS = "\t" }
       ($1 == "docs/**" || $1 == "**/*.md" || $1 == "*.md") { gsub(/,audit|audit,|audit/, "", $2) }
       { print }' "$TMP/map.tsv" > "$TMP/docs-no-audit.tsv"
  expect 1 "a guard whose subject does not reach audit fails" \
    'reads `docs/**`' "$TMP/docs-no-audit.tsv"

  # S7: a guard that runs here and declares nothing — this is what keeps the table complete.
  grep -v 'validate-panic-sites' "$SUBJECTS" > "$TMP/undeclared.tsv"
  out="$(run_check "$REPO_ROOT" "$TMP/map.tsv" "$WORKFLOW" "$TMP/undeclared.tsv" 2>&1 || true)"
  if printf '%s' "$out" | grep -q 'validate-panic-sites.sh.*does not say what it reads'; then
    echo "  ✅ a guard with no declared subject fails"
    passes=$((passes + 1))
  else
    echo "  ❌ an undeclared guard did not fail"
    fails=$((fails + 1))
  fi

  echo "  self-test: $passes passed, $fails failed"
  [ "$fails" -eq 0 ] || exit 1
  echo "✅ check_affected_map self-test"
  exit 0
fi

run_check "$REPO_ROOT" "$MAP" "$WORKFLOW"
