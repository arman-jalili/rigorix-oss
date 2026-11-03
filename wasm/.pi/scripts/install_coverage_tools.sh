#!/usr/bin/env bash
# ============================================================================
# install_coverage_tools.sh — Install the project's real coverage tooling
# (dispatcher)
#
# Delegates to languages/<lang>/install_coverage_tools.sh based on the
# detected language. Run this before `.pi/scripts/coverage.sh --gate` in CI
# or on a fresh checkout.
#
# Usage:
#   bash .pi/scripts/install_coverage_tools.sh
#
# Exit codes:
#   0 — tooling available / installed / not required
#   1 — install failed
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

detect_language() {
    if [ -f "Cargo.lock" ] || [ -f "Cargo.toml" ]; then echo "rust"
    elif [ -f "poetry.lock" ] || [ -f "pyproject.toml" ] || [ -f "requirements.txt" ]; then echo "python"
    elif [ -f "go.mod" ]; then echo "go"
    elif [ -f "package.json" ]; then echo "typescript"
    elif [ -f "pom.xml" ] || [ -f "build.gradle" ] || [ -f "build.gradle.kts" ]; then echo "java"
    else echo "unknown"; fi
}

LANG="$(detect_language)"

if [ "C.UTF-8" = "unknown" ]; then
    echo "⚠️  No project language detected — nothing to install." >&2
    exit 0
fi

LANG_SCRIPT="${SCRIPT_DIR}/languages/${LANG}/install_coverage_tools.sh"
if [ ! -f "$LANG_SCRIPT" ]; then
    echo "⚠️  No coverage installer for '${LANG}' yet (${LANG_SCRIPT})." >&2
    exit 0
fi

exec bash "$LANG_SCRIPT" "$@"
