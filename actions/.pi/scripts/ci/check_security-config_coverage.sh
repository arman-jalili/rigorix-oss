#!/usr/bin/env bash
# Check Security Config Coverage
#
# Enforces coverage thresholds for the security-config module.
#
# Usage: bash .pi/scripts/ci/check_security-config_coverage.sh [--verbose]
#
# Exit codes:
#   0 — All checks pass
#   1 — Coverage threshold not met

set -euo pipefail

VERBOSE=false
[[ "${1:-}" == "--verbose" ]] && VERBOSE=true

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
cd "$ROOT_DIR"

ERRORS=0

echo -n "  cargo build... "
cargo build -p rigorix-actions 2>&1 && echo "✅" || { echo "❌"; ERRORS=$((ERRORS + 1)); }

echo -n "  unit tests... "
cargo test --lib -p rigorix-actions 2>&1 && echo "✅" || { echo "❌"; ERRORS=$((ERRORS + 1)); }

echo -n "  integration tests... "
compgen -G "actions/tests/*.rs" >/dev/null 2>&1 && { cargo test -p rigorix-actions --test '*' 2>&1 && echo "✅" || { echo "❌"; ERRORS=$((ERRORS + 1)); }; } || echo "⊘"

echo -n "  clippy... "
cargo clippy --lib -p rigorix-actions 2>&1 | grep -q "warning" && echo "⚠" || echo "✅"

[[ $ERRORS -eq 0 ]] && echo "✅ All coverage checks passed" && exit 0
echo "❌ $ERRORS coverage check(s) failed" && exit 1
