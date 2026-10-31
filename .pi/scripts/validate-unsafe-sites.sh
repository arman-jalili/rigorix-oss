#!/usr/bin/env bash
# ============================================================================
# validate-unsafe-sites.sh — keep the production `unsafe` surface from growing
# ============================================================================
# `unsafe` is a promise the compiler cannot check. Like the panic surface, it
# must be a reviewable decision: either remove it, or record it here with a
# reason. The scanner excludes `#[cfg(test)]` items, test-attribute fns and
# test-only modules, so the ubiquitous test-only `unsafe { set_var }` is not
# counted.
#
# Failure modes:
#   * an `unsafe` site NOT in the baseline → new unfenced surface; remove it or
#     record the decision with --update
#   * a baseline entry with no site      → stale entry; run --update
#
# Usage:
#   .pi/scripts/validate-unsafe-sites.sh            # check (CI, gating)
#   .pi/scripts/validate-unsafe-sites.sh --update   # regenerate the baseline
#   .pi/scripts/validate-unsafe-sites.sh --self-test # prove the detector bites
# ============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCANNER="$ROOT/.pi/scripts/unsafe_audit.py"
BASELINE="$ROOT/.pi/unsafe-baseline.tsv"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

if [ "${1:-}" = "--self-test" ]; then
  # AC5: the gate must catch a planted regression. Seed a fixture with an
  # `unsafe` block and assert the scanner reports it.
  TMP="$(mktemp -d)"
  trap 'rm -rf "$TMP"' EXIT
  mkdir -p "$TMP/crate/src"
  printf 'pub fn f() -> i32 {\n    unsafe { std::ptr::read(0x1 as *const i32) }\n}\n' \
    > "$TMP/crate/src/regression.rs"
  found="$(python3 "$SCANNER" "$TMP" --json | python3 -c 'import json,sys; print(len(json.load(sys.stdin)))')"
  if [ "$found" -lt 1 ]; then
    echo -e "${RED}❌ self-test: scanner missed a planted unsafe block${NC}" >&2
    exit 1
  fi
  echo -e "${GREEN}✅ self-test: scanner caught the planted unsafe block${NC}"
  exit 0
fi

if [ ! -f "$SCANNER" ]; then
  echo -e "${RED}❌ missing scanner: $SCANNER${NC}" >&2
  exit 1
fi

TMP="$(mktemp -d)"
trap 'find "$TMP" -mindepth 1 -delete 2>/dev/null || true; rmdir "$TMP" 2>/dev/null || true' EXIT

python3 "$SCANNER" "$ROOT" --json > "$TMP/current.json"

python3 - "$TMP/current.json" "$BASELINE" "${1:-}" "$ROOT" <<'PY'
import json, sys, os, re

current_path, baseline_path, mode, root = sys.argv[1:5]
findings = json.load(open(current_path))

RULES = [
    (r"^engine/src/precondition/infrastructure/runner\.rs$", "libc", "medium",
     "libc::geteuid/getgroups to read the real uid/groups for the precondition "
     "runner; the existing SAFETY note covers the only pointer (a buffer sized "
     "by the first getgroups call) and every call is fallible-by-return, not UB"),
    (r"^engine/src/approval/infrastructure/repository/approval_repository_impl\.rs$",
     "libc", "low",
     "libc::flock on a file descriptor owned by the enclosing File; the call takes "
     "no pointer, its return is checked, and EINTR is retried. Used for shared "
     "read and exclusive write locks on the approval store and ledger (#1254, #1261)"),
]


def classify(f):
    for pat, cls, sev, why in RULES:
        if re.search(pat, f["file"]):
            return cls, sev, why
    return "unknown", "unknown", "unclassified — triage required"


# Key a site by (file, code text, occurrence) — NOT by line number. A line
# inserted above a site used to read as "1 new site + 1 stale entry", which fails
# a green branch for a change that touched no `unsafe` at all (it did, on the
# #1082 branch, when a lint header was added to the top of runner.rs). A guard
# that cries wolf on an unrelated edit trains people to run --update without
# reading, which is how a baseline stops meaning anything.
rows = []
seen = {}
for f in findings:
    cls, sev, why = classify(f)
    base = f"{f['file']}::{f['code'].strip()}"
    seen[base] = seen.get(base, 0) + 1
    rows.append((f"{base}#{seen[base]}", f["kind"], cls, sev, why))
rows.sort()


def fmt(rows):
    out = ["# location\tkind\tclass\tseverity\treason",
           "# Generated/maintained by .pi/scripts/validate-unsafe-sites.sh --update",
           "# `location` is file::code#occurrence, not file:line: a site is matched by",
           "# what it does, so unrelated edits above it do not churn the baseline.",
           "# Every entry is an explicit decision: production `unsafe` is either",
           "# removed or recorded here with the reason it is tolerated."]
    for loc, kind, cls, sev, why in rows:
        out.append(f"{loc}\t{kind}\t{cls}\t{sev}\t{why}")
    return "\n".join(out) + "\n"


if mode == "--update":
    open(baseline_path, "w").write(fmt(rows))
    print(f"baseline written: {baseline_path} ({len(rows)} sites)")
    sys.exit(0)

if not os.path.exists(baseline_path):
    print("no baseline yet — run --update to create it")
    sys.exit(1)

want = {}
for line in open(baseline_path):
    if line.startswith("#") or not line.strip():
        continue
    parts = line.rstrip("\n").split("\t")
    want[parts[0]] = parts[2] if len(parts) > 2 else "unknown"

have = {r[0]: r[2] for r in rows}
detail = {r[0]: (r[2], r[3], r[4]) for r in rows}

new = sorted(set(have) - set(want))
gone = sorted(set(want) - set(have))

if not new and not gone:
    print(f"✅ unsafe surface unchanged ({len(rows)} production sites, all baselined)")
    sys.exit(0)

if new:
    print(f"\n❌ {len(new)} NEW production `unsafe` site(s) — not in the baseline:")
    for loc in new:
        cls, sev, why = detail.get(loc, ("unknown", "unknown", "unclassified"))
        print(f"   [{sev}/{cls}] {loc}")
        print(f"      {why}")
    print("\n   Remove it, or record the decision with:")
    print("     .pi/scripts/validate-unsafe-sites.sh --update")
if gone:
    print(f"\n⚠️  {len(gone)} baseline entr(ies) no longer present (stale):")
    for loc in gone:
        print(f"   {loc}")
    print("\n   Regenerate with: .pi/scripts/validate-unsafe-sites.sh --update")
sys.exit(1)
PY
