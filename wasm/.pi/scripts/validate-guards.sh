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
#   .pi/scripts/validate-guards.sh --coverage   # prove the LOCAL runner executes them
#   .pi/scripts/validate-guards.sh --coverage-self-test
#                                               # prove --coverage bites on fixtures
#
# `--coverage` (#1546) answers the question the registry cannot answer about
# itself: a gate that exists, is `gating`, is `run_here=yes`, and that no local
# run ever invokes is green locally and red in the pipeline. It reads two files
# and executes nothing.
# ============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REG="$ROOT/.pi/guards.tsv"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

MODE="${1:-check}"

if [ "$MODE" = "--help" ] || [ "$MODE" = "-h" ]; then
  sed -n '2,32p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
  exit 0
fi

# --- registry integrity (and optional listing) ------------------------------
python3 - "$REG" "$ROOT" "$MODE" "${2-}" "${3-}" <<'PY'
import os, re, sys

argv = sys.argv[1:]
reg, root, mode = argv[0], argv[1], argv[2]
extra = [a for a in argv[3:] if a]

GREEN = "\033[0;32m"; RED = "\033[0;31m"; YELLOW = "\033[1;33m"; NC = "\033[0m"


def read_rows(reg_path):
    """Parse the registry. Returns (rows, error) — an error is not an exception
    because the caller decides whether a bad registry is fatal."""
    rows = []
    for lineno, line in enumerate(open(reg_path), 1):
        if line.startswith("#") or not line.strip():
            continue
        parts = line.rstrip("\n").split("\t")
        if len(parts) < 6:
            return rows, f"{reg_path}:{lineno}: malformed row (need 6 tab-separated fields)"
        guard, cls, tier, run_here, owner, notes = parts[:6]
        if tier not in ("gating", "advisory"):
            return rows, f"{reg_path}:{lineno}: bad tier {tier!r} (want gating|advisory)"
        if run_here not in ("yes", "no"):
            return rows, f"{reg_path}:{lineno}: bad run_here {run_here!r} (want yes|no)"
        rows.append((guard, cls, tier, run_here, owner, notes))
    return rows, None


def coverage(reg_path, runner_path):
    """Does the LOCAL runner execute what gates?

    The subject is `tier == gating && run_here == yes`, because `run_here=yes`
    is the registry's statement that `validate-guards.sh` executes it — so
    naming that one script in the runner covers the row. A gating row with
    `run_here=no` gates through a dedicated job and is *reported* (a reader who
    sees 18 of 22 checked deserves the other four named) but is not this
    runner's subject.

    Paths are matched whole. `engine/.pi/scripts/validate-adr-references.sh`
    and a root script of the same basename are different subjects, and a
    basename match would let one stand in for the other.
    """
    rows, err = read_rows(reg_path)
    if err:
        print(err, file=sys.stderr)
        return 2
    if not os.path.exists(runner_path):
        print(f"missing local runner: {runner_path}", file=sys.stderr)
        return 2
    runner_src = open(runner_path).read()

    subject = [(g, c, r) for g, c, t, r, _o, _n in rows if t == "gating" and r == "yes"]
    elsewhere = [(g, c) for g, c, t, r, _o, _n in rows if t == "gating" and r == "no"]
    if not subject:
        print("the registry selects no gating row with run_here=yes — wrong registry?", file=sys.stderr)
        return 2

    print("\u2500\u2500 gating coverage (can a local run see what gates?) \u2500\u2500")
    uncovered = []
    for g, c, r in subject:
        if g in runner_src:
            why = "named directly in local-ci.sh"
        elif ".pi/scripts/validate-guards.sh" in runner_src:
            why = "run by .pi/scripts/validate-guards.sh, which local-ci.sh names"
        else:
            why = None
        if why is None:
            uncovered.append((g, c, r))
            print(f"  \u274c [{c}] {g} \u2014 no local run executes it")
        else:
            print(f"  \u2705 [{c}] {g} \u2014 {why}")

    for g, c in elsewhere:
        print(f"  \u2139  [{c}] {g} \u2014 gates via a dedicated CI job (run_here=no), not this runner's subject")

    if uncovered:
        print(f"\n{len(uncovered)} of {len(subject)} gating guard(s) are absent from .pi/scripts/local-ci.sh:", file=sys.stderr)
        for g, c, r in uncovered:
            print(f"    [{c}] {g} (run_here={r})", file=sys.stderr)
        print("  Add it to a local-ci.sh stage: a gate the local run never invokes is green where", file=sys.stderr)
        print("  the author looks and red where it lands.", file=sys.stderr)
        return 1
    print(f"\n{len(subject)} gating row(s) checked, {len(subject)} covered, 0 uncovered.")
    return 0


