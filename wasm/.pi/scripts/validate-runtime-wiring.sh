#!/usr/bin/env bash
# ============================================================================
# validate-runtime-wiring.sh — ADR-009 production-reachability dispatcher
#
# Deterministic check: every application-layer service must be referenced from
# production code OUTSIDE its own module and OUTSIDE tests. A service whose
# only callers are its own tests is implemented-but-unwired (dead code with a
# test suite) — the recurring rigorix pattern ADR-009 fixes.
#
# Delegates to languages/<lang>/check_runtime_reachability.sh.
# Usage: bash .pi/scripts/validate-runtime-wiring.sh [--root <dir>]
# Exit:  0 all services reachable (or language not yet covered)
#        1 unreachable service(s) — one 'file: [runtime-reachability] ...' line each
# ============================================================================
set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(pwd)"
if [ "${1:-}" = "--root" ] && [ -n "${2:-}" ]; then ROOT="$2"; fi
cd "$ROOT" || exit 1

detect_language() {
    if [ -f "Cargo.lock" ] || [ -f "Cargo.toml" ]; then echo "rust"
    elif [ -f "go.mod" ]; then echo "go"
    elif [ -f "pom.xml" ] || [ -f "build.gradle" ] || [ -f "build.gradle.kts" ] || [ -d "src/main/java" ]; then echo "java"
    elif [ -f "package.json" ]; then echo "typescript"
    elif [ -f "pyproject.toml" ] || [ -f "requirements.txt" ]; then echo "python"
    else echo "unknown"; fi
}
LANG="$(detect_language)"
SCRIPT="${SCRIPT_DIR}/languages/${LANG}/check_runtime_reachability.sh"
if [ -f "$SCRIPT" ]; then
    exec bash "$SCRIPT" --root "$ROOT"
fi
echo "runtime-wiring: no reachability check for '${LANG}' yet (ADR-009 planned: go/python/java)"
exit 0
