#!/usr/bin/env bash
# ============================================================================
# validate-script-drift.sh — one source of truth for the mirrored .pi/scripts
# ============================================================================
# ISSUE-VALIDATOR-SINGLE-SOURCE (#1004). `.pi/scripts` is mirrored into every
# crate tree, and the same command name had drifted into a different program
# depending on which tree invoked it. This guard fails when a file mirrored
# across trees differs AND is not a recorded, reviewed difference.
#
# Scope: every file under <tree>/.pi/scripts that exists in >= 2 trees. Files
# that live in a single tree (the root-only guards; engine's code-graph
# contracts) are not mirrors and are skipped. Recorded differences live in
# .pi/script-drift-allow.tsv — each with a reason — so the guard can tell drift
# from design. A difference that is neither reconciled nor recorded fails.
#
# Usage:
#   .pi/scripts/validate-script-drift.sh              # check
#   .pi/scripts/validate-script-drift.sh --list       # recorded differences
#   .pi/scripts/validate-script-drift.sh --self-test  # prove the guard bites
# ============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ALLOW="$ROOT/.pi/script-drift-allow.tsv"
TREES=(. engine mcp cli actions)

run_check() { # $1 base dir, $2 allow file, $3 mode (check|list)
    python3 - "$1" "$2" "$3" "${TREES[@]}" <<'PY'
import fnmatch, hashlib, os, sys

base, allow_path, mode = sys.argv[1:4]
trees = sys.argv[4:]
rel_root = ".pi/scripts"

records = []
if os.path.exists(allow_path):
    for lineno, line in enumerate(open(allow_path, encoding="utf-8"), 1):
        line = line.rstrip("\n")
        if not line.strip() or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) < 3:
            print(f"{allow_path}:{lineno}: malformed row "
                  f"(need pattern<TAB>trees<TAB>reason)", file=sys.stderr)
            sys.exit(2)
        records.append((parts[0], parts[1], "\t".join(parts[2:])))

def recorded(rel):
    for pat, _trees, reason in records:
        if fnmatch.fnmatch(rel, pat):
            return reason
    return None

contents = {}
for t in trees:
    d = os.path.join(base, t, rel_root)
    if not os.path.isdir(d):
        continue
    for dirpath, _dirnames, files in os.walk(d):
        for f in files:
            p = os.path.join(dirpath, f)
            rel = os.path.relpath(p, d)
            h = hashlib.sha256(open(p, "rb").read()).hexdigest()
            contents.setdefault(rel, {})[t] = h

mirrored = {r: d for r, d in contents.items() if len(d) >= 2}
drifted = {r: d for r, d in mirrored.items() if len(set(d.values())) > 1}
allowed = {r: d for r, d in drifted.items() if recorded(r) is not None}
bad = {r: d for r, d in drifted.items() if recorded(r) is None}

if mode == "list":
    print(f"mirrored files checked: {len(mirrored)} "
          f"(byte-identical across the trees that carry them)")
    print(f"recorded differences (allowed): {len(allowed)}")
    for r in sorted(allowed):
        print(f"  {r}")
        print(f"      trees: {','.join(sorted(allowed[r]))}")
        print(f"      why:   {recorded(r)}")
    print(f"unrecorded differences: {len(bad)}")
    sys.exit(0)

print(f"script drift: {len(mirrored)} mirrored files, "
      f"{len(allowed)} recorded difference(s), {len(bad)} unrecorded")

if bad:
    print("", file=sys.stderr)
    print("Unintentional drift — mirrored files differ and are not recorded in "
          ".pi/script-drift-allow.tsv:", file=sys.stderr)
    for r in sorted(bad):
        print(f"  {r}", file=sys.stderr)
        for t in sorted(bad[r]):
            print(f"      {t:<8} {bad[r][t][:16]}", file=sys.stderr)
        print("      -> reconcile to one implementation, or record the "
              "difference with a reason", file=sys.stderr)
    sys.exit(1)

print("OK — every unrecorded mirrored file is byte-identical across trees.")
PY
}

case "${1:-check}" in
--list | list)
    run_check "$ROOT" "$ALLOW" list
    ;;
--self-test | self-test)
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    mkdir -p "$tmp/repo/.pi/scripts/languages/rust" "$tmp/repo/engine/.pi/scripts/languages/rust"
    : >"$tmp/allow.tsv"
    printf 'same\n' >"$tmp/repo/.pi/scripts/languages/rust/validate-tests.sh"
    printf 'same\n' >"$tmp/repo/engine/.pi/scripts/languages/rust/validate-tests.sh"

    # 1. byte-identical copies must pass
    if ! run_check "$tmp/repo" "$tmp/allow.tsv" check >/dev/null 2>&1; then
        echo "self-test FAILED: guard flagged byte-identical copies"
        exit 1
    fi

    # 2. planted drift must fail
    printf 'drifted\n' >"$tmp/repo/engine/.pi/scripts/languages/rust/validate-tests.sh"
    if run_check "$tmp/repo" "$tmp/allow.tsv" check >/dev/null 2>&1; then
        echo "self-test FAILED: guard did not bite on drift"
        exit 1
    fi

    # 3. a recorded difference must be honoured
    printf 'languages/rust/*\t.\trecorded in self-test\n' >"$tmp/allow.tsv"
    if ! run_check "$tmp/repo" "$tmp/allow.tsv" check >/dev/null 2>&1; then
        echo "self-test FAILED: recorded difference was not honoured"
        exit 1
    fi

    # 4. a single-tree file is not a mirror and must be ignored
    printf 'solo\n' >"$tmp/repo/engine/.pi/scripts/solo.sh"
    if ! run_check "$tmp/repo" "$tmp/allow.tsv" check >/dev/null 2>&1; then
        echo "self-test FAILED: single-tree file was treated as drift"
        exit 1
    fi

    echo "self-test passed: bites on drift, honours records, ignores single-tree files"
    ;;
*)
    run_check "$ROOT" "$ALLOW" check
    ;;
esac