if mode == "--coverage-check":
    # Fixture mode: the caller supplies the pair, so the self-test can build a
    # registry and a runner that are wrong in one specific way without touching
    # the real ones. Integrity checks do not apply to a fixture.
    if len(extra) < 2:
        print("--coverage-check needs <registry> <runner>", file=sys.stderr)
        sys.exit(2)
    sys.exit(coverage(extra[0], extra[1]))


rows, err = read_rows(reg)
if err:
    print(err, file=sys.stderr)
    sys.exit(2)
seen = set()
for guard, _cls, _tier, _run_here, _owner, _notes in rows:
    if not os.path.exists(os.path.join(root, guard)):
        print(f"{reg}: registered guard does not exist: {guard}", file=sys.stderr)
        sys.exit(2)
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

if mode in ("--coverage", "coverage"):
    sys.exit(coverage(reg, os.path.join(root, ".pi/scripts/local-ci.sh")))

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
  --list|list|--coverage|coverage|--coverage-check) exit 0 ;;
esac

# --- coverage self-test: prove the check bites on a fixture pair (#1546) ------
# A check that reads two files and executes nothing cannot be proven by running
# it against the real pair: at the tip it passes, and that says nothing about
# what it does when the pair disagrees. The fixtures are built in a temporary
# directory, so no real file is mutated and a change to the real registry cannot
# silently disable the seed — the seed supplies its own registry.
if [ "$MODE" = "--coverage-self-test" ]; then
  echo "── coverage self-test: a gate the runner never names is named back ──"
  tmp="$(mktemp -d)"
  trap 'find "${tmp:-/nonexistent}" -mindepth 0 -delete 2>/dev/null || true' EXIT
  failures=0

  write_registry() {  # $1 path; $2.. "guard<TAB>class<TAB>tier<TAB>run_here"
    local path="$1"; shift
    { echo "# guard	class	tier	run_here	owner	notes"
      local row
      # rows arrive as guard<TAB>class<TAB>tier<TAB>run_here; owner and notes are fixed
      for row in "$@"; do printf '%s	security	fixture\n' "$row"; done
    } > "$path"
  }

  expect_seed() {  # $1 label, $2 wanted exit, $3 registry, $4 runner, $5 fragment
    local label="$1" want="$2" reg="$3" runner="$4" fragment="${5:-}"
    local out code
    out="$(bash "$ROOT/.pi/scripts/validate-guards.sh" --coverage-check "$reg" "$runner" 2>&1)" && code=0 || code=$?
    if [ "$code" -ne "$want" ]; then
      echo -e "  ${RED}❌${NC} $label (exit $code, wanted $want)"
      echo "$out" | tail -5 | sed 's/^/      /'
      failures=$((failures + 1)); return
    fi
    if [ -n "$fragment" ] && ! printf '%s' "$out" | grep -qF "$fragment"; then
      echo -e "  ${RED}❌${NC} $label — ran, but the failure does not name '$fragment'"
      failures=$((failures + 1)); return
    fi
    echo -e "  ${GREEN}✅${NC} $label"
  }

  # Seed A: a gating guard, and a fixture runner naming no runner-side
  # invocation — the exact shape of #1546.
  write_registry "$tmp/reg_a.tsv" ".pi/scripts/validate-fixture-gate.sh	fixture	gating	yes"
  printf '#!/usr/bin/env bash\ncargo test\n' > "$tmp/runner_a.sh"
  expect_seed "a gating guard no fixture runner executes is named" 1 \
      "$tmp/reg_a.tsv" "$tmp/runner_a.sh" ".pi/scripts/validate-fixture-gate.sh"

  # Control: the same registry, a runner that names validate-guards.sh — which
  # is the registry's own statement that this script executes the row.
  printf '#!/usr/bin/env bash\nbash .pi/scripts/validate-guards.sh\n' > "$tmp/runner_b.sh"
  expect_seed "the same registry is covered once the runner names the gating set" 0 \
      "$tmp/reg_a.tsv" "$tmp/runner_b.sh"

  # Seed B: a row that gates through a dedicated CI job is reported, not failed —
  # broadening the subject would make the check unable to pass at all.
  write_registry "$tmp/reg_c.tsv" ".pi/scripts/validate-fixture-gate.sh	fixture	gating	yes" \
      ".pi/scripts/validate-fixture-dedicated.sh	fixture	gating	no"
  expect_seed "a row that gates elsewhere is reported, not failed" 0 \
      "$tmp/reg_c.tsv" "$tmp/runner_b.sh"

  # Seed C: an advisory row cannot make the check fail either.
  write_registry "$tmp/reg_d.tsv" ".pi/scripts/validate-fixture-gate.sh	fixture	gating	yes" \
      ".pi/scripts/validate-fixture-advisory.sh	fixture	advisory	yes"
  expect_seed "an advisory row is not a gate" 0 "$tmp/reg_d.tsv" "$tmp/runner_b.sh"

  if [ "$failures" -gt 0 ]; then
    echo -e "${RED}coverage self-test: $failures case(s) failed${NC}"
    exit 1
  fi
  echo -e "${GREEN}✅ coverage self-test: 4/4 (the check names what the runner omits)${NC}"
  exit 0
