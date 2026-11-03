#!/usr/bin/env bash
# ============================================================================
# check_model_judgment.sh — ADR-008/009 dispatcher
# Runs the language-scoped model-judgment hard gates (dip-domain,
# interface-ownership, adapter-purity, transaction placement per language).
# Usage: bash .pi/scripts/check_model_judgment.sh [--root <dir>]
# ============================================================================
set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(pwd)"
if [ "${1:-}" = "--root" ] && [ -n "${2:-}" ]; then ROOT="$2"; fi
cd "$ROOT" || exit 1

detect_language() {
    if [ -f "Cargo.lock" ] || [ -f "Cargo.toml" ]; then echo "rust"
    elif [ -f "go.mod" ]; then echo "go"
    # java first when a maven/gradle build (or src/main/java) is present — a root
    # package.json (tooling) must not shadow a Java backend
    elif [ -f "pom.xml" ] || [ -f "build.gradle" ] || [ -f "build.gradle.kts" ] || [ -d "src/main/java" ]; then echo "java"
    elif [ -f "package.json" ]; then echo "typescript"
    elif [ -f "pyproject.toml" ] || [ -f "requirements.txt" ]; then echo "python"
    else echo "unknown"; fi
}
LANG="$(detect_language)"
SCRIPT="${SCRIPT_DIR}/languages/${LANG}/check_model_judgment.sh"
if [ -f "$SCRIPT" ]; then
    exec bash "$SCRIPT" --root "$ROOT"
fi
echo "model-judgment: no gate script for language '${LANG}'"
exit 0
