#!/usr/bin/env bash
# ============================================================================
# validate-guards.sh — make gating vs advisory explicit
# ============================================================================
# ISSUE-SELF-OWNED-VULN-CHECKS. A validator that cannot fail is documentation,
# not a control. This is the single place that answers "which checks can stop a
# merge?":
#
#   * every root .pi/scripts/validate-*.sh MUST be registered in .pi/guards.tsv
#     (so a new guard cannot be silently left unclassified);
#   * the gating guards marked `run_here=yes` are executed with REAL exit codes
#     — a non-zero exit fails the build. This script runs in the `ci.yml` `audit`
#     job, which the Final Gate requires;
#   * advisory checks are printed with `--list` and never gate.
#
# Usage:
#   .pi/scripts/validate-guards.sh              # integrity + run gating set
#   .pi/scripts/validate-guards.sh --list       # the gating/advisory table
#   .pi/scripts/validate-guards.sh --self-test  # prove each gating guard bites
# ============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REG="$ROOT/.pi/guards.tsv"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

MODE="${1:-check}"

if [ "$MODE" = "--help" ] || [ "$MODE" = "-h" ]; then
  sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
  exit 0
fi

# --- registry integrity (and optional listing) ------------------------------
python3 - "$REG" "$ROOT" "$MODE" <<'PY'
import os, re, sys

reg, root, mode = sys.argv[1:4]

if not os.path.exists(reg):
    print(f"missing registry: {reg}", file=sys.stderr)
    sys.exit(2)

rows = []
seen = set()
for lineno, line in enumerate(open(reg), 1):
    if line.startswith("#") or not line.strip():
        continue
    parts = line.rstrip("\n").split("\t")
    if len(parts) < 6:
        print(f"{reg}:{lineno}: malformed row (need 6 tab-separated fields)", file=sys.stderr)
        sys.exit(2)
    guard, cls, tier, run_here, owner, notes = parts[:6]
    if tier not in ("gating", "advisory"):
        print(f"{reg}:{lineno}: bad tier {tier!r} (want gating|advisory)", file=sys.stderr)
        sys.exit(2)
    if run_here not in ("yes", "no"):
        print(f"{reg}:{lineno}: bad run_here {run_here!r} (want yes|no)", file=sys.stderr)
        sys.exit(2)
    if not os.path.exists(os.path.join(root, guard)):
        print(f"{reg}:{lineno}: registered guard does not exist: {guard}", file=sys.stderr)
        sys.exit(2)
    rows.append((guard, cls, tier, run_here, owner, notes))
    seen.add(guard)

scripts_dir = os.path.join(root, ".pi/scripts")
on_disk = sorted(
    f".pi/scripts/{name}"
    for name in os.listdir(scripts_dir)
    if re.fullmatch(r"validate-.*\.sh", name)
)
unregistered = [p for p in on_disk if p not in seen]
if unregistered:
    print("unclassified guard(s) — add them to .pi/guards.tsv:", file=sys.stderr)
    for p in unregistered:
        print(f"    {p}", file=sys.stderr)
    sys.exit(2)

if mode in ("--list", "list"):
    print("GATING (a non-zero exit fails a mandatory build path):")
    for g, c, t, r, o, _ in rows:
        if t == "gating":
            where = "run here (ci audit job)" if r == "yes" else "gates via a dedicated CI job"
            print(f"  {c:<16} {g:<45} [{where}]  owner={o}")
    print("\nADVISORY (informational; does not gate):")
    for g, c, t, r, o, _ in rows:
        if t == "advisory":
            print(f"  {c:<16} {g:<45}  owner={o}")
    print(f"\n{len(on_disk)} validator script(s) on disk, all classified.")
PY

case "$MODE" in
  --list|list) exit 0 ;;
esac

# --- self-test: prove each gating guard fails on a planted regression --------
if [ "$MODE" = "--self-test" ] || [ "$MODE" = "self-test" ]; then
  echo "── gating guard self-tests ──"
  while IFS=$'\t' read -r guard class tier run_here owner notes; do
    [[ "$guard" =~ ^# ]] && continue
    [ -z "$guard" ] && continue
    if [ "$tier" = "gating" ] && [ "$run_here" = "yes" ]; then
      echo "▶ [$class] $guard --self-test"
      bash "$ROOT/$guard" --self-test
    fi
  done < "$REG"
  echo -e "${GREEN}✅ all gating guard self-tests passed${NC}"
  exit 0
fi

# --- run the gating set with real exit codes --------------------------------
echo "── gating guards (ci audit job) ──"
while IFS=$'\t' read -r guard class tier run_here owner notes; do
  [[ "$guard" =~ ^# ]] && continue
  [ -z "$guard" ] && continue
  if [ "$tier" = "gating" ] && [ "$run_here" = "yes" ]; then
    echo "▶ [$class] $guard"
    bash "$ROOT/$guard"
  fi
done < "$REG"

echo
echo -e "${GREEN}✅ all gating guards passed${NC}"
echo "   advisory checks are listed with: .pi/scripts/validate-guards.sh --list"
