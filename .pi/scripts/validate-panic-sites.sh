#!/usr/bin/env bash
# ============================================================================
# validate-panic-sites.sh — keep the production panic surface from growing
# ============================================================================
# Rust panics abort the process: in a runner/host there is no caller to catch
# them, so a single reachable panic takes down the whole execution instead of
# failing one step. This guard makes that surface explicit and reviewable.
#
# It runs .pi/scripts/panic_audit.py (which excludes #[cfg(test)] modules,
# test-attribute fns and test-only modules — the class of false positive that a
# naive `grep .unwrap()` scan produces) and compares the result against
# .pi/panic-baseline.tsv.
#
# Failure modes:
#   * a panic site NOT in the baseline  → new unfenced panic surface; either
#     remove it (return Result) or add it with a reason via --update
#   * a baseline entry with no site     → stale entry; run --update
#
# Usage:
#   .pi/scripts/validate-panic-sites.sh            # check
#   .pi/scripts/validate-panic-sites.sh --update   # regenerate the baseline
# ============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCANNER="$ROOT/.pi/scripts/panic_audit.py"
BASELINE="$ROOT/.pi/panic-baseline.tsv"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

if [ "${1:-}" = "--self-test" ]; then
  # AC5: the gate must catch a planted regression. Seed a fixture with an
  # unwrap and assert the scanner reports it.
  TMP="$(mktemp -d)"
  trap 'rm -rf "$TMP"' EXIT
  mkdir -p "$TMP/crate/src"
  printf 'pub fn f(x: Option<i32>) -> i32 {\n    x.unwrap()\n}\n' \
    > "$TMP/crate/src/regression.rs"
  found="$(python3 "$SCANNER" "$TMP" --json | python3 -c 'import json,sys; print(len(json.load(sys.stdin)))')"
  if [ "$found" -lt 1 ]; then
    echo -e "${RED}❌ self-test: scanner missed a planted unwrap${NC}" >&2
    exit 1
  fi
  echo -e "${GREEN}✅ self-test: scanner caught the planted unwrap${NC}"
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

# Classify a site so the baseline records *why* it is tolerated, not just where
# it is. Order matters: first match wins.
#
# The poisoned-lock class (server), the validating-constructor class
# (planning/plan_validation) and the chain invariant were removed by
# ISSUE-PRODUCTION-PANIC-SURFACE; a panic re-appearing in those files is a NEW
# decision and intentionally falls through to "unclassified — triage required".
RULES = [
    (r"^engine/benches/", "bench", "low",
     "criterion harness: a failed setup or execution aborts the benchmark run, "
     "which is the intended signal; benchmark code is never linked into production"),
    (r"^engine/examples/", "demo", "low",
     "runnable demo: its asserts are the evidence it prints, so panicking on a "
     "violated invariant is the intended behaviour (a reader sees the failure)"),
    (r"^mcp/src/engine_wiring\.rs$", "startup", "medium",
     "fail-fast at composition: without an engine the host cannot serve; logs then panics"),
    (r"^mcp/src/host/mod\.rs$", "startup", "medium",
     "app_state() before init_app_state() — programmer error; panics instead of a typed error"),
    (r"^engine/src/execution_engine/application/factory\.rs$", "startup", "medium",
     "fail-fast at composition: approval store must open"),
    (r"^engine/src/cancellation/", "invariant", "low",
     "tokio watch::Receiver::borrow() — panics only on a same-task conflicting borrow"),
    (r"^engine/src/approval/domain/(intent|hash)\.rs$", "const", "low",
     "infallible in practice (serde_json on an owned struct / HMAC accepts any key length)"),
    (r"^mcp/src/audit_tools/infrastructure/in_memory_audit_service\.rs$", "const", "low",
     "HMAC accepts any key length, so new_from_slice cannot fail here"),
    (r"^actions/src/action_output/", "const", "low",
     "compile-time regex, covered by validate_name tests"),
    (r"^mcp/src/[a-z_]+/interfaces/mcp/mod\.rs$", "const", "low",
     "serde_json::from_str on a compile-time schema const (test-enforced)"),
]


def classify(f):
    for pat, cls, sev, why in RULES:
        if re.search(pat, f["file"]):
            return cls, sev, why
    return "unknown", "unknown", "unclassified — triage required"


# Key a site by (file, code text, occurrence) — NOT by line number (#1089).
# A line inserted above a site used to read as "1 new site + 1 stale entry",
# which fails a green branch for a change that touched no panic at all. The
# unsafe baseline was re-keyed for exactly this reason (#1088); this one was not.
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
           "# Generated/maintained by .pi/scripts/validate-panic-sites.sh --update",
           "# `location` is file::code#occurrence, not file:line: a site is matched by",
           "# what it does, so unrelated edits above it do not churn the baseline.",
           "# Every entry is an explicit decision: a panic that can reach production is either",
           "# removed (return a Result) or recorded here with the reason it is tolerated."]
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
    print(f"✅ panic surface unchanged ({len(rows)} production sites, all baselined)")
    sys.exit(0)

if new:
    print(f"\n❌ {len(new)} NEW production panic site(s) — not in the baseline:")
    for loc in new:
        cls, sev, why = detail.get(loc, ("unknown", "unknown", "unclassified"))
        print(f"   [{sev}/{cls}] {loc}")
        print(f"      {why}")
    print("\n   Fix by returning a Result/typed error, or record the decision with:")
    print("     .pi/scripts/validate-panic-sites.sh --update")
if gone:
    print(f"\n⚠️  {len(gone)} baseline entr(ies) no longer present (stale — nice if you removed them):")
    for loc in gone:
        print(f"   {loc}")
    print("\n   Regenerate with: .pi/scripts/validate-panic-sites.sh --update")
sys.exit(1)
PY
