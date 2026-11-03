#!/usr/bin/env bash
# ============================================================================
# coverage.sh — Real project coverage measurement + gate (dispatcher)
#
# Runs the project's REAL coverage tool for the detected language and reports
# line coverage. This is the honest replacement for the old per-module
# heuristic `*_coverage.sh` checks (coverage-theater — they counted test
# functions via grep, never a real percentage).
#
# Delegates to languages/<lang>/coverage.sh. Real coverage tool per language:
#   rust        — cargo llvm-cov (workspace line coverage, lcov artifact)
#   python      — coverage.py (pytest/unittest)
#   typescript  — vitest / jest / bun test --coverage (istanbul text)
#   go          — go test -coverprofile + go tool cover
#   java        — JaCoCo (maven/gradle)
#
# Usage:
#   bash .pi/scripts/coverage.sh            # measure + print summary
#   bash .pi/scripts/coverage.sh --gate     # exit 1 if lines < COVERAGE_THRESHOLD
#   bash .pi/scripts/coverage.sh --lcov     # also emit the tool's report artifact
#
# Env:
#   COVERAGE_THRESHOLD  minimum line-coverage % (default: 60 — a conservative
#                       floor; set it to the threshold your CI actually enforces)
#
# Exit codes:
#   0 — measured and (with --gate) meets threshold, or language not applicable
#   1 — coverage tool missing / below threshold / could not measure (honest
#       fail — never fake-pass)
#   2 — unknown argument
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

detect_language() {
    if [ -f "Cargo.lock" ] || [ -f "Cargo.toml" ]; then echo "rust"
    elif [ -f "go.mod" ]; then echo "go"
    elif [ -f "pom.xml" ] || [ -f "build.gradle" ] || [ -f "build.gradle.kts" ] || [ -d "src/main/java" ]; then echo "java"
    elif [ -f "package.json" ]; then echo "typescript"
    elif [ -f "pyproject.toml" ] || [ -f "requirements.txt" ]; then echo "python"
    else echo "unknown"; fi
}

LANG="$(detect_language)"

if [ "C.UTF-8" = "unknown" ]; then
    echo "⚠️  No project language detected — no real coverage gate applies." >&2
    exit 0
fi

LANG_SCRIPT="${SCRIPT_DIR}/languages/${LANG}/coverage.sh"
if [ ! -f "$LANG_SCRIPT" ]; then
    echo "⚠️  No coverage gate script for '${LANG}' yet (${LANG_SCRIPT})." >&2
    exit 0
fi

exec bash "$LANG_SCRIPT" "$@"
