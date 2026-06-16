#!/usr/bin/env bash
# ============================================================================
# validate-ci.sh — Dispatcher
#
# Detects the project language and delegates to the language-specific script.
# Language-specific scripts live in: .pi/scripts/languages/<lang>/validate-ci.sh
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

detect_language() {
    if [ -f "${PROJECT_ROOT}/poetry.lock" ]; then
        echo "python"
    elif [ -f "${PROJECT_ROOT}/pyproject.toml" ]; then
        echo "python"
    elif [ -f "${PROJECT_ROOT}/Cargo.lock" ] || [ -f "${PROJECT_ROOT}/Cargo.toml" ]; then
        echo "rust"
    elif [ -f "${PROJECT_ROOT}/go.mod" ]; then
        echo "go"
    elif [ -f "${PROJECT_ROOT}/package.json" ]; then
        echo "typescript"
    else
        echo "unknown"
    fi
}

LANG=$(detect_language)
LANG_SCRIPT="${PROJECT_ROOT}/.pi/scripts/languages/${LANG}/validate-ci.sh"

if [ -f "$LANG_SCRIPT" ]; then
    exec bash "$LANG_SCRIPT" "$@"
else
    echo "No CI validator for language: ${LANG}"
    echo "Expected at: ${LANG_SCRIPT}"
    exit 0
fi
