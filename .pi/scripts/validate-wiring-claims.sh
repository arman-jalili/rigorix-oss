#!/usr/bin/env bash
# ============================================================================
# validate-wiring-claims.sh — keep "it is wired" claims resolved (#1100)
# ============================================================================
# The repeating failure this guards against is: code that exists, compiles,
# passes its own tests, and is never reached — while the module doc says it is
# wired. Grep finds the symbol; only reachability answers the question, and a
# one-off audit is true for one commit.
#
# The manifest `engine/.pi/architecture/wiring-claims.tsv` is the audit in
# machine-readable form (`engine/.pi/architecture/wiring-claims.md` is the same
# table for humans, with the reasoning):
#
#   * a CONFIRMED row fails when its evidence disappears — the wiring was
#     removed and the doc still claims it;
#   * a GAP row fails when (a) its document stops declaring the gap, i.e. the
#     overclaim came back, or (b) callers appear, i.e. the gap was closed and
#     the row should be promoted to CONFIRMED.
#
# Both directions matter here: this series watched the docs drift back into
# claiming wiring *after* it was known to be missing, twice (#1072, #1099).
#
# Usage:
#   .pi/scripts/validate-wiring-claims.sh                 # check (gating)
#   .pi/scripts/validate-wiring-claims.sh --list          # print the table
#   .pi/scripts/validate-wiring-claims.sh --self-test     # prove the check bites
#   .pi/scripts/validate-wiring-claims.sh --claims F --root D
#                                                         # check another manifest
#                                                         # (used by --self-test)
# ============================================================================
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CLAIMS="$ROOT/engine/.pi/architecture/wiring-claims.tsv"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

MODE="check"
while [ $# -gt 0 ]; do
    case "$1" in
        --self-test) MODE="self-test"; shift ;;
        --list) MODE="list"; shift ;;
        --claims) CLAIMS="$2"; shift 2 ;;
        --root) ROOT="$2"; shift 2 ;;
        -h|--help) sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

check_manifest() {
    local root="$1" claims="$2"
    python3 - "$root" "$claims" <<'PY'
import os
import sys

root, claims_path = sys.argv[1], sys.argv[2]

if not os.path.exists(claims_path):
    print(f"missing manifest: {claims_path}", file=sys.stderr)
    sys.exit(2)

failures = []
rows = 0

def read(path):
    full = os.path.join(root, path)
    if not os.path.exists(full):
        return None
    with open(full, encoding="utf-8", errors="replace") as handle:
        return handle.read()

def callers(needle, exclude):
    """Files outside `exclude` that mention `needle`."""
    hits = []
    exclude_abs = os.path.join(root, exclude)
    for base, dirs, files in os.walk(root):
        dirs[:] = [d for d in dirs if d not in (".git", "target", "node_modules")]
        if os.path.abspath(base).startswith(os.path.abspath(exclude_abs)):
            continue
        for name in files:
            if not name.endswith(".rs"):
                continue
            path = os.path.join(base, name)
            try:
                with open(path, encoding="utf-8", errors="replace") as handle:
                    body = handle.read()
            except OSError:
                continue
            if needle in body:
                hits.append(os.path.relpath(path, root))
    return hits

for lineno, line in enumerate(open(claims_path, encoding="utf-8"), 1):
    if line.startswith("#") or not line.strip():
        continue
    parts = line.rstrip("\n").split("\t")
    if len(parts) < 5:
        failures.append(f"{claims_path}:{lineno}: malformed row")
        continue
    row_id, doc, verdict, checks, claim = parts[:5]
    rows += 1

    for check in checks.split(";;"):
        kind, _, rest = check.partition(":")
        if kind == "contains":
            path, _, needle = rest.partition(":")
            body = read(path)
            if body is None:
                failures.append(
                    f"[{row_id}] {verdict} claim '{claim}': evidence file is gone: {path}"
                )
            elif needle not in body:
                failures.append(
                    f"[{row_id}] {verdict} claim '{claim}': {path} no longer contains {needle!r} "
                    f"— the wiring was removed, or the claim is no longer true"
                )
        elif kind == "no-callers":
            needle, _, exclude = rest.partition(":")
            hits = callers(needle, exclude)
            if hits:
                failures.append(
                    f"[{row_id}] {verdict} claim '{claim}': {needle} now has caller(s) outside "
                    f"{exclude}: {', '.join(hits[:3])} — if the gap is closed, promote the row "
                    f"to CONFIRMED instead of leaving a GAP row that no longer describes the code"
                )
        elif kind == "declares-gap":
            path, _, needle = rest.partition(":")
            body = read(path)
            if body is None:
                failures.append(f"[{row_id}] GAP claim '{claim}': document is gone: {path}")
            elif needle not in body:
                failures.append(
                    f"[{row_id}] GAP claim '{claim}': {path} no longer declares the gap "
                    f"({needle!r}) — the overclaim came back"
                )
        else:
            failures.append(f"[{row_id}] unknown check kind: {kind}")

if failures:
    print(f"❌ {len(failures)} wiring-claim failure(s) across {rows} row(s):")
    for failure in failures:
        print(f"   {failure}")
    sys.exit(1)

print(f"✅ wiring claims resolved ({rows} rows)")
PY
}

verdict_summary() { :; }

if [ "$MODE" = "self-test" ]; then
    # Plant a regression in a throwaway tree: a CONFIRMED row whose evidence is
    # missing, and a GAP row whose document stopped declaring the gap.
    TMP="$(mktemp -d)"
    trap 'find "$TMP" -mindepth 1 -delete 2>/dev/null; rmdir "$TMP" 2>/dev/null' EXIT
    mkdir -p "$TMP/src" "$TMP/doc"
    printf 'fn dispatch() {}\n' > "$TMP/src/site.rs"
    printf 'the module is wired\n' > "$TMP/doc/doc.md"
    cat > "$TMP/planted.tsv" <<'EOF'
P1	doc/doc.md	CONFIRMED	contains:src/site.rs:is_wired	the site is wired
P2	doc/doc.md	GAP	declares-gap:doc/doc.md:NOT WIRED	the feature runs
EOF
    if bash "$0" --claims "$TMP/planted.tsv" --root "$TMP" >/dev/null 2>&1; then
        echo -e "${RED}❌ self-test: planted regressions were NOT caught${NC}" >&2
        exit 1
    fi
    # And a healthy manifest must pass, so the failure above is the planted one.
    cat > "$TMP/healthy.tsv" <<'EOF'
H1	doc/doc.md	CONFIRMED	contains:src/site.rs:fn dispatch	the site is wired
H2	doc/doc.md	GAP	declares-gap:doc/doc.md:wired	the feature runs
EOF
    if ! bash "$0" --claims "$TMP/healthy.tsv" --root "$TMP" >/dev/null 2>&1; then
        echo -e "${RED}❌ self-test: a healthy manifest was rejected${NC}" >&2
        exit 1
    fi
    echo -e "${GREEN}✅ self-test: caught the planted regression and accepted the healthy manifest${NC}"
    exit 0
fi

if [ "$MODE" = "list" ]; then
    echo "wiring claims ($CLAIMS):"
    awk -F'\t' '!/^#/ && NF>=5 {printf "  %-4s %-10s %s\n", $1, $3, $5}' "$CLAIMS"
    exit 0
fi

check_manifest "$ROOT" "$CLAIMS"
