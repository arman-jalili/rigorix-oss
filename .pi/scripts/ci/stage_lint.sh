#!/usr/bin/env bash
# Stage 3: Lint
#
# Verifies:
# - Language-specific linting (ruff/biome/clippy/golangci-lint)
# - Format check
# - Coverage report generation

set -euo pipefail

FAIL=0
PASS=0

log_pass() { echo "  ✓ PASS: $1"; ((PASS++)); }
log_fail() { echo "  ✗ FAIL: $1 — $2"; ((FAIL++)); }

echo "  Running lint checks..."

# Determine language
if [[ -f "pyproject.toml" || -f "requirements.txt" || -f "Pipfile" ]]; then
    LANG="python"
elif [[ -f "package.json" ]]; then
    LANG="typescript"
elif [[ -f "Cargo.toml" ]]; then
    LANG="rust"
elif [[ -f "go.mod" ]]; then
    LANG="go"
else
    echo "  No project configuration found, skipping lint."
    exit 0
fi

case "C.UTF-8" in
    python)
        echo "  Linting: Python"
        if command -v ruff &>/dev/null; then
            if ruff check . 2>/dev/null; then
                log_pass "ruff lint"
            else
                log_fail "ruff lint" "Lint errors found"
            fi
            if ruff format --check . 2>/dev/null; then
                log_pass "ruff format check"
            else
                log_fail "ruff format check" "Format errors found"
            fi
        elif command -v flake8 &>/dev/null; then
            if flake8 . 2>/dev/null; then
                log_pass "flake8 lint"
            else
                log_fail "flake8 lint" "Lint errors found"
            fi
        else
            log_fail "ruff lint" "ruff not installed"
        fi

        # Coverage report
        if command -v pytest &>/dev/null; then
            if pytest --cov=app --cov-report=xml --cov-report=term 2>/dev/null; then
                log_pass "coverage report"
            else
                log_fail "coverage report" "Tests or coverage failed"
            fi
        fi
        ;;

    typescript)
        echo "  Linting: TypeScript"
        if command -v biome &>/dev/null; then
            if biome check . 2>/dev/null; then
                log_pass "biome lint"
            else
                log_fail "biome lint" "Lint errors found"
            fi
            if biome format . --check 2>/dev/null; then
                log_pass "biome format check"
            else
                log_fail "biome format check" "Format errors found"
            fi
        elif command -v eslint &>/dev/null; then
            if eslint . 2>/dev/null; then
                log_pass "eslint lint"
            else
                log_fail "eslint lint" "Lint errors found"
            fi
        fi
        ;;

    rust)
        echo "  Linting: Rust"
        if command -v cargo &>/dev/null; then
            if cargo clippy --all-targets -- -D warnings 2>/dev/null; then
                log_pass "cargo clippy"
            else
                log_fail "cargo clippy" "Clippy warnings found"
            fi
            if cargo fmt --check 2>/dev/null; then
                log_pass "cargo fmt check"
            else
                log_fail "cargo fmt check" "Format errors found"
            fi
        fi
        ;;

    go)
        echo "  Linting: Go"
        if command -v golangci-lint &>/dev/null; then
            if golangci-lint run 2>/dev/null; then
                log_pass "golangci-lint"
            else
                log_fail "golangci-lint" "Lint errors found"
            fi
        fi
        if command -v gofmt &>/dev/null; then
            if gofmt -d . 2>/dev/null | grep -q .; then
                log_fail "gofmt check" "Format errors found"
            else
                log_pass "gofmt check"
            fi
        fi
        ;;
esac

if [[ $FAIL -gt 0 ]]; then
    echo "  Lint stage FAILED (${FAIL} failure(s))"
    exit 1
fi

echo "  Lint stage passed (${PASS} check(s))"
exit 0
