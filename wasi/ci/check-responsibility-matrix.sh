#!/usr/bin/env bash
# check-responsibility-matrix.sh — every claim on the responsibility page has an artifact (#1473).
#
#   bash wasi/ci/check-responsibility-matrix.sh
#   bash wasi/ci/check-responsibility-matrix.sh --self-test
#
# `docs/binding-responsibility.md` is quoted by GTM and by security questionnaires, so its rows
# are claims with owners. `validate-docs.sh` already resolves *links* and backticked identifiers;
# it cannot see a table row whose evidence column is empty, because an empty cell is not an
# identifier — that was measured, not assumed: a planted row with no evidence passed it.
#
# So three rules, on the table only:
#
#   1. every row has four non-empty cells (claim, owner, evidence, what it does not cover) —
#      an empty "does not cover" is the column that makes the page useful, and it goes first;
#   2. every evidence cell cites at least one thing in backticks — a row that cites nothing is
#      a sentence, not evidence;
#   3. every backticked evidence entry that looks like a path exists in the tree — a citation
#      that points at a file nobody has is worse than none, because it reads as proof.
#
# A missing page is a failure, not a skip: a checker that reports on a file it did not read
# passes everything.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
PAGE="$ROOT/docs/binding-responsibility.md"
MODE="check"

while [ $# -gt 0 ]; do
  case "$1" in
    --self-test) MODE="self-test"; shift ;;
    --page) [ $# -ge 2 ] || { echo "check-responsibility-matrix: --page needs a file" >&2; exit 2; }; PAGE="$2"; shift 2 ;;
    -h|--help) sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "check-responsibility-matrix: unknown argument '$1'" >&2; exit 2 ;;
  esac
done

check() {
  local page="$1" root="$2"
  python3 - "$page" "$root" <<'PY'
import os
import re
import sys

page, root = sys.argv[1], sys.argv[2]
if not os.path.isfile(page):
    print(f"  ✗ {page} does not exist — there is no page to check", file=sys.stderr)
    sys.exit(1)

rows = []
for number, line in enumerate(open(page, encoding="utf-8"), start=1):
    stripped = line.strip()
    if not stripped.startswith("|"):
        continue
    if re.fullmatch(r"\|[\s:|-]+\|", stripped):
        continue  # the header separator
    cells = [cell.strip() for cell in stripped.strip("|").split("|")]
    rows.append((number, cells))

# The first `|` row is the header; the rest are claims.
if len(rows) < 2:
    print(f"  ✗ {page} has no table rows — a page with no claims passes trivially", file=sys.stderr)
    sys.exit(1)
header, claims = rows[0], rows[1:]
if len(header[1]) != 4:
    print(f"  ✗ the table has {len(header[1])} column(s); the page claims four "
          f"(claim, who proves it, evidence, what it does not cover)", file=sys.stderr)
    sys.exit(1)

findings = []
for number, cells in claims:
    if len(cells) != 4:
        findings.append(f"line {number}: {len(cells)} cell(s), not the four the header declares")
        continue
    claim, owner, evidence, uncovered = cells
    for label, value in (("claim", claim), ("owner", owner), ("does not cover", uncovered)):
        if not value:
            findings.append(f"line {number}: the `{label}` cell is empty — {claim or '<no claim>'}")
    if not evidence:
        findings.append(f"line {number}: `{claim}` has no evidence — a claim with no artifact "
                        f"behind it is deleted, not softened")
        continue
    citations = re.findall(r"`([^`]+)`", evidence)
    if not citations:
        findings.append(f"line {number}: `{claim}` cites nothing in backticks")
        continue
    for citation in citations:
        if "/" not in citation:
            continue
        if not os.path.exists(os.path.join(root, citation)):
            findings.append(f"line {number}: `{claim}` cites `{citation}`, which is not in the tree — "
                            f"a citation that points at nothing reads as proof")

if findings:
    for finding in findings:
        print(f"  ✗ {finding}", file=sys.stderr)
    sys.exit(1)
print(f"  ✓ {len(claims)} claim(s): each has an owner, an artifact, and a stated limit")
sys.exit(0)
PY
}

if [ "$MODE" = "check" ]; then
  check "$PAGE" "$ROOT" || { echo "❌ responsibility matrix: a claim has no artifact behind it" >&2; exit 1; }
  exit 0
fi

# ── self-test: each rule must be able to fail ────────────────────────────────────
check "$PAGE" "$ROOT" >/dev/null || { echo "self-test: the real page does not pass" >&2; exit 1; }
echo "✓ self-test: the real page passes"

scratch="$(mktemp -d)"
trap 'find "$scratch" -type f -delete; find "$scratch" -depth -type d -exec rmdir {} + 2>/dev/null || true' EXIT

expect_failure() {
  local label="$1" line="$2" why="$3"
  { cat "$PAGE"; printf '%s\n' "$line"; } > "$scratch/page.md"
  if check "$scratch/page.md" "$ROOT" >/dev/null 2>&1; then
    echo "self-test: $label passed — $why" >&2
    exit 1
  fi
  echo "✓ self-test: $label fails ($why)"
}

# The issue's seed: a claim with no evidence.
expect_failure "a row with an empty evidence cell" \
  "| The gate cannot spawn a process. | us |  | nothing |" \
  "a claim with no artifact is deleted, not softened"

# A citation that does not exist: the defect that reads as proof.
expect_failure "a row citing a missing file" \
  "| The gate cannot spawn a process. | us | \`wasi/ci/no-such-gate.sh\` | nothing |" \
  "a citation that points at nothing reads as proof"

# An empty "does not cover": the column that makes the page useful in a review.
expect_failure "a row with no stated limit" \
  "| The gate cannot spawn a process. | us | \`wasi/ci/imports.txt\` | |" \
  "a claim with no stated limit is a marketing sentence"

# A page with no table at all.
printf '# nothing here\n' > "$scratch/empty.md"
if check "$scratch/empty.md" "$ROOT" >/dev/null 2>&1; then
  echo "self-test: a page with no table passed" >&2
  exit 1
fi
echo "✓ self-test: a page with no table fails closed"

echo
echo "✅ responsibility-matrix self-test: 5 seed(s), each one bites"
