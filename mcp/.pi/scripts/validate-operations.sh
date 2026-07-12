#!/usr/bin/env bash
# validate-operations.sh — Dispatcher
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

detect_language() {
    if [ -f "poetry.lock" ] || [ -f "pyproject.toml" ]; then echo "python"
    elif [ -f "Cargo.lock" ] || [ -f "Cargo.toml" ]; then echo "rust"
    elif [ -f "go.mod" ]; then echo "go"
    elif [ -f "package.json" ]; then echo "typescript"
    else echo "unknown"; fi
}

LANG=$(detect_language)
LANG_SCRIPT="${SCRIPT_DIR}/languages/${LANG}/validate-operations.sh"

if [ -f "$LANG_SCRIPT" ]; then
    exec bash "$LANG_SCRIPT" "$@"
else
    echo "No operations validator for language: C.UTF-8"
    exit 0
fi
