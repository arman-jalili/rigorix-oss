#!/usr/bin/env bash
# validate-repo-hygiene.sh — `.gitignore` says what it means, and nothing tracked is ignored.
#
#   bash .pi/scripts/validate-repo-hygiene.sh
#   bash .pi/scripts/validate-repo-hygiene.sh --report
#   bash .pi/scripts/validate-repo-hygiene.sh --self-test
#
# # Why this exists
#
# `.gitignore:96` read
#
#     profraw  # coverage artifacts (cargo-llvm-cov)
#
# `.gitignore` has **no inline comments**, so that line is a pattern matching a file literally named
# `profraw  # coverage artifacts (cargo-llvm-cov)`. It ignored nothing, and four coverage artifacts
# were committed through three pull requests before anyone noticed (#1520 follow-up). A pattern
# that silently matches nothing is the same defect class as a guard that silently reads nothing.
#
# Rules:
#
#   1. no `.gitignore` line carries a trailing ` # …` comment — it is not a comment, it is part of
#      the pattern;
#   2. no tracked file matches an ignore pattern (`git ls-files -i -c --exclude-standard` is empty)
#      — a file that is both tracked and ignored is one somebody had to force.
#
# Exit codes: 0 = clean; 1 = a finding; 2 = malformed input (no `.gitignore`, not a git tree).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
MODE="check"
while [ $# -gt 0 ]; do
  case "$1" in
    --report) MODE="report"; shift ;;
    --self-test) MODE="self-test"; shift ;;
    --root) [ $# -ge 2 ] || { echo "validate-repo-hygiene: --root needs a directory" >&2; exit 2; }; ROOT="$2"; shift 2 ;;
    -h|--help) sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "validate-repo-hygiene: unknown argument '$1'" >&2; exit 2 ;;
  esac
done

if [ "$MODE" = "self-test" ]; then
  scratch="$(mktemp -d)"
  # shellcheck disable=SC2064
  trap "find '$scratch' -type f -delete 2>/dev/null; find '$scratch' -depth -type d -exec rmdir {} + 2>/dev/null || true" EXIT
  git -C "$scratch" init -q 2>/dev/null
  ACCEPTED=0
  REFUSED=0
  MALFORMED=0
  case_() {  # case_ <label> <expect pass|fail|malformed>
    local label="$1" expect="$2" out status
    set +e
    out=$(bash "$0" --root "$scratch" 2>&1)
    status=$?
    set -e
    case "$expect" in
      pass) [ "$status" -eq 0 ] || { echo "✗ self-test: $label failed" >&2; printf '%s\n' "$out" >&2; return 1; }; ACCEPTED=$((ACCEPTED + 1)) ;;
      fail) [ "$status" -eq 1 ] || { echo "✗ self-test: $label did not fail (status $status)" >&2; printf '%s\n' "$out" >&2; return 1; }; REFUSED=$((REFUSED + 1)) ;;
      malformed) [ "$status" -eq 2 ] || { echo "✗ self-test: $label was not refused as malformed (status $status)" >&2; printf '%s\n' "$out" >&2; return 1; }; MALFORMED=$((MALFORMED + 1)) ;;
    esac
    echo "  ✓ $label"
  }

  echo "── self-test: a pattern that matches nothing is a finding ──"
  printf '# a comment\n*.profraw\n' > "$scratch/.gitignore"
  printf 'x\n' > "$scratch/tracked.txt"
  git -C "$scratch" add tracked.txt 2>/dev/null
  case_ "control: a real pattern and a tracked file pass" pass

  printf 'profraw  # coverage artifacts (cargo-llvm-cov)\n' > "$scratch/.gitignore"
  case_ "an inline comment in .gitignore fails" fail
  out=$(bash "$0" --root "$scratch" 2>&1 || true)
  case "$out" in
    *".gitignore:1"*) echo "  ✓ the finding names the line" ;;
    *) echo "✗ self-test: the finding did not name the line: $out" >&2; exit 1 ;;
  esac

  printf '*.log\n' > "$scratch/.gitignore"
  printf 'y\n' > "$scratch/ignored.log"
  git -C "$scratch" add -f ignored.log 2>/dev/null
  case_ "a file that is both tracked and ignored fails" fail

  rm -f "$scratch/.gitignore"
  case_ "no .gitignore at all is malformed, not a pass" malformed

  printf '✅ %s acceptance case(s) passed, %s finding(s) refused, %s malformed-input case(s) refused\n' \
    "$ACCEPTED" "$REFUSED" "$MALFORMED"
  exit 0
fi

exec python3 - "$ROOT" "$MODE" <<'PY'
import os
import re
import subprocess
import sys

ROOT, MODE = sys.argv[1], sys.argv[2]
GREEN, RED, DIM, RESET = "\033[0;32m", "\033[0;31m", "\033[2m", "\033[0m"
IGNORE = os.path.join(ROOT, ".gitignore")


def malformed(msg):
    print(f"{RED}❌ the tree cannot be read{RESET}", file=sys.stderr)
    print(f"   {msg}", file=sys.stderr)
    print(f"{DIM}   remedy: fix the input — a malformed tree and a finding have different "
          f"fixes{RESET}", file=sys.stderr)
    sys.exit(2)


try:
    lines = open(IGNORE, encoding="utf-8").read().split("\n")
except OSError as exc:
    malformed(f"{IGNORE}: {exc.strerror}")

findings = []
# rule 1: `.gitignore` has no inline comments
for lineno, line in enumerate(lines, 1):
    if not line.strip() or line.lstrip().startswith("#"):
        continue
    if re.search(r"\s#", line):
        findings.append(f"{os.path.relpath(IGNORE, ROOT)}:{lineno}: `{line.strip()}` — `.gitignore` "
                        f"has no inline comments, so everything after the space is part of the "
                        f"pattern and this line ignores nothing")

# rule 2: nothing tracked is ignored
try:
    out = subprocess.run(["git", "-C", ROOT, "ls-files", "-i", "-c", "--exclude-standard"],
                         capture_output=True, text=True, check=True).stdout
except (subprocess.CalledProcessError, FileNotFoundError) as exc:
    malformed(f"`git ls-files -i -c --exclude-standard` failed: {exc}")

tracked_but_ignored = [p for p in out.split("\n") if p]
for path in tracked_but_ignored:
    findings.append(f"{path}: tracked and ignored at the same time — an ignore pattern matches a "
                    f"file in the index, so somebody had to force it in")

if MODE == "report":
    print(f"{os.path.relpath(IGNORE, ROOT)}: {len(lines)} line(s), "
          f"{sum(1 for l in lines if l.strip() and not l.lstrip().startswith('#'))} pattern(s)")
    print(f"  tracked and ignored: {len(tracked_but_ignored)}")
    for f in findings:
        print(f"  {f}")
    sys.exit(1 if findings else 0)

if findings:
    print(f"{RED}❌ {len(findings)} repository-hygiene finding(s){RESET}", file=sys.stderr)
    for f in findings:
        print(f"   {f}", file=sys.stderr)
    print(f"{DIM}   a pattern that matches nothing ignores nothing — put the comment on its own "
          f"line, and keep artifacts out of the index{RESET}", file=sys.stderr)
    sys.exit(1)

print(f"{GREEN}✅ repo hygiene: every `.gitignore` line is a pattern, and no tracked file is "
      f"ignored{RESET}")
sys.exit(0)
PY