#!/usr/bin/env bash
# ============================================================================
# validate-ci.sh — Dispatcher
#
# Detects the project language and delegates to the language-specific script.
# Language-specific scripts live in: .pi/scripts/languages/<lang>/validate-ci.sh
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

detect_language() {
    if [ -f "poetry.lock" ]; then
        echo "python"
    elif [ -f "pyproject.toml" ]; then
        echo "python"
    elif [ -f "Cargo.lock" ] || [ -f "Cargo.toml" ]; then
        echo "rust"
    elif [ -f "go.mod" ]; then
        echo "go"
    elif [ -f "package.json" ]; then
        echo "typescript"
    else
        echo "unknown"
    fi
}

LANG=$(detect_language)
LANG_SCRIPT="${SCRIPT_DIR}/languages/${LANG}/validate-ci.sh"

if [ -f "$LANG_SCRIPT" ]; then
    exec bash "$LANG_SCRIPT" "$@"
else
    echo "No CI validator for language: C.UTF-8"
    echo "Create: $LANG_SCRIPT"
    exit 0
fi