fi

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
  bash "$ROOT/.pi/scripts/validate-guards.sh" --coverage-self-test
  echo -e "${GREEN}✅ all gating guard self-tests passed${NC}"
  exit 0
fi

# --- run the gating set with real exit codes --------------------------------
# Every guard runs, and each verdict is printed with the guard's name. Aborting
# on the first failure (`set -e` alone) makes the local runner's output name a
# script path buried in the middle of a log; a stage that says "no local run
# executes this" has to be able to say *which* gate answered red (#1546).
echo "── gating guards (ci audit job) ──"
FAILED_GUARDS=()
while IFS=$'\t' read -r guard class tier run_here owner notes; do
  [[ "$guard" =~ ^# ]] && continue
  [ -z "$guard" ] && continue
  if [ "$tier" = "gating" ] && [ "$run_here" = "yes" ]; then
    echo "▶ [$class] $guard"
    if bash "$ROOT/$guard"; then
      echo -e "  ${GREEN}✅${NC} [$class] $guard"
    else
      echo -e "  ${RED}❌${NC} [$class] $guard"
      FAILED_GUARDS+=("[$class] $guard")
    fi
  fi
done < "$REG"

if [ "${#FAILED_GUARDS[@]}" -gt 0 ]; then
  echo
  echo -e "${RED}gating guard(s) failed: ${#FAILED_GUARDS[@]}${NC}"
  for g in "${FAILED_GUARDS[@]}"; do
    echo -e "  ${RED}❌${NC} $g"
  done
  exit 1
fi

echo
echo -e "${GREEN}✅ all gating guards passed${NC}"
echo "   advisory checks are listed with: .pi/scripts/validate-guards.sh --list"
