#!/usr/bin/env bash
# ============================================================================
# check_runtime_reachability.sh — Rust (ADR-009)
#
# Every service struct under src/<module>/application/ must be referenced from
# production code outside its own module and outside tests (tests dirs,
# *_test.rs, and #[cfg(test)] units inside the same file do not count — we
# require a reference from a different production file outside the module).
#
# Usage: bash check_runtime_reachability.sh [--root <dir>]
# Exit:  0 all reachable ; 1 dead service(s)
# ============================================================================
set -uo pipefail
ROOT="$(pwd)"
if [ "${1:-}" = "--root" ] && [ -n "${2:-}" ]; then ROOT="$2"; fi
cd "$ROOT" || exit 1

VIOLATIONS=0
[ -d src ] || { echo "runtime-reachability (rust): no src/"; exit 0; }

while IFS= read -r svc; do
    [ -n "$svc" ] || continue
    mod="$(echo "$svc" | sed 's|^src/||; s|/application/.*||')"
    stem="$(basename "$svc" .rs)"
    # candidate type names: file stem in PascalCase and snake_case both
    pascal="$(echo "$stem" | python3 -c 'import sys; s=sys.stdin.read().strip(); print("".join(w.capitalize() for w in s.split("_")) if s else "")' 2>/dev/null || echo "$stem")"
    # production references: outside the module, excluding tests
    refs="$(grep -rlE "\b(${pascal}|${stem})\b" src 2>/dev/null | grep -vE "/tests?/|_test\.rs$|/test/" | grep -v "^src/${mod}/" | grep -v "^${svc}$" | grep -cv '^$' || true)"
    if [ "${refs:-0}" -eq 0 ]; then
        echo "$svc: [runtime-reachability] application service has no production caller outside its module (tests-only callers don't count — ADR-009)"
        VIOLATIONS=$((VIOLATIONS + 1))
    fi
done < <(find src -path "*/application/*.rs" -type f 2>/dev/null || true)

if [ "$VIOLATIONS" -gt 0 ]; then
    echo "runtime-wiring (rust): ${VIOLATIONS} unwired service(s)" >&2
    exit 1
fi
echo "runtime-reachability (rust): all application services reachable from production code"
exit 0
