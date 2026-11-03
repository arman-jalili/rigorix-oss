#!/usr/bin/env bash
# ============================================================================
# validate-secret-logs.sh — keep secret material out of `tracing` output
# ============================================================================
# The 1.9.4 evidence-redaction work (and ADR-017) show how easy it is to funnel
# a token/key into a log line. This scans PRODUCTION `tracing` macro invocations
# for a secret-ish identifier that is not wrapped in a redaction helper.
#
# Failure modes:
#   * a flagged site NOT in the baseline → new disclosure; redact it or record
#     the decision with --update
#   * a baseline entry with no site      → stale entry; run --update
#
# Usage:
#   .pi/scripts/validate-secret-logs.sh            # check (CI, gating)
#   .pi/scripts/validate-secret-logs.sh --update   # regenerate the baseline
#   .pi/scripts/validate-secret-logs.sh --self-test # prove the detector bites
# ============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCANNER="$ROOT/.pi/scripts/secret_log_audit.py"
BASELINE="$ROOT/.pi/secret-log-baseline.tsv"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

if [ "${1:-}" = "--self-test" ]; then
  # AC5: the gate must catch a planted regression. Seed a fixture that logs a
  # secret-ish argument and assert the scanner reports it.
  TMP="$(mktemp -d)"
  trap 'rm -rf "$TMP"' EXIT
  mkdir -p "$TMP/crate/src"
  printf 'pub fn f(api_key: &str) {\n    tracing::info!(api_key = %api_key, "call");\n}\n' \
    > "$TMP/crate/src/regression.rs"
  found="$(python3 "$SCANNER" "$TMP" --json | python3 -c 'import json,sys; print(len(json.load(sys.stdin)))')"
  if [ "$found" -lt 1 ]; then
    echo -e "${RED}❌ self-test: scanner missed a planted secret-in-log${NC}" >&2
    exit 1
  fi
  echo -e "${GREEN}✅ self-test: scanner caught the planted secret-in-log${NC}"
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

# No site is tolerated today: the one real finding (a truncated secret prefix in
# actions/src/security_config) was removed, not baselined. A future site must be
# redacted, or added here via --update with a written reason.
RULES = []


def classify(f):
    for pat, cls, sev, why in RULES:
        if re.search(pat, f["file"]):
            return cls, sev, why
    return "unknown", "unknown", "unclassified — triage required"


rows = []
for f in findings:
    cls, sev, why = classify(f)
    rows.append((f"{f['file']}:{f['line']}", f["kind"], cls, sev, why))
rows.sort()


def fmt(rows):
    out = ["# location\tkind\tclass\tseverity\treason",
           "# Generated/maintained by .pi/scripts/validate-secret-logs.sh --update",
           "# A flagged tracing call is either redacted or recorded here with the",
           "# reason the logged identifier is not secret material."]
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
    print(f"✅ secret-in-log surface unchanged ({len(rows)} production sites, all baselined)")
    sys.exit(0)

if new:
    print(f"\n❌ {len(new)} NEW tracing call(s) that may log secret material:")
    for loc in new:
        cls, sev, why = detail.get(loc, ("unknown", "unknown", "unclassified"))
        print(f"   [{sev}/{cls}] {loc}")
        print(f"      {why}")
    print("\n   Redact it, or record the decision with:")
    print("     .pi/scripts/validate-secret-logs.sh --update")
if gone:
    print(f"\n⚠️  {len(gone)} baseline entr(ies) no longer present (stale):")
    for loc in gone:
        print(f"   {loc}")
    print("\n   Regenerate with: .pi/scripts/validate-secret-logs.sh --update")
sys.exit(1)
PY
